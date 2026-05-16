// swift-tools-version:5.9
import PackageDescription

let package = Package(
    name: "MultipeerConnectivityBridge",
    platforms: [.macOS(.v13)],
    products: [
        .library(
            name: "MultipeerConnectivityBridge",
            type: .static,
            targets: ["MultipeerConnectivityBridge"]
        ),
    ],
    targets: [
        .target(
            name: "MultipeerConnectivityBridge",
            path: "Sources/MultipeerConnectivityBridge",
            publicHeadersPath: "include"
        ),
    ]
)
