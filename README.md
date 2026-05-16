# multipeerconnectivity-rs

Safe Rust bindings for Apple's [MultipeerConnectivity](https://developer.apple.com/documentation/multipeerconnectivity) framework on macOS — peer IDs, sessions, nearby browser/advertiser APIs, advertiser assistant, browser view controller, and typed `MCError` handling.

> **Status:** experimental. v0.2 covers the public MultipeerConnectivity surface for `MCPeerID`, `MCSession`, `MCNearbyServiceAdvertiser`, `MCNearbyServiceBrowser`, `MCAdvertiserAssistant`, `MCBrowserViewController`, and `MCError`.

## Package vs crate name

- Cargo package: `multipeerconnectivity-rs`
- Rust crate: `multipeerconnectivity`

## Quick start

```rust,no_run
use multipeerconnectivity::prelude::*;

fn main() -> Result<()> {
    let peer = PeerId::new("doom-fish-demo")?;
    let session = Session::new(&peer, EncryptionPreference::Optional)?;
    let browser = NearbyServiceBrowser::new(&peer, "doom-chat")?;

    println!("local peer = {}", peer.display_name());
    println!("browser service type = {}", browser.service_type());
    println!("connected peers = {}", session.connected_peers().len());
    Ok(())
}
```

## Covered areas

- `MCPeerID` creation, display name access, and `NSSecureCoding` archive/unarchive helpers
- `MCSession` creation, connected-peer inspection, send/resource/stream helpers, custom discovery, and delegate callbacks for state/data/stream/resource/certificate events
- `MCNearbyServiceAdvertiser` creation, property access, invitation handling, and startup-failure callbacks
- `MCNearbyServiceBrowser` creation, property access, invitations, and startup-failure callbacks
- `MCAdvertiserAssistant` construction, property access, start/stop, and invitation presentation callbacks
- `MCBrowserViewController` construction, browser/session access, peer-limit tuning, and delegate callbacks
- `MCError` domain lookup plus typed `MCErrorCode` mapping

## Delegate callbacks

`MCSession`, `MCNearbyServiceBrowser`, `MCNearbyServiceAdvertiser`, `MCAdvertiserAssistant`, and `MCBrowserViewController` all use Swift-side delegate objects that call back into Rust via function pointers + refcon. The safe Rust API wraps that in builder-style delegate structs such as `SessionDelegate` and `BrowserViewControllerDelegate`.

## Examples

```bash
cargo run --example 01_mcpeerid_roundtrip
cargo run --example 02_mcsession_properties
cargo run --example 03_mcnearbyserviceadvertiser_properties
cargo run --example 04_mcnearbyservicebrowser_properties
cargo run --example 05_mcadvertiserassistant_properties
cargo run --example 06_mcbrowserviewcontroller_properties
cargo run --example 07_mcerror_domain
```

## Notes

- `Session::with_security_identity(...)` keeps the existing raw-pointer escape hatch for advanced Security.framework users.
- `Session::with_security_identity_items(...)` accepts identity items previously returned by the framework.
- `Session::nearby_connection_data_for_peer(...)`, `connect_peer(...)`, and `cancel_connect_peer(...)` cover Apple's custom-discovery extension.
- `BrowserViewController` and `AdvertiserAssistant` are created on the main thread inside the Swift bridge so they can be used safely from tests/examples.
- UI/network-sensitive behavior is intentionally left out of the examples so they exit successfully on a headless macOS machine.

## Coverage matrix

See [COVERAGE.md](COVERAGE.md) for the detailed API audit.

## License

Licensed under either of [Apache-2.0](LICENSE-APACHE) or [MIT](LICENSE-MIT) at your option.
