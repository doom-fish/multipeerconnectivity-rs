# multipeerconnectivity

Safe Rust bindings for Apple's [MultipeerConnectivity](https://developer.apple.com/documentation/multipeerconnectivity) framework on macOS — peer discovery, invitations, sessions, data sends, resource transfer, and byte streams for ad-hoc local networking.

> **Status:** experimental. v0.1 ships `MCPeerID`, `MCSession`, `MCNearbyServiceBrowser`, and `MCNearbyServiceAdvertiser` with callback-based delegate bridges. The smoke example intentionally avoids scanning or browsing so it won't trigger permission prompts on recent macOS releases.

## Quick start

```rust,no_run
use multipeerconnectivity::prelude::*;

fn main() -> Result<()> {
    let peer = PeerId::new("doom-fish-demo")?;
    let session = Session::new(&peer, EncryptionPreference::Optional)?;
    let _browser = NearbyServiceBrowser::new(&peer, "doom-demo")?;

    println!("local peer = {}", peer.display_name());
    println!("connected peers = {}", session.connected_peers().len());
    Ok(())
}
```

## Delegate callbacks

`MCSession`, `MCNearbyServiceBrowser`, and `MCNearbyServiceAdvertiser` use Swift-side inner delegate classes that call back into Rust via function pointers + refcon. The safe Rust API wraps that in closure-based `set_delegate(...)` helpers.

## Smoke example

```bash
cargo run --example 01_smoke
```

Expected output:

```text
peer display name: doom-fish-smoke
✅ multipeer peer + session OK
```

## Notes

- `Session::with_security_identity(...)` accepts the raw `[SecIdentityRef, certs...]` pointer array that Apple's API expects. Passing `None` is the common case.
- `NearbyServiceAdvertiser::set_delegate(...)` accepts an optional `Session` to use when an invitation is accepted.
- `send_resource` returns an opaque `ResourceTransfer` wrapper over `NSProgress`.
- `start_stream` returns an `OutputStream` wrapper over `NSOutputStream` with `open`, `write`, and `close` helpers.

## Roadmap

- [x] `PeerId::new` + `display_name`
- [x] `Session::{new, with_security_identity, connected_peers, send, send_resource, start_stream, disconnect}`
- [x] `NearbyServiceAdvertiser` + invitation delegate
- [x] `NearbyServiceBrowser` + found/lost delegate + `invite_peer`
- [ ] Custom-discovery helpers (`nearbyConnectionDataForPeer`, `connectPeer`, `cancelConnectPeer`)
- [ ] Resource receive / stream receive delegate wrappers
- [ ] Certificate acceptance delegate hook

## License

Licensed under either of [Apache-2.0](LICENSE-APACHE) or [MIT](LICENSE-MIT) at your option.
