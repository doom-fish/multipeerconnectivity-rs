# multipeerconnectivity-rs

Safe Rust bindings for Apple's [MultipeerConnectivity](https://developer.apple.com/documentation/multipeerconnectivity) framework on macOS — peer IDs, sessions, nearby browser/advertiser APIs, advertiser assistant, browser view controller, and typed `MCError` handling.

> **Status:** experimental. v0.4 covers the public MultipeerConnectivity surface for `MCPeerID`, `MCSession`, `MCNearbyServiceAdvertiser`, `MCNearbyServiceBrowser`, `MCAdvertiserAssistant`, `MCBrowserViewController`, `MCError`, all five public delegate protocols via builder-style Rust wrappers, and Tier-2 async event streams where available.

Apple deprecates the whole `MultipeerConnectivity` framework in the macOS 27 SDK ("Use Network Framework instead"). It still works, but prefer Network framework for new code.

## Requirements

- macOS 13 or later, the deployment target of the Swift bridge, and a Swift toolchain (Xcode or the Command Line Tools) to build it.
- Apps that browse or advertise must list `_<service-type>._tcp` and `_<service-type>._udp` under `NSBonjourServices` and set `NSLocalNetworkUsageDescription` in their `Info.plist`; macOS 15 and later can ask the user for local network access. Sandboxed apps also need the `com.apple.security.network.client` and `com.apple.security.network.server` entitlements.

## Package vs crate name

- Cargo package: `multipeerconnectivity-rs`
- Rust crate: `multipeerconnectivity`

## Quick start

```rust,no_run
use multipeerconnectivity::prelude::*;

fn main() -> Result<()> {
    let peer = PeerId::new("doom-fish-demo")?;
    let session = Session::new(&peer, EncryptionPreference::Required)?;
    let browser = NearbyServiceBrowser::new(&peer, "doom-chat")?;

    println!("local peer = {}", peer.display_name());
    println!("browser service type = {}", browser.service_type());
    println!("connected peers = {}", session.connected_peers().len());
    Ok(())
}
```

## Encryption

Create sessions with `EncryptionPreference::Required`, which is also `EncryptionPreference::default()`. `Optional` accepts unencrypted connections, so a nearby attacker can downgrade the session to plaintext, and `None` turns encryption off. Peers that use `None` can't join a `Required` session. Encryption alone doesn't authenticate peers; see the certificate notes below.

## Async API

Enable the optional `async` Cargo feature to access executor-agnostic event streams backed by `doom-fish-utils::stream::BoundedAsyncStream`.

```rust
# #[cfg(feature = "async")]
# {
use multipeerconnectivity::{EncryptionPreference, PeerId, Session};
use multipeerconnectivity::async_api::SessionEventStream;

# fn demo() -> multipeerconnectivity::Result<()> {
let peer = PeerId::new("async-demo")?;
let session = Session::new(&peer, EncryptionPreference::Required)?;
let stream = SessionEventStream::subscribe_default(&session);
assert!(!stream.is_closed());
# Ok(())
# }
# }
```

The feature adds `SessionEventStream`, `BrowserEventStream`, and `AdvertiserEventStream`. Each stream unsubscribes automatically when dropped.

Each object has a single delegate, so subscribing a stream takes over from the current one. Dropping the stream hands the delegate back to the one it replaced (a `set_callbacks` delegate or an older stream) if that one is still active, and it never detaches a delegate that was installed after the stream. Likewise `clear_delegate` only removes the delegate that its own handle installed.

`SessionEventStream` never accepts a peer on its own. Each `SessionEvent::CertificateReceived` carries a `CertificateHandle`, and the peer can connect only after you call `accept()` on it. Calling `reject()`, dropping the handle, or dropping the event unread (including when the stream is dropped or its buffer overflows) refuses the peer. The framework does not validate certificates, so check them before accepting. Peers without a security identity arrive with no certificate items and still need an explicit `accept()`.

Async examples:

```bash
cargo run --example 08_async_session_stream --features async
cargo run --example 09_async_browser_stream --features async
cargo run --example 10_async_advertiser_stream --features async
```

## Covered areas

- `MCPeerID` creation, display name access, and `NSSecureCoding` archive/unarchive helpers
- `MCSession` creation, connected-peer inspection, send/resource/stream helpers, custom discovery, and delegate callbacks for state/data/stream/resource/certificate events
- `MCNearbyServiceAdvertiser` creation, property access, invitation handling, and startup-failure callbacks
- `MCNearbyServiceBrowser` creation, property access, invitations, and startup-failure callbacks
- `MCAdvertiserAssistant` construction, property access, start/stop, and invitation presentation callbacks
- `MCBrowserViewController` construction, browser/session access, peer-limit tuning, and delegate callbacks
- `MCError` domain lookup plus typed `MCErrorCode` mapping
- Tier-2 async event streams for session, browser, and advertiser delegates

## Delegate callbacks

`MCSession`, `MCNearbyServiceBrowser`, `MCNearbyServiceAdvertiser`, `MCAdvertiserAssistant`, and `MCBrowserViewController` all use Swift-side delegate objects that call back into Rust via function pointers + refcon. The safe Rust API wraps that in builder-style delegate structs such as `SessionDelegate` and `BrowserViewControllerDelegate`.

A `SessionDelegate` without `on_certificate` leaves the decision to the framework, which accepts every peer certificate without validating it. Register `on_certificate` and return `false` for peers you don't trust.

## Examples

```bash
cargo run --example 01_mcpeerid_roundtrip
cargo run --example 02_mcsession_properties
cargo run --example 03_mcnearbyserviceadvertiser_properties
cargo run --example 04_mcnearbyservicebrowser_properties
cargo run --example 05_mcadvertiserassistant_properties
cargo run --example 06_mcbrowserviewcontroller_properties
cargo run --example 07_mcerror_domain
cargo run --example 08_async_session_stream --features async
cargo run --example 09_async_browser_stream --features async
cargo run --example 10_async_advertiser_stream --features async
```

## Notes

- Service types must be 1–15 lowercase ASCII letters, digits, or hyphens, with at least one letter and no leading, trailing, or doubled hyphen (RFC 6335). Discovery-info keys must be non-empty printable ASCII without `=`, and each `key=value` pair can be at most 254 UTF-8 bytes. The constructors return `MultipeerError::InvalidArgument` for anything else, because the framework raises an uncatchable exception for it.
- `Session::with_security_identity(...)` keeps the existing raw-pointer escape hatch for advanced Security.framework users.
- `Session::with_security_identity_items(...)` accepts identity items previously returned by the framework.
- `Session::nearby_connection_data_for_peer(...)`, `connect_peer(...)`, and `cancel_connect_peer(...)` cover Apple's custom-discovery extension.
- `BrowserViewController` and `AdvertiserAssistant` are created on the main thread inside the Swift bridge so they can be used safely from tests/examples.
- UI/network-sensitive behavior is intentionally left out of the examples so they exit successfully on a headless macOS machine.

## Coverage matrix

See [COVERAGE.md](COVERAGE.md) for the detailed API audit.

## License

Licensed under either of [Apache-2.0](LICENSE-APACHE) or [MIT](LICENSE-MIT) at your option.
