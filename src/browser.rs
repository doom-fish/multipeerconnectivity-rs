#![allow(clippy::missing_errors_doc, clippy::struct_field_names)]

use core::ffi::c_void;
use core::ptr::{self, NonNull};
use std::collections::HashMap;
use std::ffi::{CStr, CString};
use std::fmt;
use std::sync::Mutex;

use doom_fish_utils::panic_safe::catch_user_panic;

use crate::error::{
    copy_and_free_string, take_framework_error, FrameworkError, MultipeerError, Result,
};
use crate::ffi;
use crate::peer::PeerId;
use crate::session::Session;

type FoundPeerHandler = dyn FnMut(PeerId, Option<HashMap<String, String>>) + Send;
type LostPeerHandler = dyn FnMut(PeerId) + Send;
type BrowserErrorHandler = dyn FnMut(FrameworkError) + Send;

fn validate_service_type(service_type: &str) -> Result<CString> {
    if service_type.is_empty() {
        return Err(MultipeerError::InvalidArgument(
            "service type must not be empty".into(),
        ));
    }
    if service_type.len() > 15 {
        return Err(MultipeerError::InvalidArgument(
            "service type must be at most 15 ASCII characters".into(),
        ));
    }
    if !service_type
        .bytes()
        .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    {
        return Err(MultipeerError::InvalidArgument(
            "service type must contain only lowercase ASCII letters, digits, or hyphens".into(),
        ));
    }
    CString::new(service_type).map_err(|_| {
        MultipeerError::InvalidArgument("service type must not contain NUL bytes".into())
    })
}

/// Configures `MultipeerConnectivity` browser delegate callbacks.
pub struct NearbyServiceBrowserDelegate {
    on_found: Option<Box<FoundPeerHandler>>,
    on_lost: Option<Box<LostPeerHandler>>,
    on_error: Option<Box<BrowserErrorHandler>>,
}

impl NearbyServiceBrowserDelegate {
    #[must_use]
    /// Creates an empty `MultipeerConnectivity` browser delegate.
    pub const fn new() -> Self {
        Self {
            on_found: None,
            on_lost: None,
            on_error: None,
        }
    }

    #[must_use]
    /// Registers a callback for discovered `MultipeerConnectivity` peers.
    pub fn on_found<F>(mut self, handler: F) -> Self
    where
        F: FnMut(PeerId, Option<HashMap<String, String>>) + Send + 'static,
    {
        self.on_found = Some(Box::new(handler));
        self
    }

    #[must_use]
    /// Registers a callback for lost `MultipeerConnectivity` peers.
    pub fn on_lost<F>(mut self, handler: F) -> Self
    where
        F: FnMut(PeerId) + Send + 'static,
    {
        self.on_lost = Some(Box::new(handler));
        self
    }

    #[must_use]
    /// Registers a `MultipeerConnectivity` browser error callback.
    pub fn on_error<F>(mut self, handler: F) -> Self
    where
        F: FnMut(FrameworkError) + Send + 'static,
    {
        self.on_error = Some(Box::new(handler));
        self
    }
}

impl Default for NearbyServiceBrowserDelegate {
    fn default() -> Self {
        Self::new()
    }
}

struct BrowserDelegateState {
    callbacks: Mutex<NearbyServiceBrowserDelegate>,
}

/// Wraps a `MultipeerConnectivity` `MCNearbyServiceBrowser`.
pub struct NearbyServiceBrowser {
    raw: NonNull<c_void>,
    delegate_state: Option<NonNull<BrowserDelegateState>>,
}

impl NearbyServiceBrowser {
    /// Creates a `MultipeerConnectivity` browser for the local peer.
    pub fn new(peer: &PeerId, service_type: impl AsRef<str>) -> Result<Self> {
        let service_type = validate_service_type(service_type.as_ref())?;
        let raw = unsafe { ffi::browser::mpc_browser_create(peer.as_ptr(), service_type.as_ptr()) };
        let raw = NonNull::new(raw).ok_or_else(|| {
            MultipeerError::OperationFailed("failed to create MCNearbyServiceBrowser".into())
        })?;
        Ok(Self {
            raw,
            delegate_state: None,
        })
    }

    pub(crate) unsafe fn from_owned_raw(raw: *mut c_void) -> Self {
        Self {
            raw: NonNull::new(raw).expect("browser raw pointer must not be null"),
            delegate_state: None,
        }
    }

    #[must_use]
    /// Returns the local `MultipeerConnectivity` peer identifier.
    pub fn my_peer_id(&self) -> PeerId {
        let raw = unsafe { ffi::browser::mpc_browser_copy_my_peer(self.raw.as_ptr()) };
        unsafe { PeerId::from_owned_raw(raw) }
    }

    #[must_use]
    /// Returns the `MultipeerConnectivity` service type.
    pub fn service_type(&self) -> String {
        let string = unsafe { ffi::browser::mpc_browser_service_type(self.raw.as_ptr()) };
        copy_and_free_string(string)
    }

    /// Starts browsing for `MultipeerConnectivity` peers.
    pub fn start_browsing_for_peers(&self) {
        unsafe { ffi::browser::mpc_browser_start(self.raw.as_ptr()) };
    }

    /// Stops browsing for `MultipeerConnectivity` peers.
    pub fn stop_browsing_for_peers(&self) {
        unsafe { ffi::browser::mpc_browser_stop(self.raw.as_ptr()) };
    }

    /// Invites a peer through the `MultipeerConnectivity` browser.
    pub fn invite_peer(
        &self,
        peer: &PeerId,
        session: &Session,
        context: Option<&[u8]>,
        timeout_seconds: f64,
    ) {
        let (context_ptr, context_len) = context.map_or((ptr::null(), 0), |bytes| {
            (bytes.as_ptr().cast::<c_void>(), bytes.len())
        });
        unsafe {
            ffi::browser::mpc_browser_invite_peer(
                self.raw.as_ptr(),
                peer.as_ptr(),
                session.as_ptr(),
                context_ptr,
                context_len,
                timeout_seconds,
            );
        }
    }

    /// Installs basic `MultipeerConnectivity` browser callbacks.
    pub fn set_delegate<F, G>(&mut self, on_found: F, on_lost: G)
    where
        F: FnMut(PeerId, Option<HashMap<String, String>>) + Send + 'static,
        G: FnMut(PeerId) + Send + 'static,
    {
        self.set_callbacks(
            NearbyServiceBrowserDelegate::new()
                .on_found(on_found)
                .on_lost(on_lost),
        );
    }

    /// Installs typed `MultipeerConnectivity` browser callbacks.
    pub fn set_callbacks(&mut self, callbacks: NearbyServiceBrowserDelegate) {
        self.clear_delegate();
        let has_error = callbacks.on_error.is_some();
        let state = Box::new(BrowserDelegateState {
            callbacks: Mutex::new(callbacks),
        });
        let ptr = NonNull::from(Box::leak(state));
        unsafe {
            ffi::browser::mpc_browser_set_delegate(
                self.raw.as_ptr(),
                ptr.as_ptr().cast::<c_void>(),
                Some(browser_found_trampoline),
                Some(browser_lost_trampoline),
                if has_error {
                    Some(browser_error_trampoline)
                } else {
                    None
                },
            );
        }
        self.delegate_state = Some(ptr);
    }

    /// Removes the `MultipeerConnectivity` browser delegate.
    pub fn clear_delegate(&mut self) {
        if let Some(state) = self.delegate_state.take() {
            unsafe {
                ffi::browser::mpc_browser_clear_delegate(self.raw.as_ptr());
                drop(Box::from_raw(state.as_ptr()));
            }
        }
    }

    pub(crate) const fn as_ptr(&self) -> *mut c_void {
        self.raw.as_ptr()
    }
}

impl Clone for NearbyServiceBrowser {
    fn clone(&self) -> Self {
        let raw = unsafe { ffi::core::mpc_object_retain(self.raw.as_ptr()) };
        unsafe { Self::from_owned_raw(raw) }
    }
}

impl Drop for NearbyServiceBrowser {
    fn drop(&mut self) {
        self.clear_delegate();
        unsafe { ffi::core::mpc_object_release(self.raw.as_ptr()) };
    }
}

impl fmt::Debug for NearbyServiceBrowser {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NearbyServiceBrowser")
            .field("my_peer_id", &self.my_peer_id())
            .field("service_type", &self.service_type())
            .finish()
    }
}

unsafe extern "C" fn browser_found_trampoline(
    context: *mut c_void,
    peer: *mut c_void,
    discovery_json: *const std::ffi::c_char,
) {
    let Some(context) = NonNull::new(context.cast::<BrowserDelegateState>()) else {
        return;
    };
    let peer = unsafe { PeerId::from_owned_raw(peer) };
    let discovery = if discovery_json.is_null() {
        None
    } else {
        let json = unsafe { CStr::from_ptr(discovery_json) }.to_string_lossy();
        serde_json::from_str::<HashMap<String, String>>(&json).ok()
    };
    if let Ok(mut callbacks) = unsafe { context.as_ref() }.callbacks.lock() {
        if let Some(callback) = callbacks.on_found.as_mut() {
            catch_user_panic("browser_found_trampoline", || callback(peer, discovery));
        }
    }
}

unsafe extern "C" fn browser_lost_trampoline(context: *mut c_void, peer: *mut c_void) {
    let Some(context) = NonNull::new(context.cast::<BrowserDelegateState>()) else {
        return;
    };
    let peer = unsafe { PeerId::from_owned_raw(peer) };
    if let Ok(mut callbacks) = unsafe { context.as_ref() }.callbacks.lock() {
        if let Some(callback) = callbacks.on_lost.as_mut() {
            catch_user_panic("browser_lost_trampoline", || callback(peer));
        }
    }
}

unsafe extern "C" fn browser_error_trampoline(context: *mut c_void, error: *mut c_void) {
    let Some(context) = NonNull::new(context.cast::<BrowserDelegateState>()) else {
        if !error.is_null() {
            let _ = take_framework_error(error);
        }
        return;
    };
    let error = take_framework_error(error);
    if let Ok(mut callbacks) = unsafe { context.as_ref() }.callbacks.lock() {
        if let Some(callback) = callbacks.on_error.as_mut() {
            catch_user_panic("browser_error_trampoline", || callback(error));
        }
    }
}
