#![allow(
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::must_use_candidate,
    clippy::struct_field_names
)]

use core::ffi::c_void;
use core::ptr::NonNull;
use std::collections::HashMap;
use std::ffi::CString;
use std::fmt;
use std::sync::Mutex;

use doom_fish_utils::panic_safe::catch_user_panic;

use crate::browser::NearbyServiceBrowser;
use crate::error::{MultipeerError, Result};
use crate::ffi;
use crate::peer::PeerId;
use crate::session::Session;

type FinishHandler = dyn FnMut() + Send;
type CancelHandler = dyn FnMut() + Send;
type ShouldPresentHandler = dyn FnMut(PeerId, Option<HashMap<String, String>>) -> bool + Send;

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

pub struct BrowserViewControllerDelegate {
    on_finish: Option<Box<FinishHandler>>,
    on_cancel: Option<Box<CancelHandler>>,
    should_present_peer: Option<Box<ShouldPresentHandler>>,
}

impl BrowserViewControllerDelegate {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            on_finish: None,
            on_cancel: None,
            should_present_peer: None,
        }
    }

    #[must_use]
    pub fn on_finish<F>(mut self, handler: F) -> Self
    where
        F: FnMut() + Send + 'static,
    {
        self.on_finish = Some(Box::new(handler));
        self
    }

    #[must_use]
    pub fn on_cancel<F>(mut self, handler: F) -> Self
    where
        F: FnMut() + Send + 'static,
    {
        self.on_cancel = Some(Box::new(handler));
        self
    }

    #[must_use]
    pub fn should_present_peer<F>(mut self, handler: F) -> Self
    where
        F: FnMut(PeerId, Option<HashMap<String, String>>) -> bool + Send + 'static,
    {
        self.should_present_peer = Some(Box::new(handler));
        self
    }
}

impl Default for BrowserViewControllerDelegate {
    fn default() -> Self {
        Self::new()
    }
}

struct BrowserViewControllerDelegateState {
    callbacks: Mutex<BrowserViewControllerDelegate>,
}

pub struct BrowserViewController {
    raw: NonNull<c_void>,
    delegate_state: Option<NonNull<BrowserViewControllerDelegateState>>,
}

impl BrowserViewController {
    pub fn new_with_service_type(service_type: impl AsRef<str>, session: &Session) -> Result<Self> {
        let service_type = validate_service_type(service_type.as_ref())?;
        let raw = unsafe {
            ffi::browser_view_controller::mpc_browser_view_controller_create_with_service_type(
                service_type.as_ptr(),
                session.as_ptr(),
            )
        };
        let raw = NonNull::new(raw).ok_or_else(|| {
            MultipeerError::OperationFailed("failed to create MCBrowserViewController".into())
        })?;
        Ok(Self {
            raw,
            delegate_state: None,
        })
    }

    pub fn new_with_browser(browser: &NearbyServiceBrowser, session: &Session) -> Self {
        let raw = unsafe {
            ffi::browser_view_controller::mpc_browser_view_controller_create_with_browser(
                browser.as_ptr(),
                session.as_ptr(),
            )
        };
        let raw = NonNull::new(raw).expect("browser view controller raw pointer must not be null");
        Self {
            raw,
            delegate_state: None,
        }
    }

    pub(crate) unsafe fn from_owned_raw(raw: *mut c_void) -> Self {
        Self {
            raw: NonNull::new(raw).expect("browser view controller raw pointer must not be null"),
            delegate_state: None,
        }
    }

    #[must_use]
    pub fn browser(&self) -> NearbyServiceBrowser {
        let raw = unsafe {
            ffi::browser_view_controller::mpc_browser_view_controller_copy_browser(
                self.raw.as_ptr(),
            )
        };
        unsafe { NearbyServiceBrowser::from_owned_raw(raw) }
    }

    #[must_use]
    pub fn session(&self) -> Session {
        let raw = unsafe {
            ffi::browser_view_controller::mpc_browser_view_controller_copy_session(
                self.raw.as_ptr(),
            )
        };
        unsafe { Session::from_owned_raw(raw) }
    }

    #[must_use]
    pub fn minimum_number_of_peers(&self) -> usize {
        unsafe {
            ffi::browser_view_controller::mpc_browser_view_controller_minimum_number_of_peers(
                self.raw.as_ptr(),
            )
        }
    }

    pub fn set_minimum_number_of_peers(&self, value: usize) {
        unsafe {
            ffi::browser_view_controller::mpc_browser_view_controller_set_minimum_number_of_peers(
                self.raw.as_ptr(),
                value,
            );
        };
    }

    #[must_use]
    pub fn maximum_number_of_peers(&self) -> usize {
        unsafe {
            ffi::browser_view_controller::mpc_browser_view_controller_maximum_number_of_peers(
                self.raw.as_ptr(),
            )
        }
    }

    pub fn set_maximum_number_of_peers(&self, value: usize) {
        unsafe {
            ffi::browser_view_controller::mpc_browser_view_controller_set_maximum_number_of_peers(
                self.raw.as_ptr(),
                value,
            );
        };
    }

    pub fn set_callbacks(&mut self, callbacks: BrowserViewControllerDelegate) {
        self.clear_delegate();
        let has_should_present = callbacks.should_present_peer.is_some();
        let state = Box::new(BrowserViewControllerDelegateState {
            callbacks: Mutex::new(callbacks),
        });
        let ptr = NonNull::from(Box::leak(state));
        unsafe {
            ffi::browser_view_controller::mpc_browser_view_controller_set_delegate(
                self.raw.as_ptr(),
                ptr.as_ptr().cast::<c_void>(),
                Some(browser_view_controller_finish_trampoline),
                Some(browser_view_controller_cancel_trampoline),
                if has_should_present {
                    Some(browser_view_controller_should_present_trampoline)
                } else {
                    None
                },
            );
        }
        self.delegate_state = Some(ptr);
    }

    pub fn clear_delegate(&mut self) {
        if let Some(state) = self.delegate_state.take() {
            unsafe {
                ffi::browser_view_controller::mpc_browser_view_controller_clear_delegate(
                    self.raw.as_ptr(),
                );
                drop(Box::from_raw(state.as_ptr()));
            }
        }
    }
}

impl Clone for BrowserViewController {
    fn clone(&self) -> Self {
        let raw = unsafe { ffi::core::mpc_object_retain(self.raw.as_ptr()) };
        unsafe { Self::from_owned_raw(raw) }
    }
}

impl Drop for BrowserViewController {
    fn drop(&mut self) {
        self.clear_delegate();
        unsafe { ffi::core::mpc_object_release(self.raw.as_ptr()) };
    }
}

impl fmt::Debug for BrowserViewController {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BrowserViewController")
            .field("minimum_number_of_peers", &self.minimum_number_of_peers())
            .field("maximum_number_of_peers", &self.maximum_number_of_peers())
            .finish()
    }
}

unsafe extern "C" fn browser_view_controller_finish_trampoline(context: *mut c_void) {
    let Some(context) = NonNull::new(context.cast::<BrowserViewControllerDelegateState>()) else {
        return;
    };
    if let Ok(mut callbacks) = unsafe { context.as_ref() }.callbacks.lock() {
        if let Some(callback) = callbacks.on_finish.as_mut() {
            catch_user_panic("browser_view_controller_finish_trampoline", callback);
        }
    }
}

unsafe extern "C" fn browser_view_controller_cancel_trampoline(context: *mut c_void) {
    let Some(context) = NonNull::new(context.cast::<BrowserViewControllerDelegateState>()) else {
        return;
    };
    if let Ok(mut callbacks) = unsafe { context.as_ref() }.callbacks.lock() {
        if let Some(callback) = callbacks.on_cancel.as_mut() {
            catch_user_panic("browser_view_controller_cancel_trampoline", callback);
        }
    }
}

unsafe extern "C" fn browser_view_controller_should_present_trampoline(
    context: *mut c_void,
    peer: *mut c_void,
    discovery_json: *const std::ffi::c_char,
) -> bool {
    let Some(context) = NonNull::new(context.cast::<BrowserViewControllerDelegateState>()) else {
        return true;
    };
    let peer = unsafe { PeerId::from_owned_raw(peer) };
    let discovery = if discovery_json.is_null() {
        None
    } else {
        let json = unsafe { std::ffi::CStr::from_ptr(discovery_json) }.to_string_lossy();
        serde_json::from_str::<HashMap<String, String>>(&json).ok()
    };
    let mut result = true;
    if let Ok(mut callbacks) = unsafe { context.as_ref() }.callbacks.lock() {
        if let Some(callback) = callbacks.should_present_peer.as_mut() {
            catch_user_panic("browser_view_controller_should_present_trampoline", || {
                result = callback(peer, discovery);
            });
        }
    }
    result
}
