# Changelog

## 0.3.1

- Added `catch_user_panic` wrappers to all `extern "C"` trampolines to prevent
  panics from unwinding across the FFI boundary into Swift (UB).
- Fixed `doom-fish-utils` version range to `>=0.1, <0.3` per workspace hygiene rules.

## 0.3.0

- Added `async` Cargo feature with `src/async_api.rs` module providing Tier-2
  async event streams backed by `doom-fish-utils::BoundedAsyncStream`.
- Added `SessionEventStream` wrapping all six `MCSessionDelegate` callbacks
  (state change, data received, stream received, resource started/finished,
  certificate received).
- Added `BrowserEventStream` wrapping all three `MCNearbyServiceBrowserDelegate`
  callbacks (found peer, lost peer, did-not-start).
- Added `AdvertiserEventStream` wrapping both `MCNearbyServiceAdvertiserDelegate`
  callbacks (received invitation, did-not-start), with an `InvitationHandle`
  RAII type for accept/decline responses.
- Added three examples: `08_async_session_stream`, `09_async_browser_stream`,
  `10_async_advertiser_stream`.
- All streams uninstall their delegate and close the event channel on drop.

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
