//! Async stream wrappers for `MultipeerConnectivity` delegates.
//!
//! This module exposes the three main delegate surfaces of the framework as
//! executor-agnostic [`doom_fish_utils::stream::BoundedAsyncStream`] event
//! streams. The streams are lossy by default: if the consumer is too slow the
//! oldest buffered event is dropped to make room for the newest.
//!
//! # Feature gate
//!
//! Enable the `async` Cargo feature to compile this module.
//!
//! ```toml
//! [dependencies]
//! multipeerconnectivity = { package = "multipeerconnectivity-rs", version = "0.4", features = ["async"] }
//! ```
//!
//! # Quick start
//!
//! ```no_run
//! use multipeerconnectivity::async_api::{SessionEvent, SessionEventStream};
//! use multipeerconnectivity::{EncryptionPreference, PeerId, SecurityIdentityItem, Session};
//!
//! # fn is_trusted(_peer: &PeerId, _items: &[SecurityIdentityItem]) -> bool { false }
//! # async fn run() -> multipeerconnectivity::Result<()> {
//! let peer = PeerId::new("my-peer")?;
//! let session = Session::new(&peer, EncryptionPreference::Required)?;
//! let stream = SessionEventStream::subscribe_default(&session);
//!
//! while let Some(event) = stream.next().await {
//!     match event {
//!         SessionEvent::CertificateReceived { peer, items, handle } => {
//!             if is_trusted(&peer, &items) {
//!                 handle.accept();
//!             } else {
//!                 handle.reject();
//!             }
//!         }
//!         other => println!("session event: {other:?}"),
//!     }
//! }
//! # Ok(())
//! # }
//! ```

use core::ptr::NonNull;
use std::collections::HashMap;
use std::ffi::{c_void, CStr};
use std::path::PathBuf;

use doom_fish_utils::panic_safe::catch_user_panic;
use doom_fish_utils::stream::{AsyncStreamSender, BoundedAsyncStream, NextItem};

use crate::advertiser::NearbyServiceAdvertiser;
use crate::browser::NearbyServiceBrowser;
use crate::error::{take_framework_error, FrameworkError};
use crate::peer::PeerId;
use crate::session::{InputStream, ResourceTransfer, SecurityIdentityItem, Session, SessionState};

/// Default event buffer capacity for all stream types.
pub const DEFAULT_CAPACITY: usize = 64;

type EventCallback = unsafe extern "C" fn(i32, *const c_void, *mut c_void);
type CtxRetainCallback = unsafe extern "C" fn(*mut c_void);

extern "C" {
    fn mpc_session_stream_subscribe(
        session: *mut c_void,
        on_event: EventCallback,
        ctx: *mut c_void,
        ctx_retain: CtxRetainCallback,
        ctx_release: CtxRetainCallback,
    ) -> *mut c_void;
    fn mpc_session_stream_unsubscribe(handle: *mut c_void);

    fn mpc_browser_stream_subscribe(
        browser: *mut c_void,
        on_event: EventCallback,
        ctx: *mut c_void,
        ctx_retain: CtxRetainCallback,
        ctx_release: CtxRetainCallback,
    ) -> *mut c_void;
    fn mpc_browser_stream_unsubscribe(handle: *mut c_void);

    fn mpc_advertiser_stream_subscribe(
        advertiser: *mut c_void,
        on_event: EventCallback,
        ctx: *mut c_void,
        ctx_retain: CtxRetainCallback,
        ctx_release: CtxRetainCallback,
    ) -> *mut c_void;
    fn mpc_advertiser_stream_unsubscribe(handle: *mut c_void);

    fn mpc_invitation_handle_accept(handle: *mut c_void, session: *mut c_void);
    fn mpc_invitation_handle_decline(handle: *mut c_void);
    fn mpc_certificate_handle_respond(handle: *mut c_void, accept: bool);

    /// Cross-language ABI check for the packed event payload structs. Returns
    /// `true` only if the Swift `MemoryLayout` matches the Rust layout asserted
    /// below. Verified by `tests/async_stream_tests.rs`.
    fn mpc_async_verify_ffi_layout() -> bool;
}

/// Reference-counted wrapper around the event-stream sender shared with Swift.
///
/// The raw pointer to this box is handed to the Swift bridge as the callback
/// context. Swift takes a `+1` in the bridge object's `init` and drops it in
/// `deinit` (via the `ctx_retain`/`ctx_release` trampolines), so an event
/// callback already in flight on a background queue can never deref a freed
/// sender when the Rust [`SubscriptionHandle`] is dropped.
struct SenderContext<E> {
    sender: AsyncStreamSender<E>,
    ref_count: crate::refcount::RefCount,
}

impl<E> SenderContext<E> {
    fn new(sender: AsyncStreamSender<E>) -> *mut Self {
        Box::into_raw(Box::new(Self {
            sender,
            ref_count: crate::refcount::RefCount::new(),
        }))
    }
}

impl<E> crate::refcount::RefCounted for SenderContext<E> {
    fn ref_count(&self) -> &crate::refcount::RefCount {
        &self.ref_count
    }
}

extern "C" fn session_ctx_retain(ctx: *mut c_void) {
    unsafe { crate::refcount::retain::<SenderContext<SessionEvent>>(ctx) };
}
extern "C" fn session_ctx_release(ctx: *mut c_void) {
    unsafe { crate::refcount::release::<SenderContext<SessionEvent>>(ctx) };
}
extern "C" fn browser_ctx_retain(ctx: *mut c_void) {
    unsafe { crate::refcount::retain::<SenderContext<BrowserEvent>>(ctx) };
}
extern "C" fn browser_ctx_release(ctx: *mut c_void) {
    unsafe { crate::refcount::release::<SenderContext<BrowserEvent>>(ctx) };
}
extern "C" fn advertiser_ctx_retain(ctx: *mut c_void) {
    unsafe { crate::refcount::retain::<SenderContext<AdvertiserEvent>>(ctx) };
}
extern "C" fn advertiser_ctx_release(ctx: *mut c_void) {
    unsafe { crate::refcount::release::<SenderContext<AdvertiserEvent>>(ctx) };
}

/// Drops the async bridge and releases the boxed sender context when the stream
/// is dropped.
struct SubscriptionHandle<E> {
    bridge_handle: *mut c_void,
    ctx: *mut SenderContext<E>,
    unsubscribe_fn: unsafe extern "C" fn(*mut c_void),
    ctx_release_fn: unsafe extern "C" fn(*mut c_void),
}

impl<E> Drop for SubscriptionHandle<E> {
    fn drop(&mut self) {
        // Unsubscribe first so the Swift bridge object detaches its delegate
        // and (absent any in-flight callback) drops its `+1` on the context.
        if !self.bridge_handle.is_null() {
            unsafe { (self.unsubscribe_fn)(self.bridge_handle) };
        }
        // Then drop the Rust-owned reference. The box is freed only once the
        // Swift side has also released, so an in-flight `event_cb` is safe.
        if !self.ctx.is_null() {
            unsafe { (self.ctx_release_fn)(self.ctx.cast()) };
        }
    }
}

// SAFETY: the Swift bridge is thread-safe; the sender is only touched through
// its shared-memory lock.
unsafe impl<E: Send> Send for SubscriptionHandle<E> {}
// SAFETY: see `Send` above; shared access only reaches the internally locked sender.
unsafe impl<E: Send> Sync for SubscriptionHandle<E> {}

// MARK: - ABI Layout Assertions
//
// The `#[repr(C)]` payload structs below are written by the Swift async bridge
// (`swift-bridge/Sources/MultipeerConnectivityBridge/AsyncStream.swift`) and
// read back here by casting the opaque `payload` pointer in each `*_event_cb`.
// If a field type, order, or padding ever drifts between the two sides the
// marshalled data silently corrupts. These compile-time assertions pin the
// exact ABI; the cross-language `mpc_async_verify_ffi_layout` check in
// `tests/async_stream_tests.rs` guards that Swift agrees.
use core::mem::{align_of, offset_of, size_of};

const _: () = assert!(size_of::<SessionStatePayload>() == 16);
const _: () = assert!(align_of::<SessionStatePayload>() == 8);
const _: () = assert!(offset_of!(SessionStatePayload, peer) == 0);
const _: () = assert!(offset_of!(SessionStatePayload, state) == 8);

const _: () = assert!(size_of::<SessionDataPayload>() == 24);
const _: () = assert!(align_of::<SessionDataPayload>() == 8);
const _: () = assert!(offset_of!(SessionDataPayload, peer) == 0);
const _: () = assert!(offset_of!(SessionDataPayload, data) == 8);
const _: () = assert!(offset_of!(SessionDataPayload, len) == 16);

const _: () = assert!(size_of::<SessionStreamPayload>() == 24);
const _: () = assert!(align_of::<SessionStreamPayload>() == 8);
const _: () = assert!(offset_of!(SessionStreamPayload, peer) == 0);
const _: () = assert!(offset_of!(SessionStreamPayload, name) == 8);
const _: () = assert!(offset_of!(SessionStreamPayload, stream) == 16);

const _: () = assert!(size_of::<SessionResourceStartPayload>() == 24);
const _: () = assert!(align_of::<SessionResourceStartPayload>() == 8);
const _: () = assert!(offset_of!(SessionResourceStartPayload, peer) == 0);
const _: () = assert!(offset_of!(SessionResourceStartPayload, name) == 8);
const _: () = assert!(offset_of!(SessionResourceStartPayload, progress) == 16);

const _: () = assert!(size_of::<SessionResourceFinishPayload>() == 32);
const _: () = assert!(align_of::<SessionResourceFinishPayload>() == 8);
const _: () = assert!(offset_of!(SessionResourceFinishPayload, peer) == 0);
const _: () = assert!(offset_of!(SessionResourceFinishPayload, name) == 8);
const _: () = assert!(offset_of!(SessionResourceFinishPayload, url_path) == 16);
const _: () = assert!(offset_of!(SessionResourceFinishPayload, error) == 24);

const _: () = assert!(size_of::<SessionCertPayload>() == 32);
const _: () = assert!(align_of::<SessionCertPayload>() == 8);
const _: () = assert!(offset_of!(SessionCertPayload, peer) == 0);
const _: () = assert!(offset_of!(SessionCertPayload, items) == 8);
const _: () = assert!(offset_of!(SessionCertPayload, count) == 16);
const _: () = assert!(offset_of!(SessionCertPayload, handler) == 24);

const _: () = assert!(size_of::<BrowserFoundPayload>() == 16);
const _: () = assert!(align_of::<BrowserFoundPayload>() == 8);
const _: () = assert!(offset_of!(BrowserFoundPayload, peer) == 0);
const _: () = assert!(offset_of!(BrowserFoundPayload, discovery_json) == 8);

const _: () = assert!(size_of::<BrowserLostPayload>() == 8);
const _: () = assert!(align_of::<BrowserLostPayload>() == 8);
const _: () = assert!(offset_of!(BrowserLostPayload, peer) == 0);

const _: () = assert!(size_of::<BrowserErrorPayload>() == 8);
const _: () = assert!(align_of::<BrowserErrorPayload>() == 8);
const _: () = assert!(offset_of!(BrowserErrorPayload, error) == 0);

const _: () = assert!(size_of::<AdvertiserInvitationPayload>() == 32);
const _: () = assert!(align_of::<AdvertiserInvitationPayload>() == 8);
const _: () = assert!(offset_of!(AdvertiserInvitationPayload, peer) == 0);
const _: () = assert!(offset_of!(AdvertiserInvitationPayload, context_ptr) == 8);
const _: () = assert!(offset_of!(AdvertiserInvitationPayload, context_len) == 16);
const _: () = assert!(offset_of!(AdvertiserInvitationPayload, invitation_handle) == 24);

const _: () = assert!(size_of::<AdvertiserErrorPayload>() == 8);
const _: () = assert!(align_of::<AdvertiserErrorPayload>() == 8);
const _: () = assert!(offset_of!(AdvertiserErrorPayload, error) == 0);

/// Asks the Swift bridge to confirm its `MemoryLayout` for every packed event
/// payload struct matches the Rust layout pinned by the `const _` asserts above.
///
/// Returns `false` on a genuine ABI mismatch between the Rust and Swift sides.
#[must_use]
pub fn verify_ffi_layout() -> bool {
    // SAFETY: the Swift function takes no arguments and only reads compile-time
    // `MemoryLayout` constants.
    unsafe { mpc_async_verify_ffi_layout() }
}

#[repr(C)]
struct SessionStatePayload {
    peer: *mut c_void,
    state: i32,
}

#[repr(C)]
struct SessionDataPayload {
    peer: *mut c_void,
    data: *const c_void,
    len: usize,
}

#[repr(C)]
struct SessionStreamPayload {
    peer: *mut c_void,
    name: *const std::ffi::c_char,
    stream: *mut c_void,
}

#[repr(C)]
struct SessionResourceStartPayload {
    peer: *mut c_void,
    name: *const std::ffi::c_char,
    progress: *mut c_void,
}

#[repr(C)]
struct SessionResourceFinishPayload {
    peer: *mut c_void,
    name: *const std::ffi::c_char,
    url_path: *const std::ffi::c_char,
    error: *mut c_void,
}

#[repr(C)]
struct SessionCertPayload {
    peer: *mut c_void,
    items: *mut *mut c_void,
    count: usize,
    handler: *mut c_void,
}

#[derive(Default)]
pub struct CertificateHandle {
    ptr: Option<*mut c_void>,
}

impl CertificateHandle {
    pub fn accept(mut self) {
        if let Some(ptr) = self.ptr.take() {
            unsafe { mpc_certificate_handle_respond(ptr, true) };
        }
    }

    pub fn reject(mut self) {
        if let Some(ptr) = self.ptr.take() {
            unsafe { mpc_certificate_handle_respond(ptr, false) };
        }
    }
}

impl Drop for CertificateHandle {
    fn drop(&mut self) {
        if let Some(ptr) = self.ptr.take() {
            unsafe { mpc_certificate_handle_respond(ptr, false) };
        }
    }
}

unsafe impl Send for CertificateHandle {}

impl std::fmt::Debug for CertificateHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CertificateHandle")
            .field("pending", &self.ptr.is_some())
            .finish()
    }
}

/// An event emitted by an [`MCSession`](crate::session::Session) delegate.
#[non_exhaustive]
#[derive(Debug)]
pub enum SessionEvent {
    /// A connected peer changed state (connected / connecting / not-connected).
    StateChanged {
        /// The peer whose state changed.
        peer: PeerId,
        /// The new session state.
        state: SessionState,
    },
    /// Raw data was received from a peer.
    DataReceived {
        /// The sending peer.
        peer: PeerId,
        /// The received bytes.
        data: Vec<u8>,
    },
    /// An [`InputStream`] was received from a peer.
    StreamReceived {
        /// The sending peer.
        peer: PeerId,
        /// The stream name chosen by the sender.
        name: String,
        /// The incoming byte stream.
        stream: InputStream,
    },
    /// A resource transfer from a peer has started.
    ResourceStarted {
        /// The sending peer.
        peer: PeerId,
        /// The resource name.
        name: String,
        /// Progress tracker for the transfer.
        transfer: ResourceTransfer,
    },
    /// A resource transfer from a peer finished.
    ResourceFinished {
        /// The sending peer.
        peer: PeerId,
        /// The resource name.
        name: String,
        /// Local file URL where the resource was saved, if successful.
        local_url: Option<PathBuf>,
        /// Error if the transfer failed.
        error: Option<FrameworkError>,
    },
    /// Certificate data was received from a peer.
    ///
    /// The framework does not validate `items` in any way. Inspect them, then
    /// call [`CertificateHandle::accept`] to let the peer connect or
    /// [`CertificateHandle::reject`] to refuse it. Dropping the handle without
    /// a decision rejects the peer, as does dropping the event unread (for
    /// example when the stream is dropped or its buffer overflows). Peers
    /// without a security identity arrive with empty `items` and still need an
    /// explicit accept.
    CertificateReceived {
        /// The peer sending the certificate.
        peer: PeerId,
        /// The certificate items (typically `SecCertificate` objects).
        items: Vec<SecurityIdentityItem>,
        handle: CertificateHandle,
    },
}

unsafe extern "C" fn session_event_cb(kind: i32, payload: *const c_void, ctx: *mut c_void) {
    let Some(ctx) = NonNull::new(ctx.cast::<SenderContext<SessionEvent>>()) else {
        return;
    };
    catch_user_panic("session_event_cb", || {
        let sender = &unsafe { ctx.as_ref() }.sender;
        let event = match kind {
            0 => {
                let p = unsafe { &*payload.cast::<SessionStatePayload>() };
                Some(SessionEvent::StateChanged {
                    peer: unsafe { PeerId::from_owned_raw(p.peer) },
                    state: SessionState::from_raw(p.state),
                })
            }
            1 => {
                let p = unsafe { &*payload.cast::<SessionDataPayload>() };
                let data = if p.data.is_null() || p.len == 0 {
                    vec![]
                } else {
                    unsafe { std::slice::from_raw_parts(p.data.cast::<u8>(), p.len) }.to_vec()
                };
                Some(SessionEvent::DataReceived {
                    peer: unsafe { PeerId::from_owned_raw(p.peer) },
                    data,
                })
            }
            2 => {
                let p = unsafe { &*payload.cast::<SessionStreamPayload>() };
                let name = unsafe { CStr::from_ptr(p.name) }
                    .to_string_lossy()
                    .into_owned();
                Some(SessionEvent::StreamReceived {
                    peer: unsafe { PeerId::from_owned_raw(p.peer) },
                    name,
                    stream: unsafe { InputStream::from_owned_raw(p.stream) },
                })
            }
            3 => {
                let p = unsafe { &*payload.cast::<SessionResourceStartPayload>() };
                let name = unsafe { CStr::from_ptr(p.name) }
                    .to_string_lossy()
                    .into_owned();
                Some(SessionEvent::ResourceStarted {
                    peer: unsafe { PeerId::from_owned_raw(p.peer) },
                    name,
                    transfer: unsafe { ResourceTransfer::from_owned_raw(p.progress) },
                })
            }
            4 => {
                let p = unsafe { &*payload.cast::<SessionResourceFinishPayload>() };
                let name = unsafe { CStr::from_ptr(p.name) }
                    .to_string_lossy()
                    .into_owned();
                let local_url = if p.url_path.is_null() {
                    None
                } else {
                    Some(PathBuf::from(
                        unsafe { CStr::from_ptr(p.url_path) }
                            .to_string_lossy()
                            .as_ref(),
                    ))
                };
                let error = if p.error.is_null() {
                    None
                } else {
                    Some(take_framework_error(p.error))
                };
                Some(SessionEvent::ResourceFinished {
                    peer: unsafe { PeerId::from_owned_raw(p.peer) },
                    name,
                    local_url,
                    error,
                })
            }
            5 => {
                let p = unsafe { &*payload.cast::<SessionCertPayload>() };
                let handle = CertificateHandle {
                    ptr: (!p.handler.is_null()).then_some(p.handler),
                };
                let items = if p.items.is_null() || p.count == 0 {
                    vec![]
                } else {
                    unsafe { std::slice::from_raw_parts(p.items, p.count) }
                        .iter()
                        .map(|&raw| unsafe { SecurityIdentityItem::from_owned_raw(raw) })
                        .collect()
                };
                Some(SessionEvent::CertificateReceived {
                    peer: unsafe { PeerId::from_owned_raw(p.peer) },
                    items,
                    handle,
                })
            }
            _ => None,
        };
        if let Some(event) = event {
            sender.push(event);
        }
    });
}

/// Async stream of [`SessionEvent`]s produced by an
/// [`MCSession`](crate::session::Session) delegate.
///
/// Dropping the stream automatically uninstalls the delegate and closes the
/// event channel — no separate cleanup call is needed.
pub struct SessionEventStream {
    inner: BoundedAsyncStream<SessionEvent>,
    _handle: SubscriptionHandle<SessionEvent>,
}

impl SessionEventStream {
    /// Subscribe to session events with the given buffer `capacity`.
    ///
    /// When the buffer is full the **oldest** event is dropped to make room for
    /// the newest. Increase `capacity` to reduce drops under bursty loads.
    ///
    /// # Panics
    ///
    /// Panics if `capacity` is 0.
    #[must_use]
    pub fn subscribe(session: &Session, capacity: usize) -> Self {
        let (stream, sender) = BoundedAsyncStream::new(capacity);
        let ctx = SenderContext::new(sender);
        let handle = unsafe {
            mpc_session_stream_subscribe(
                session.as_ptr(),
                session_event_cb,
                ctx.cast(),
                session_ctx_retain,
                session_ctx_release,
            )
        };
        Self {
            inner: stream,
            _handle: SubscriptionHandle {
                bridge_handle: handle,
                ctx,
                unsubscribe_fn: mpc_session_stream_unsubscribe,
                ctx_release_fn: session_ctx_release,
            },
        }
    }

    /// Subscribe with the default buffer capacity (`DEFAULT_CAPACITY` = 64).
    #[must_use]
    pub fn subscribe_default(session: &Session) -> Self {
        Self::subscribe(session, DEFAULT_CAPACITY)
    }

    /// Await the next session event, returning `None` when the stream closes.
    #[must_use]
    pub const fn next(&self) -> NextItem<'_, SessionEvent> {
        self.inner.next()
    }

    /// Non-blocking poll; returns `None` if no event is currently buffered.
    #[must_use]
    pub fn try_next(&self) -> Option<SessionEvent> {
        self.inner.try_next()
    }

    /// Number of events currently waiting in the buffer.
    #[must_use]
    pub fn buffered_count(&self) -> usize {
        self.inner.buffered_count()
    }

    /// Returns `true` once the stream has been closed and drained.
    #[must_use]
    pub fn is_closed(&self) -> bool {
        self.inner.is_closed()
    }
}

#[repr(C)]
struct BrowserFoundPayload {
    peer: *mut c_void,
    discovery_json: *mut std::ffi::c_char,
}

#[repr(C)]
struct BrowserLostPayload {
    peer: *mut c_void,
}

#[repr(C)]
struct BrowserErrorPayload {
    error: *mut c_void,
}

/// An event emitted by an [`MCNearbyServiceBrowser`](crate::browser::NearbyServiceBrowser).
#[non_exhaustive]
#[derive(Debug)]
pub enum BrowserEvent {
    /// A peer was discovered.
    FoundPeer {
        /// The discovered peer.
        peer: PeerId,
        /// Discovery info advertised by the peer (if any).
        discovery_info: Option<HashMap<String, String>>,
    },
    /// A previously discovered peer is no longer reachable.
    LostPeer {
        /// The peer that was lost.
        peer: PeerId,
    },
    /// Browsing failed to start.
    BrowsingFailed(FrameworkError),
}

unsafe extern "C" fn browser_event_cb(kind: i32, payload: *const c_void, ctx: *mut c_void) {
    let Some(ctx) = NonNull::new(ctx.cast::<SenderContext<BrowserEvent>>()) else {
        return;
    };
    catch_user_panic("browser_event_cb", || {
        let sender = &unsafe { ctx.as_ref() }.sender;
        let event = match kind {
            0 => {
                let p = unsafe { &*payload.cast::<BrowserFoundPayload>() };
                let discovery_info = if p.discovery_json.is_null() {
                    None
                } else {
                    let json = unsafe { CStr::from_ptr(p.discovery_json) }.to_string_lossy();
                    serde_json::from_str::<HashMap<String, String>>(&json).ok()
                };
                Some(BrowserEvent::FoundPeer {
                    peer: unsafe { PeerId::from_owned_raw(p.peer) },
                    discovery_info,
                })
            }
            1 => {
                let p = unsafe { &*payload.cast::<BrowserLostPayload>() };
                Some(BrowserEvent::LostPeer {
                    peer: unsafe { PeerId::from_owned_raw(p.peer) },
                })
            }
            2 => {
                let p = unsafe { &*payload.cast::<BrowserErrorPayload>() };
                Some(BrowserEvent::BrowsingFailed(take_framework_error(p.error)))
            }
            _ => None,
        };
        if let Some(event) = event {
            sender.push(event);
        }
    });
}

/// Async stream of [`BrowserEvent`]s from an
/// [`MCNearbyServiceBrowser`](crate::browser::NearbyServiceBrowser).
///
/// Dropping the stream automatically uninstalls the delegate.
pub struct BrowserEventStream {
    inner: BoundedAsyncStream<BrowserEvent>,
    _handle: SubscriptionHandle<BrowserEvent>,
}

impl BrowserEventStream {
    /// Subscribe to browser events with the given buffer `capacity`.
    ///
    /// # Panics
    ///
    /// Panics if `capacity` is 0.
    #[must_use]
    pub fn subscribe(browser: &NearbyServiceBrowser, capacity: usize) -> Self {
        let (stream, sender) = BoundedAsyncStream::new(capacity);
        let ctx = SenderContext::new(sender);
        let handle = unsafe {
            mpc_browser_stream_subscribe(
                browser.as_ptr(),
                browser_event_cb,
                ctx.cast(),
                browser_ctx_retain,
                browser_ctx_release,
            )
        };
        Self {
            inner: stream,
            _handle: SubscriptionHandle {
                bridge_handle: handle,
                ctx,
                unsubscribe_fn: mpc_browser_stream_unsubscribe,
                ctx_release_fn: browser_ctx_release,
            },
        }
    }

    /// Subscribe with the default buffer capacity.
    #[must_use]
    pub fn subscribe_default(browser: &NearbyServiceBrowser) -> Self {
        Self::subscribe(browser, DEFAULT_CAPACITY)
    }

    /// Await the next browser event.
    #[must_use]
    pub const fn next(&self) -> NextItem<'_, BrowserEvent> {
        self.inner.next()
    }

    /// Non-blocking poll.
    #[must_use]
    pub fn try_next(&self) -> Option<BrowserEvent> {
        self.inner.try_next()
    }

    /// Number of events currently buffered.
    #[must_use]
    pub fn buffered_count(&self) -> usize {
        self.inner.buffered_count()
    }

    /// Returns `true` once the stream is closed.
    #[must_use]
    pub fn is_closed(&self) -> bool {
        self.inner.is_closed()
    }
}

#[repr(C)]
struct AdvertiserInvitationPayload {
    peer: *mut c_void,
    context_ptr: *const c_void,
    context_len: usize,
    invitation_handle: *mut c_void,
}

#[repr(C)]
struct AdvertiserErrorPayload {
    error: *mut c_void,
}

/// A handle to a pending invitation.
///
/// Call [`accept`](InvitationHandle::accept) or
/// [`decline`](InvitationHandle::decline) to respond. If the handle is dropped
/// without a response, the invitation is automatically declined.
#[derive(Default)]
pub struct InvitationHandle {
    ptr: Option<*mut c_void>,
}

impl InvitationHandle {
    /// Accept the invitation and join `session`.
    pub fn accept(mut self, session: &Session) {
        if let Some(ptr) = self.ptr.take() {
            unsafe { mpc_invitation_handle_accept(ptr, session.as_ptr()) };
        }
    }

    /// Decline the invitation.
    pub fn decline(mut self) {
        if let Some(ptr) = self.ptr.take() {
            unsafe { mpc_invitation_handle_decline(ptr) };
        }
    }
}

impl Drop for InvitationHandle {
    fn drop(&mut self) {
        if let Some(ptr) = self.ptr.take() {
            unsafe { mpc_invitation_handle_decline(ptr) };
        }
    }
}

// SAFETY: the Swift `MpcInvitationHandlerBox` is ARC-managed and consumed at most once.
unsafe impl Send for InvitationHandle {}

/// An event emitted by an [`MCNearbyServiceAdvertiser`](crate::advertiser::NearbyServiceAdvertiser).
#[non_exhaustive]
pub enum AdvertiserEvent {
    /// An invitation was received from a peer.
    ReceivedInvitation {
        /// The inviting peer.
        peer: PeerId,
        /// Optional context data sent with the invitation.
        context: Option<Vec<u8>>,
        /// Handle to accept or decline the invitation.
        handle: InvitationHandle,
    },
    /// Advertising failed to start.
    AdvertisingFailed(FrameworkError),
}

impl std::fmt::Debug for AdvertiserEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ReceivedInvitation { peer, context, .. } => f
                .debug_struct("ReceivedInvitation")
                .field("peer", peer)
                .field("context_len", &context.as_ref().map(Vec::len))
                .finish_non_exhaustive(),
            Self::AdvertisingFailed(error) => {
                f.debug_tuple("AdvertisingFailed").field(error).finish()
            }
        }
    }
}

unsafe extern "C" fn advertiser_event_cb(kind: i32, payload: *const c_void, ctx: *mut c_void) {
    let Some(ctx) = NonNull::new(ctx.cast::<SenderContext<AdvertiserEvent>>()) else {
        return;
    };
    catch_user_panic("advertiser_event_cb", || {
        let sender = &unsafe { ctx.as_ref() }.sender;
        let event = match kind {
            0 => {
                let p = unsafe { &*payload.cast::<AdvertiserInvitationPayload>() };
                let context = if p.context_ptr.is_null() || p.context_len == 0 {
                    None
                } else {
                    Some(
                        unsafe {
                            std::slice::from_raw_parts(p.context_ptr.cast::<u8>(), p.context_len)
                        }
                        .to_vec(),
                    )
                };
                Some(AdvertiserEvent::ReceivedInvitation {
                    peer: unsafe { PeerId::from_owned_raw(p.peer) },
                    context,
                    handle: InvitationHandle {
                        ptr: Some(p.invitation_handle),
                    },
                })
            }
            1 => {
                let p = unsafe { &*payload.cast::<AdvertiserErrorPayload>() };
                Some(AdvertiserEvent::AdvertisingFailed(take_framework_error(
                    p.error,
                )))
            }
            _ => None,
        };
        if let Some(event) = event {
            sender.push(event);
        }
    });
}

/// Async stream of [`AdvertiserEvent`]s from an
/// [`MCNearbyServiceAdvertiser`](crate::advertiser::NearbyServiceAdvertiser).
///
/// Dropping the stream automatically uninstalls the delegate. Any pending
/// [`InvitationHandle`]s are automatically declined on drop.
pub struct AdvertiserEventStream {
    inner: BoundedAsyncStream<AdvertiserEvent>,
    _handle: SubscriptionHandle<AdvertiserEvent>,
}

impl AdvertiserEventStream {
    /// Subscribe to advertiser events with the given buffer `capacity`.
    ///
    /// # Panics
    ///
    /// Panics if `capacity` is 0.
    #[must_use]
    pub fn subscribe(advertiser: &NearbyServiceAdvertiser, capacity: usize) -> Self {
        let (stream, sender) = BoundedAsyncStream::new(capacity);
        let ctx = SenderContext::new(sender);
        let handle = unsafe {
            mpc_advertiser_stream_subscribe(
                advertiser.as_ptr(),
                advertiser_event_cb,
                ctx.cast(),
                advertiser_ctx_retain,
                advertiser_ctx_release,
            )
        };
        Self {
            inner: stream,
            _handle: SubscriptionHandle {
                bridge_handle: handle,
                ctx,
                unsubscribe_fn: mpc_advertiser_stream_unsubscribe,
                ctx_release_fn: advertiser_ctx_release,
            },
        }
    }

    /// Subscribe with the default buffer capacity.
    #[must_use]
    pub fn subscribe_default(advertiser: &NearbyServiceAdvertiser) -> Self {
        Self::subscribe(advertiser, DEFAULT_CAPACITY)
    }

    /// Await the next advertiser event.
    #[must_use]
    pub const fn next(&self) -> NextItem<'_, AdvertiserEvent> {
        self.inner.next()
    }

    /// Non-blocking poll.
    #[must_use]
    pub fn try_next(&self) -> Option<AdvertiserEvent> {
        self.inner.try_next()
    }

    /// Number of events currently buffered.
    #[must_use]
    pub fn buffered_count(&self) -> usize {
        self.inner.buffered_count()
    }

    /// Returns `true` once the stream is closed.
    #[must_use]
    pub fn is_closed(&self) -> bool {
        self.inner.is_closed()
    }
}

#[cfg(test)]
mod tests {
    use core::ffi::c_void;
    use std::sync::atomic::{AtomicI32, Ordering};

    use super::{CertificateHandle, SessionEvent, SessionEventStream};
    use crate::peer::PeerId;
    use crate::session::{EncryptionPreference, Session};

    extern "C" {
        fn mpc_session_stream_deliver_certificate(
            handle: *mut c_void,
            peer: *mut c_void,
            include_item: bool,
            decision: unsafe extern "C" fn(*mut c_void, bool),
            decision_context: *mut c_void,
        );
    }

    const PENDING: i32 = -1;
    const REJECTED: i32 = 0;
    const ACCEPTED: i32 = 1;

    unsafe extern "C" fn record_decision(context: *mut c_void, accepted: bool) {
        let decision = unsafe { &*context.cast::<AtomicI32>() };
        decision.store(i32::from(accepted), Ordering::SeqCst);
    }

    #[allow(clippy::used_underscore_binding)]
    fn deliver(
        stream: &SessionEventStream,
        peer: &PeerId,
        include_item: bool,
        decision: &AtomicI32,
    ) {
        unsafe {
            mpc_session_stream_deliver_certificate(
                stream._handle.bridge_handle,
                peer.as_ptr(),
                include_item,
                record_decision,
                core::ptr::from_ref(decision).cast_mut().cast(),
            );
        }
    }

    fn session(name: &str) -> Session {
        let peer = PeerId::new(name).expect("peer");
        Session::new(&peer, EncryptionPreference::Required).expect("session")
    }

    fn take_certificate(stream: &SessionEventStream) -> (PeerId, usize, CertificateHandle) {
        match stream.try_next() {
            Some(SessionEvent::CertificateReceived {
                peer,
                items,
                handle,
            }) => (peer, items.len(), handle),
            other => panic!("expected a certificate event, got {other:?}"),
        }
    }

    #[test]
    fn accepting_a_certificate_reaches_the_framework_handler() {
        let decision = AtomicI32::new(PENDING);
        let remote = PeerId::new("cert-remote").expect("peer");
        let session = session("cert-accept");
        let stream = SessionEventStream::subscribe_default(&session);

        deliver(&stream, &remote, true, &decision);
        let (peer, item_count, handle) = take_certificate(&stream);
        assert_eq!(peer.display_name(), "cert-remote");
        assert_eq!(item_count, 1);
        assert_eq!(decision.load(Ordering::SeqCst), PENDING);

        handle.accept();
        assert_eq!(decision.load(Ordering::SeqCst), ACCEPTED);
    }

    #[test]
    fn rejecting_a_certificate_reaches_the_framework_handler() {
        let decision = AtomicI32::new(PENDING);
        let remote = PeerId::new("cert-remote").expect("peer");
        let session = session("cert-reject");
        let stream = SessionEventStream::subscribe_default(&session);

        deliver(&stream, &remote, false, &decision);
        let (_, item_count, handle) = take_certificate(&stream);
        assert_eq!(item_count, 0);
        assert_eq!(decision.load(Ordering::SeqCst), PENDING);

        handle.reject();
        assert_eq!(decision.load(Ordering::SeqCst), REJECTED);
    }

    #[test]
    fn dropping_an_undecided_certificate_rejects_the_peer() {
        let decision = AtomicI32::new(PENDING);
        let remote = PeerId::new("cert-remote").expect("peer");
        let session = session("cert-drop");
        let stream = SessionEventStream::subscribe_default(&session);

        deliver(&stream, &remote, false, &decision);
        let event = stream.try_next().expect("certificate event");
        assert_eq!(decision.load(Ordering::SeqCst), PENDING);

        drop(event);
        assert_eq!(decision.load(Ordering::SeqCst), REJECTED);
    }

    #[test]
    fn dropping_the_stream_rejects_buffered_certificates() {
        let decision = AtomicI32::new(PENDING);
        let remote = PeerId::new("cert-remote").expect("peer");
        let session = session("cert-stream-drop");
        let stream = SessionEventStream::subscribe_default(&session);

        deliver(&stream, &remote, true, &decision);
        assert_eq!(stream.buffered_count(), 1);
        assert_eq!(decision.load(Ordering::SeqCst), PENDING);

        drop(stream);
        assert_eq!(decision.load(Ordering::SeqCst), REJECTED);
    }

    #[test]
    fn a_certificate_evicted_from_a_full_buffer_is_rejected() {
        let first = AtomicI32::new(PENDING);
        let second = AtomicI32::new(PENDING);
        let remote = PeerId::new("cert-remote").expect("peer");
        let session = session("cert-overflow");
        let stream = SessionEventStream::subscribe(&session, 1);

        deliver(&stream, &remote, false, &first);
        deliver(&stream, &remote, false, &second);
        assert_eq!(first.load(Ordering::SeqCst), REJECTED);
        assert_eq!(second.load(Ordering::SeqCst), PENDING);

        let (_, _, handle) = take_certificate(&stream);
        handle.accept();
        assert_eq!(second.load(Ordering::SeqCst), ACCEPTED);
        assert_eq!(first.load(Ordering::SeqCst), REJECTED);
    }

    #[test]
    fn an_empty_certificate_handle_is_inert() {
        let handle = CertificateHandle::default();
        assert_eq!(
            format!("{handle:?}"),
            "CertificateHandle { pending: false }"
        );
        handle.accept();
        CertificateHandle::default().reject();
        drop(CertificateHandle::default());
    }
}
