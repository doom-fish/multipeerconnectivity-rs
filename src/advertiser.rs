#![allow(clippy::missing_errors_doc, clippy::struct_field_names)]

use core::ffi::c_void;
use core::ptr::{self, NonNull};
use std::collections::HashMap;
use std::fmt;
use std::sync::Mutex;

use doom_fish_utils::panic_safe::catch_user_panic;

use crate::error::{
    copy_and_free_string, take_error, take_framework_error, FrameworkError, Result,
};
use crate::ffi;
use crate::peer::PeerId;
use crate::session::Session;

type InvitationHandler = dyn FnMut(PeerId, Option<Vec<u8>>) -> InvitationResponse + Send;
type AdvertiserErrorHandler = dyn FnMut(FrameworkError) + Send;

struct RetainedSessionHandle(*mut c_void);

unsafe impl Send for RetainedSessionHandle {}

impl RetainedSessionHandle {
    fn new(session: &Session) -> Self {
        Self(unsafe { ffi::core::mpc_object_retain(session.as_ptr()) })
    }

    fn cloned_session(&self) -> Session {
        unsafe { Session::from_owned_raw(ffi::core::mpc_object_retain(self.0)) }
    }
}

impl Drop for RetainedSessionHandle {
    fn drop(&mut self) {
        unsafe { ffi::core::mpc_object_release(self.0) };
    }
}

#[derive(Debug)]
/// Represents how a `MultipeerConnectivity` advertiser should handle an invitation.
pub enum InvitationResponse {
    /// Declines the `MultipeerConnectivity` invitation.
    Decline,
    /// Accepts the `MultipeerConnectivity` invitation with the provided session.
    Accept(Session),
}

/// Configures `MultipeerConnectivity` advertiser delegate callbacks.
pub struct NearbyServiceAdvertiserDelegate {
    on_invitation: Option<Box<InvitationHandler>>,
    on_error: Option<Box<AdvertiserErrorHandler>>,
}

impl NearbyServiceAdvertiserDelegate {
    #[must_use]
    /// Creates an empty `MultipeerConnectivity` advertiser delegate.
    pub const fn new() -> Self {
        Self {
            on_invitation: None,
            on_error: None,
        }
    }

    #[must_use]
    /// Registers a `MultipeerConnectivity` invitation callback.
    pub fn on_invitation<F>(mut self, handler: F) -> Self
    where
        F: FnMut(PeerId, Option<Vec<u8>>) -> InvitationResponse + Send + 'static,
    {
        self.on_invitation = Some(Box::new(handler));
        self
    }

    #[must_use]
    /// Registers a `MultipeerConnectivity` advertiser error callback.
    pub fn on_error<F>(mut self, handler: F) -> Self
    where
        F: FnMut(FrameworkError) + Send + 'static,
    {
        self.on_error = Some(Box::new(handler));
        self
    }
}

impl Default for NearbyServiceAdvertiserDelegate {
    fn default() -> Self {
        Self::new()
    }
}

struct AdvertiserDelegateState {
    callbacks: Mutex<NearbyServiceAdvertiserDelegate>,
    ref_count: crate::refcount::RefCount,
}

impl crate::refcount::RefCounted for AdvertiserDelegateState {
    fn ref_count(&self) -> &crate::refcount::RefCount {
        &self.ref_count
    }
}

extern "C" fn advertiser_context_retain(context: *mut c_void) {
    unsafe { crate::refcount::retain::<AdvertiserDelegateState>(context) };
}

extern "C" fn advertiser_context_release(context: *mut c_void) {
    unsafe { crate::refcount::release::<AdvertiserDelegateState>(context) };
}

/// Wraps a `MultipeerConnectivity` `MCNearbyServiceAdvertiser`.
pub struct NearbyServiceAdvertiser {
    raw: NonNull<c_void>,
    delegate_state: Option<NonNull<AdvertiserDelegateState>>,
}

impl NearbyServiceAdvertiser {
    /// Creates a `MultipeerConnectivity` advertiser for the local peer.
    pub fn new(
        peer: &PeerId,
        discovery_info: Option<&HashMap<String, String>>,
        service_type: impl AsRef<str>,
    ) -> Result<Self> {
        let discovery_info_json = crate::validation::discovery_info_cstring(discovery_info)?;
        let service_type = crate::validation::service_type_cstring(service_type.as_ref())?;
        let mut error = ptr::null_mut();
        let raw = unsafe {
            ffi::advertiser::mpc_advertiser_create(
                peer.as_ptr(),
                discovery_info_json
                    .as_ref()
                    .map_or(ptr::null(), |value| value.as_ptr()),
                service_type.as_ptr(),
                &raw mut error,
            )
        };
        let raw = NonNull::new(raw).ok_or_else(|| take_error(error))?;
        Ok(Self {
            raw,
            delegate_state: None,
        })
    }

    #[must_use]
    /// Returns the local `MultipeerConnectivity` peer identifier.
    pub fn my_peer_id(&self) -> PeerId {
        let raw = unsafe { ffi::advertiser::mpc_advertiser_copy_my_peer(self.raw.as_ptr()) };
        unsafe { PeerId::from_owned_raw(raw) }
    }

    #[must_use]
    /// Returns the `MultipeerConnectivity` discovery info dictionary.
    pub fn discovery_info(&self) -> Option<HashMap<String, String>> {
        let string =
            unsafe { ffi::advertiser::mpc_advertiser_discovery_info_json(self.raw.as_ptr()) };
        if string.is_null() {
            return None;
        }
        let json = copy_and_free_string(string);
        serde_json::from_str(&json).ok()
    }

    #[must_use]
    /// Returns the `MultipeerConnectivity` service type.
    pub fn service_type(&self) -> String {
        let string = unsafe { ffi::advertiser::mpc_advertiser_service_type(self.raw.as_ptr()) };
        copy_and_free_string(string)
    }

    /// Starts advertising this `MultipeerConnectivity` peer.
    pub fn start_advertising_peer(&self) {
        unsafe { ffi::advertiser::mpc_advertiser_start(self.raw.as_ptr()) };
    }

    /// Stops advertising this `MultipeerConnectivity` peer.
    pub fn stop_advertising_peer(&self) {
        unsafe { ffi::advertiser::mpc_advertiser_stop(self.raw.as_ptr()) };
    }

    /// Installs basic `MultipeerConnectivity` invitation handling callbacks.
    pub fn set_delegate<F>(&mut self, invitation_session: Option<&Session>, mut on_invitation: F)
    where
        F: FnMut(PeerId, Option<Vec<u8>>) -> bool + Send + 'static,
    {
        let invitation_session = invitation_session.map(RetainedSessionHandle::new);
        self.set_callbacks(NearbyServiceAdvertiserDelegate::new().on_invitation(
            move |peer, payload| {
                if on_invitation(peer, payload) {
                    invitation_session
                        .as_ref()
                        .map_or(InvitationResponse::Decline, |session| {
                            InvitationResponse::Accept(session.cloned_session())
                        })
                } else {
                    InvitationResponse::Decline
                }
            },
        ));
    }

    /// Installs typed `MultipeerConnectivity` advertiser callbacks.
    pub fn set_callbacks(&mut self, callbacks: NearbyServiceAdvertiserDelegate) {
        self.clear_delegate();
        let has_error = callbacks.on_error.is_some();
        let state = Box::new(AdvertiserDelegateState {
            callbacks: Mutex::new(callbacks),
            ref_count: crate::refcount::RefCount::new(),
        });
        let ptr = NonNull::from(Box::leak(state));
        unsafe {
            ffi::advertiser::mpc_advertiser_set_delegate(
                self.raw.as_ptr(),
                ptr.as_ptr().cast::<c_void>(),
                Some(advertiser_invitation_trampoline),
                if has_error {
                    Some(advertiser_error_trampoline)
                } else {
                    None
                },
                advertiser_context_retain,
                advertiser_context_release,
            );
        }
        self.delegate_state = Some(ptr);
    }

    /// Removes the `MultipeerConnectivity` advertiser delegate.
    pub fn clear_delegate(&mut self) {
        if let Some(state) = self.delegate_state.take() {
            unsafe {
                ffi::advertiser::mpc_advertiser_clear_delegate(
                    self.raw.as_ptr(),
                    state.as_ptr().cast::<c_void>(),
                );
                crate::refcount::release::<AdvertiserDelegateState>(
                    state.as_ptr().cast::<c_void>(),
                );
            }
        }
    }

    #[cfg(feature = "async")]
    pub(crate) const fn as_ptr(&self) -> *mut c_void {
        self.raw.as_ptr()
    }
}

impl Drop for NearbyServiceAdvertiser {
    fn drop(&mut self) {
        self.clear_delegate();
        unsafe { ffi::core::mpc_object_release(self.raw.as_ptr()) };
    }
}

impl fmt::Debug for NearbyServiceAdvertiser {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NearbyServiceAdvertiser")
            .field("my_peer_id", &self.my_peer_id())
            .field("discovery_info", &self.discovery_info())
            .field("service_type", &self.service_type())
            .finish()
    }
}

unsafe extern "C" fn advertiser_invitation_trampoline(
    context: *mut c_void,
    peer: *mut c_void,
    context_bytes: *const c_void,
    context_length: usize,
) -> *mut c_void {
    let Some(context) = NonNull::new(context.cast::<AdvertiserDelegateState>()) else {
        return ptr::null_mut();
    };
    let peer = unsafe { PeerId::from_owned_raw(peer) };
    let payload = if context_bytes.is_null() || context_length == 0 {
        None
    } else {
        Some(
            unsafe { std::slice::from_raw_parts(context_bytes.cast::<u8>(), context_length) }
                .to_vec(),
        )
    };
    let mut result: *mut c_void = ptr::null_mut();
    if let Ok(mut callbacks) = unsafe { context.as_ref() }.callbacks.lock() {
        if let Some(callback) = callbacks.on_invitation.as_mut() {
            catch_user_panic("advertiser_invitation_trampoline", || {
                result = match callback(peer, payload) {
                    InvitationResponse::Decline => ptr::null_mut(),
                    InvitationResponse::Accept(session) => unsafe {
                        ffi::core::mpc_object_retain(session.as_ptr())
                    },
                };
            });
        }
    }
    result
}

unsafe extern "C" fn advertiser_error_trampoline(context: *mut c_void, error: *mut c_void) {
    let Some(context) = NonNull::new(context.cast::<AdvertiserDelegateState>()) else {
        if !error.is_null() {
            let _ = take_framework_error(error);
        }
        return;
    };
    let error = take_framework_error(error);
    if let Ok(mut callbacks) = unsafe { context.as_ref() }.callbacks.lock() {
        if let Some(callback) = callbacks.on_error.as_mut() {
            catch_user_panic("advertiser_error_trampoline", || callback(error));
        }
    }
}

#[cfg(test)]
mod tests {
    use core::ffi::CStr;
    use core::ptr;

    use crate::error::{take_error, MultipeerError};
    use crate::ffi;
    use crate::peer::PeerId;

    fn bridge_error(json: Option<&CStr>, service_type: &CStr) -> String {
        let peer = PeerId::new("bridge-validation").expect("peer");
        let mut error = ptr::null_mut();
        let raw = unsafe {
            ffi::advertiser::mpc_advertiser_create(
                peer.as_ptr(),
                json.map_or(ptr::null(), CStr::as_ptr),
                service_type.as_ptr(),
                &raw mut error,
            )
        };
        assert!(raw.is_null());
        match take_error(error) {
            MultipeerError::InvalidArgument(message) => message,
            other => panic!("expected an invalid-argument error, got {other:?}"),
        }
    }

    #[test]
    fn bridge_reports_malformed_discovery_info_json() {
        for json in [c"not json", c"[1]", c"{\"k\":1}"] {
            assert_eq!(
                bridge_error(Some(json), c"doom-chat"),
                "discoveryInfo must be a JSON object of string pairs"
            );
        }
    }

    #[test]
    fn bridge_reports_discovery_info_the_framework_would_abort_on() {
        assert_eq!(
            bridge_error(Some(c"{\"\":\"v\"}"), c"doom-chat"),
            "discovery info keys must not be empty"
        );
        assert_eq!(
            bridge_error(Some(c"{\"a=b\":\"v\"}"), c"doom-chat"),
            "discovery info keys must contain only printable ASCII characters other than '='"
        );
        let oversized = format!("{{\"k\":\"{}\"}}", "v".repeat(253));
        let oversized = std::ffi::CString::new(oversized).expect("json");
        assert_eq!(
            bridge_error(Some(&oversized), c"doom-chat"),
            "discovery info entries must be at most 254 bytes as key=value"
        );
    }

    #[test]
    fn bridge_reports_invalid_service_types() {
        assert_eq!(
            bridge_error(None, c"-chat"),
            "service type must not begin or end with a hyphen"
        );
        assert_eq!(
            bridge_error(None, c"doom--chat"),
            "service type must not contain consecutive hyphens"
        );
        assert_eq!(
            bridge_error(None, c"1234"),
            "service type must contain at least one letter"
        );
    }
}
