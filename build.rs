use std::env;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=DOCS_RS");
    if env::var("DOCS_RS").is_ok() {
        return;
    }

    println!("cargo:rustc-link-lib=framework=MultipeerConnectivity");
    println!("cargo:rustc-link-lib=framework=Foundation");

    let swift_dir = "swift-bridge";
    let out_dir = env::var("OUT_DIR").expect("OUT_DIR");
    let swift_build_dir = format!("{out_dir}/swift-build");
    println!("cargo:rerun-if-changed={swift_dir}");

    let arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
    let triple = match arch.as_str() {
        "x86_64" => "x86_64-apple-macosx",
        "aarch64" => "arm64-apple-macosx",
        other => panic!("multipeerconnectivity: unsupported arch '{other}'"),
    };

    let out = Command::new("swift")
        .args([
            "build",
            "-c",
            "release",
            "--triple",
            triple,
            "--package-path",
            swift_dir,
            "--scratch-path",
            &swift_build_dir,
        ])
        .output()
        .expect("swift build");
    if !out.status.success() {
        eprintln!("STDOUT:\n{}", String::from_utf8_lossy(&out.stdout));
        eprintln!("STDERR:\n{}", String::from_utf8_lossy(&out.stderr));
        panic!("swift build failed");
    }

    println!("cargo:rustc-link-search=native={swift_build_dir}/release");
    println!("cargo:rustc-link-lib=static=MultipeerConnectivityBridge");
    println!("cargo:rustc-link-arg=-Wl,-rpath,/usr/lib/swift");

    if let Ok(output) = Command::new("xcode-select").arg("-p").output() {
        if output.status.success() {
            let xcode = String::from_utf8_lossy(&output.stdout).trim().to_string();
            println!(
                "cargo:rustc-link-arg=-Wl,-rpath,{xcode}/Toolchains/XcodeDefault.xctoolchain/usr/lib/swift/macosx"
            );
        }
    }
}
