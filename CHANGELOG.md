# Changelog

## 0.2.0

- Renamed the Cargo package to `multipeerconnectivity-rs` while keeping the Rust crate name `multipeerconnectivity`.
- Split the Swift bridge and Rust FFI into logical-area modules following the ScreenCaptureKit bridge pattern.
- Added full wrappers for `MCAdvertiserAssistant`, `MCBrowserViewController`, and typed `MCError` handling.
- Expanded `MCSession` with custom-discovery helpers, richer delegate callbacks, property getters, and stream/resource receive wrappers.
- Added numbered examples and per-area tests covering every required MultipeerConnectivity area.
- Added `COVERAGE.md` documenting the framework audit.

## 0.1.0

- Initial release.
- Added `PeerId`, `Session`, `NearbyServiceBrowser`, and `NearbyServiceAdvertiser`.
- Added callback-based delegate bridges for session state/data, browser found/lost peer, and advertiser invitations.
- Added smoke example that creates a peer, session, and browser without touching the network.
