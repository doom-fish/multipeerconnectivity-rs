#![allow(clippy::missing_errors_doc, clippy::struct_field_names)]

use core::ffi::c_void;
use core::ptr::{self, NonNull};
use std::collections::HashMap;
use std::ffi::CString;
use std::fmt;
use std::sync::Mutex;

use doom_fish_utils::panic_safe::catch_user_panic;

use crate::error::{
    copy_and_free_string, take_framework_error, FrameworkError, MultipeerError, Result,
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
        let discovery_info_json = match discovery_info {
            Some(info) => Some(
                CString::new(
                    serde_json::to_string(info)
                        .map_err(|err| MultipeerError::InvalidArgument(err.to_string()))?,
                )
                .map_err(|_| {
                    MultipeerError::InvalidArgument(
                        "discovery info JSON must not contain NUL bytes".into(),
                    )
                })?,
            ),
            None => None,
        };
        let service_type = validate_service_type(service_type.as_ref())?;
        let raw = unsafe {
            ffi::advertiser::mpc_advertiser_create(
                peer.as_ptr(),
                discovery_info_json
                    .as_ref()
                    .map_or(ptr::null(), |value| value.as_ptr()),
                service_type.as_ptr(),
            )
        };
        let raw = NonNull::new(raw).ok_or_else(|| {
            MultipeerError::OperationFailed("failed to create MCNearbyServiceAdvertiser".into())
        })?;
        Ok(Self {
            raw,
            delegate_state: None,
        })
    }

    pub(crate) unsafe fn from_owned_raw(raw: *mut c_void) -> Self {
        Self {
            raw: NonNull::new(raw).expect("advertiser raw pointer must not be null"),
            delegate_state: None,
        }
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
            );
        }
        self.delegate_state = Some(ptr);
    }

    /// Removes the `MultipeerConnectivity` advertiser delegate.
    pub fn clear_delegate(&mut self) {
        if let Some(state) = self.delegate_state.take() {
            unsafe {
                ffi::advertiser::mpc_advertiser_clear_delegate(self.raw.as_ptr());
                drop(Box::from_raw(state.as_ptr()));
            }
        }
    }

    #[cfg(feature = "async")]
    pub(crate) const fn as_ptr(&self) -> *mut c_void {
        self.raw.as_ptr()
    }
}

impl Clone for NearbyServiceAdvertiser {
    fn clone(&self) -> Self {
        let raw = unsafe { ffi::core::mpc_object_retain(self.raw.as_ptr()) };
        unsafe { Self::from_owned_raw(raw) }
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
