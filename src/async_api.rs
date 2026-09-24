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
//! multipeerconnectivity = { package = "multipeerconnectivity-rs", version = "0.5", features = ["async"] }
//! ```
//!
//! # Quick start
//!
//! ```no_run
//! use multipeerconnectivity::async_api::SessionEventStream;
//! use multipeerconnectivity::{
//!     CertificatePolicy, EncryptionPreference, PeerId, SecurityIdentityItem, Session,
//! };
//!
//! # fn is_trusted(_peer: &PeerId, _items: &[SecurityIdentityItem]) -> bool { false }
//! # async fn run() -> multipeerconnectivity::Result<()> {
//! let peer = PeerId::new("my-peer")?;
//! let session = Session::new(
//!     &peer,
//!     EncryptionPreference::Required,
//!     CertificatePolicy::Verify(Box::new(|request| {
//!         if is_trusted(request.peer(), request.items()) {
//!             request.accept();
//!         } else {
//!             request.reject();
//!         }
//!     })),
//! )?;
//! let stream = SessionEventStream::subscribe_default(&session);
//!
//! while let Some(event) = stream.next().await {
//!     println!("session event: {event:?}");
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
use crate::session::{InputStream, ResourceTransfer, Session, SessionState};

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

    use super::{AdvertiserEventStream, BrowserEventStream, SessionEventStream};
    use crate::advertiser::{NearbyServiceAdvertiser, NearbyServiceAdvertiserDelegate};
    use crate::browser::{NearbyServiceBrowser, NearbyServiceBrowserDelegate};
    use crate::ffi;
    use crate::peer::PeerId;
    use crate::session::test_support::{
        delegate_of, deliver_certificate, rejecting_session, session_with, ACCEPTED, PENDING,
        REJECTED,
    };
    use crate::session::{CertificatePolicy, Session, SessionDelegate};

    #[allow(clippy::used_underscore_binding)]
    fn session_bridge(stream: &SessionEventStream) -> *mut c_void {
        stream._handle.bridge_handle
    }

    #[allow(clippy::used_underscore_binding)]
    fn browser_bridge(stream: &BrowserEventStream) -> *mut c_void {
        stream._handle.bridge_handle
    }

    #[allow(clippy::used_underscore_binding)]
    fn advertiser_bridge(stream: &AdvertiserEventStream) -> *mut c_void {
        stream._handle.bridge_handle
    }

    fn remote() -> PeerId {
        PeerId::new("cert-remote").expect("peer")
    }

    #[test]
    fn a_stream_does_not_bypass_the_certificate_policy() {
        let rejected = AtomicI32::new(PENDING);
        let accepted = AtomicI32::new(PENDING);
        let strict = rejecting_session("stream-policy-strict");
        let open = session_with("stream-policy-open", CertificatePolicy::AcceptAll);
        let strict_stream = SessionEventStream::subscribe_default(&strict);
        let open_stream = SessionEventStream::subscribe_default(&open);
        assert_eq!(delegate_of(strict.as_ptr()), session_bridge(&strict_stream));
        assert_eq!(delegate_of(open.as_ptr()), session_bridge(&open_stream));

        deliver_certificate(&strict, &remote(), true, &rejected);
        deliver_certificate(&open, &remote(), true, &accepted);
        assert_eq!(rejected.load(Ordering::SeqCst), REJECTED);
        assert_eq!(accepted.load(Ordering::SeqCst), ACCEPTED);
        assert_eq!(strict_stream.buffered_count(), 0);
    }

    #[test]
    fn dropping_a_session_stream_restores_the_registered_delegate() {
        let mut session = rejecting_session("delegate-restore");
        let policy = delegate_of(session.as_ptr());
        assert!(!policy.is_null());
        session.set_callbacks(SessionDelegate::new());
        let sync_delegate = delegate_of(session.as_ptr());
        assert_ne!(sync_delegate, policy);

        let stream = SessionEventStream::subscribe_default(&session);
        assert_eq!(delegate_of(session.as_ptr()), session_bridge(&stream));
        drop(stream);
        assert_eq!(delegate_of(session.as_ptr()), sync_delegate);

        session.clear_delegate();
        assert_eq!(delegate_of(session.as_ptr()), policy);
    }

    #[test]
    fn dropping_a_session_stream_keeps_a_delegate_installed_later() {
        let mut session = rejecting_session("delegate-later");
        let stream = SessionEventStream::subscribe_default(&session);
        session.set_callbacks(SessionDelegate::new());
        let sync_delegate = delegate_of(session.as_ptr());
        assert_ne!(sync_delegate, session_bridge(&stream));

        drop(stream);
        assert_eq!(delegate_of(session.as_ptr()), sync_delegate);
    }

    #[test]
    fn clearing_the_session_delegate_keeps_a_stream_installed_later() {
        let mut session = rejecting_session("delegate-clear");
        let policy = delegate_of(session.as_ptr());
        session.set_callbacks(SessionDelegate::new());
        let stream = SessionEventStream::subscribe_default(&session);

        session.clear_delegate();
        assert_eq!(delegate_of(session.as_ptr()), session_bridge(&stream));

        drop(stream);
        assert_eq!(delegate_of(session.as_ptr()), policy);
    }

    #[test]
    fn nested_session_streams_unwind_in_either_order() {
        let session = rejecting_session("delegate-nested");
        let policy = delegate_of(session.as_ptr());

        let outer = SessionEventStream::subscribe_default(&session);
        let inner = SessionEventStream::subscribe_default(&session);
        let outer_bridge = session_bridge(&outer);
        assert_eq!(delegate_of(session.as_ptr()), session_bridge(&inner));
        drop(inner);
        assert_eq!(delegate_of(session.as_ptr()), outer_bridge);
        drop(outer);
        assert_eq!(delegate_of(session.as_ptr()), policy);

        let outer = SessionEventStream::subscribe_default(&session);
        let inner = SessionEventStream::subscribe_default(&session);
        let inner_bridge = session_bridge(&inner);
        drop(outer);
        assert_eq!(delegate_of(session.as_ptr()), inner_bridge);
        drop(inner);
        assert_eq!(delegate_of(session.as_ptr()), policy);
    }

    #[test]
    fn dropping_one_handle_keeps_the_delegate_another_handle_installed() {
        let decision = AtomicI32::new(PENDING);
        let mut original = rejecting_session("delegate-alias");
        let policy = delegate_of(original.as_ptr());
        original.set_callbacks(SessionDelegate::new());
        let mut alias =
            unsafe { Session::from_owned_raw(ffi::core::mpc_object_retain(original.as_ptr())) };
        alias.set_callbacks(SessionDelegate::new());
        let alias_delegate = delegate_of(alias.as_ptr());
        assert_ne!(alias_delegate, policy);

        drop(original);
        assert_eq!(delegate_of(alias.as_ptr()), alias_delegate);

        alias.clear_delegate();
        assert_eq!(delegate_of(alias.as_ptr()), policy);
        deliver_certificate(&alias, &remote(), true, &decision);
        assert_eq!(decision.load(Ordering::SeqCst), REJECTED);
    }

    #[test]
    fn browser_streams_restore_and_keep_later_delegates() {
        let peer = PeerId::new("delegate-browser").expect("peer");
        let mut browser = NearbyServiceBrowser::new(&peer, "doom-test").expect("browser");
        browser.set_callbacks(NearbyServiceBrowserDelegate::new());
        let sync_delegate = delegate_of(browser.as_ptr());
        assert!(!sync_delegate.is_null());

        let stream = BrowserEventStream::subscribe_default(&browser);
        assert_eq!(delegate_of(browser.as_ptr()), browser_bridge(&stream));
        drop(stream);
        assert_eq!(delegate_of(browser.as_ptr()), sync_delegate);

        let stream = BrowserEventStream::subscribe_default(&browser);
        browser.clear_delegate();
        assert_eq!(delegate_of(browser.as_ptr()), browser_bridge(&stream));
        browser.set_callbacks(NearbyServiceBrowserDelegate::new());
        let later_delegate = delegate_of(browser.as_ptr());
        assert_ne!(later_delegate, browser_bridge(&stream));
        drop(stream);
        assert_eq!(delegate_of(browser.as_ptr()), later_delegate);
    }

    #[test]
    fn advertiser_streams_restore_and_keep_later_delegates() {
        let peer = PeerId::new("delegate-advertiser").expect("peer");
        let mut advertiser =
            NearbyServiceAdvertiser::new(&peer, None, "doom-test").expect("advertiser");
        advertiser.set_callbacks(NearbyServiceAdvertiserDelegate::new());
        let sync_delegate = delegate_of(advertiser.as_ptr());
        assert!(!sync_delegate.is_null());

        let stream = AdvertiserEventStream::subscribe_default(&advertiser);
        assert_eq!(delegate_of(advertiser.as_ptr()), advertiser_bridge(&stream));
        drop(stream);
        assert_eq!(delegate_of(advertiser.as_ptr()), sync_delegate);

        let stream = AdvertiserEventStream::subscribe_default(&advertiser);
        advertiser.clear_delegate();
        assert_eq!(delegate_of(advertiser.as_ptr()), advertiser_bridge(&stream));
        advertiser.set_callbacks(NearbyServiceAdvertiserDelegate::new());
        let later_delegate = delegate_of(advertiser.as_ptr());
        assert_ne!(later_delegate, advertiser_bridge(&stream));
        drop(stream);
        assert_eq!(delegate_of(advertiser.as_ptr()), later_delegate);
    }
}
