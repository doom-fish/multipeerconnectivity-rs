# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.5.0] - Unreleased

### Security

- `SessionEventStream` no longer accepts every peer certificate on the
  caller's behalf. `SessionEvent::CertificateReceived` carries a
  `CertificateHandle`: the peer connects only after `accept()`, while
  `reject()`, dropping the handle, or dropping the event unread (stream
  drop, buffer overflow) refuses it. **Breaking:** stream consumers must
  accept peers explicitly, including peers without a security identity.
- `EncryptionPreference::Required` is the documented default and every
  example uses it. The docs used to call `Optional` the default, but it
  accepts unencrypted connections, so a nearby attacker could downgrade a
  session. The bridge maps an unknown raw preference to `Required`.
- Delegate and async-stream contexts are reference-counted between Rust and
  the Swift delegate objects, so a callback already running on the
  framework's queue can no longer use a freed context after
  `clear_delegate` or a stream drop.

### Fixed

- Service types that break RFC 6335 (no letter, a leading, trailing or
  doubled hyphen) and discovery info with an empty or non-printable-ASCII
  key, a key containing `=`, or a `key=value` pair over 254 bytes return
  `MultipeerError::InvalidArgument`. They used to abort the process with an
  uncaught `NSInvalidArgumentException`.
- The Swift bridge reports its validation errors instead of silently
  dropping malformed discovery-info JSON or returning a generic "failed to
  create" error.
- Dropping an async stream or calling `clear_delegate` no longer detaches a
  delegate installed later (another stream, `set_callbacks`, or a clone's
  delegate). A dropped stream hands the delegate back to the one it
  replaced if that one is still active.
- The Swift bridge converts error codes and enum raw values with clamping
  instead of trapping on out-of-range values.

### Changed

- **Breaking:** `SessionEvent::CertificateReceived` has a new
  `handle: CertificateHandle` field.
- **Breaking (raw FFI):** `mpc_advertiser_create`, `mpc_browser_create`,
  `mpc_advertiser_assistant_create` and
  `mpc_browser_view_controller_create_with_service_type` take an error
  out-pointer; the `mpc_*_clear_delegate` functions take the owner's
  context; `mpc_*_set_delegate` and `mpc_*_stream_subscribe` take context
  retain/release callbacks.
- Requires `doom-fish-utils` `>=0.4.1, <0.5`.
- `rust-version` is now 1.82 (was 1.76).

### Added

- `async_api::CertificateHandle` with `accept()` and `reject()`.
- `EncryptionPreference` implements `Default`, returning `Required`.

## [0.4.1] - 2026-05-20

- Widen `doom-fish-utils` dependency bound to `<0.4` so the 0.3.x SPSC-ring release resolves cleanly. No source changes.

## 0.4.0

- Explicitly documented the five public delegate protocol declarations in `COVERAGE_AUDIT.md` / `COVERAGE_AUDIT_V2.md`, mapping them to the Rust callback builders and async event streams that already back the safe API.
- Expanded delegate setup tests to exercise all five delegate wrapper builders, including the session / browser / advertiser async-backed surfaces.

## 0.3.2

- Added concise rustdoc coverage across the non-FFI MultipeerConnectivity wrapper API, raising public-item coverage above 60%.

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
