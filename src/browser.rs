use core::ffi::c_void;
use core::ptr::{self, NonNull};
use std::collections::HashMap;
use std::ffi::{CStr, CString};
use std::sync::Mutex;

use crate::error::{MultipeerError, Result};
use crate::ffi;
use crate::peer::{last_error, PeerId};
use crate::session::Session;

type FoundPeerHandler = dyn FnMut(PeerId, Option<HashMap<String, String>>) + Send;
type LostPeerHandler = dyn FnMut(PeerId) + Send;

struct BrowserDelegateState {
    on_found: Mutex<Box<FoundPeerHandler>>,
    on_lost: Mutex<Box<LostPeerHandler>>,
}

pub struct NearbyServiceBrowser {
    raw: NonNull<c_void>,
    delegate_state: Option<NonNull<BrowserDelegateState>>,
}

impl NearbyServiceBrowser {
    /// Create a browser for a service type.
    ///
    /// # Errors
    ///
    /// Returns an error if the service type contains an embedded NUL byte or the browser cannot be created.
    pub fn new(peer: &PeerId, service_type: impl AsRef<str>) -> Result<Self> {
        let service_type = CString::new(service_type.as_ref()).map_err(|_| {
            MultipeerError::InvalidArgument("service type must not contain NUL bytes".into())
        })?;
        let mut error = ptr::null_mut();
        let raw = unsafe { ffi::mpc_browser_create(peer.as_ptr(), service_type.as_ptr(), &mut error) };
        let raw = NonNull::new(raw).ok_or_else(|| last_error(error))?;
        Ok(Self {
            raw,
            delegate_state: None,
        })
    }

    pub fn start_browsing_for_peers(&self) {
        unsafe { ffi::mpc_browser_start(self.raw.as_ptr()) };
    }

    pub fn stop_browsing_for_peers(&self) {
        unsafe { ffi::mpc_browser_stop(self.raw.as_ptr()) };
    }

    pub fn invite_peer(
        &self,
        peer: &PeerId,
        session: &Session,
        context: Option<&[u8]>,
        timeout_seconds: f64,
    ) {
        let (context_ptr, context_len) =
            context.map_or((ptr::null(), 0), |bytes| (bytes.as_ptr().cast::<c_void>(), bytes.len()));
        unsafe {
            ffi::mpc_browser_invite_peer(
                self.raw.as_ptr(),
                peer.as_ptr(),
                session.as_ptr(),
                context_ptr,
                context_len,
                timeout_seconds,
            );
        }
    }

    pub fn set_delegate<F, G>(&mut self, on_found: F, on_lost: G)
    where
        F: FnMut(PeerId, Option<HashMap<String, String>>) + Send + 'static,
        G: FnMut(PeerId) + Send + 'static,
    {
        self.clear_delegate();
        let state = Box::new(BrowserDelegateState {
            on_found: Mutex::new(Box::new(on_found)),
            on_lost: Mutex::new(Box::new(on_lost)),
        });
        let ptr = NonNull::from(Box::leak(state));
        unsafe {
            ffi::mpc_browser_set_delegate(
                self.raw.as_ptr(),
                ptr.as_ptr().cast::<c_void>(),
                browser_found_trampoline,
                browser_lost_trampoline,
            );
        }
        self.delegate_state = Some(ptr);
    }

    pub fn clear_delegate(&mut self) {
        if let Some(state) = self.delegate_state.take() {
            unsafe {
                ffi::mpc_browser_clear_delegate(self.raw.as_ptr());
                drop(Box::from_raw(state.as_ptr()));
            }
        }
    }
}

impl Drop for NearbyServiceBrowser {
    fn drop(&mut self) {
        self.clear_delegate();
        unsafe { ffi::mpc_object_release(self.raw.as_ptr()) };
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
    let peer = PeerId::from_owned_raw(peer);
    let discovery = if discovery_json.is_null() {
        None
    } else {
        let json = unsafe { CStr::from_ptr(discovery_json) }.to_string_lossy();
        serde_json::from_str::<HashMap<String, String>>(&json).ok()
    };
    if let Ok(mut callback) = context.as_ref().on_found.lock() {
        callback(peer, discovery);
    }
}

unsafe extern "C" fn browser_lost_trampoline(context: *mut c_void, peer: *mut c_void) {
    let Some(context) = NonNull::new(context.cast::<BrowserDelegateState>()) else {
        return;
    };
    let peer = PeerId::from_owned_raw(peer);
    if let Ok(mut callback) = context.as_ref().on_lost.lock() {
        callback(peer);
    }
}
