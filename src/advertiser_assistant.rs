#![allow(clippy::missing_errors_doc, clippy::struct_field_names)]

use core::ffi::c_void;
use core::ptr::{self, NonNull};
use std::collections::HashMap;
use std::ffi::CString;
use std::fmt;
use std::sync::Mutex;

use crate::error::{copy_and_free_string, MultipeerError, Result};
use crate::ffi;
use crate::session::Session;

type AssistantHandler = dyn FnMut() + Send;

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
            "service type must contain only lowercase ASCII letters, digits, or hyphens"
                .into(),
        ));
    }
    CString::new(service_type).map_err(|_| {
        MultipeerError::InvalidArgument("service type must not contain NUL bytes".into())
    })
}

pub struct AdvertiserAssistantDelegate {
    on_will_present_invitation: Option<Box<AssistantHandler>>,
    on_did_dismiss_invitation: Option<Box<AssistantHandler>>,
}

impl AdvertiserAssistantDelegate {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            on_will_present_invitation: None,
            on_did_dismiss_invitation: None,
        }
    }

    #[must_use]
    pub fn on_will_present_invitation<F>(mut self, handler: F) -> Self
    where
        F: FnMut() + Send + 'static,
    {
        self.on_will_present_invitation = Some(Box::new(handler));
        self
    }

    #[must_use]
    pub fn on_did_dismiss_invitation<F>(mut self, handler: F) -> Self
    where
        F: FnMut() + Send + 'static,
    {
        self.on_did_dismiss_invitation = Some(Box::new(handler));
        self
    }
}

impl Default for AdvertiserAssistantDelegate {
    fn default() -> Self {
        Self::new()
    }
}

struct AdvertiserAssistantDelegateState {
    callbacks: Mutex<AdvertiserAssistantDelegate>,
}

pub struct AdvertiserAssistant {
    raw: NonNull<c_void>,
    delegate_state: Option<NonNull<AdvertiserAssistantDelegateState>>,
}

impl AdvertiserAssistant {
    pub fn new(
        service_type: impl AsRef<str>,
        discovery_info: Option<&HashMap<String, String>>,
        session: &Session,
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
            ffi::advertiser_assistant::mpc_advertiser_assistant_create(
                service_type.as_ptr(),
                discovery_info_json
                    .as_ref()
                    .map_or(ptr::null(), |value| value.as_ptr()),
                session.as_ptr(),
            )
        };
        let raw = NonNull::new(raw).ok_or_else(|| {
            MultipeerError::OperationFailed("failed to create MCAdvertiserAssistant".into())
        })?;
        Ok(Self {
            raw,
            delegate_state: None,
        })
    }

    pub(crate) unsafe fn from_owned_raw(raw: *mut c_void) -> Self {
        Self {
            raw: NonNull::new(raw).expect("assistant raw pointer must not be null"),
            delegate_state: None,
        }
    }

    #[must_use]
    pub fn session(&self) -> Session {
        let raw = unsafe { ffi::advertiser_assistant::mpc_advertiser_assistant_copy_session(self.raw.as_ptr()) };
        unsafe { Session::from_owned_raw(raw) }
    }

    #[must_use]
    pub fn discovery_info(&self) -> Option<HashMap<String, String>> {
        let string = unsafe {
            ffi::advertiser_assistant::mpc_advertiser_assistant_discovery_info_json(
                self.raw.as_ptr(),
            )
        };
        if string.is_null() {
            return None;
        }
        let json = copy_and_free_string(string);
        serde_json::from_str(&json).ok()
    }

    #[must_use]
    pub fn service_type(&self) -> String {
        let string = unsafe {
            ffi::advertiser_assistant::mpc_advertiser_assistant_service_type(self.raw.as_ptr())
        };
        copy_and_free_string(string)
    }

    pub fn start(&self) {
        unsafe { ffi::advertiser_assistant::mpc_advertiser_assistant_start(self.raw.as_ptr()) };
    }

    pub fn stop(&self) {
        unsafe { ffi::advertiser_assistant::mpc_advertiser_assistant_stop(self.raw.as_ptr()) };
    }

    pub fn set_callbacks(&mut self, callbacks: AdvertiserAssistantDelegate) {
        self.clear_delegate();
        let has_will = callbacks.on_will_present_invitation.is_some();
        let has_did = callbacks.on_did_dismiss_invitation.is_some();
        let state = Box::new(AdvertiserAssistantDelegateState {
            callbacks: Mutex::new(callbacks),
        });
        let ptr = NonNull::from(Box::leak(state));
        unsafe {
            ffi::advertiser_assistant::mpc_advertiser_assistant_set_delegate(
                self.raw.as_ptr(),
                ptr.as_ptr().cast::<c_void>(),
                if has_will {
                    Some(advertiser_assistant_will_present_trampoline)
                } else {
                    None
                },
                if has_did {
                    Some(advertiser_assistant_did_dismiss_trampoline)
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
                ffi::advertiser_assistant::mpc_advertiser_assistant_clear_delegate(self.raw.as_ptr());
                drop(Box::from_raw(state.as_ptr()));
            }
        }
    }
}

impl Clone for AdvertiserAssistant {
    fn clone(&self) -> Self {
        let raw = unsafe { ffi::core::mpc_object_retain(self.raw.as_ptr()) };
        unsafe { Self::from_owned_raw(raw) }
    }
}

impl Drop for AdvertiserAssistant {
    fn drop(&mut self) {
        self.clear_delegate();
        unsafe { ffi::core::mpc_object_release(self.raw.as_ptr()) };
    }
}

impl fmt::Debug for AdvertiserAssistant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AdvertiserAssistant")
            .field("service_type", &self.service_type())
            .field("discovery_info", &self.discovery_info())
            .finish()
    }
}

unsafe extern "C" fn advertiser_assistant_will_present_trampoline(context: *mut c_void) {
    let Some(context) = NonNull::new(context.cast::<AdvertiserAssistantDelegateState>()) else {
        return;
    };
    if let Ok(mut callbacks) = unsafe { context.as_ref() }.callbacks.lock() {
        if let Some(callback) = callbacks.on_will_present_invitation.as_mut() {
            callback();
        }
    }
}

unsafe extern "C" fn advertiser_assistant_did_dismiss_trampoline(context: *mut c_void) {
    let Some(context) = NonNull::new(context.cast::<AdvertiserAssistantDelegateState>()) else {
        return;
    };
    if let Ok(mut callbacks) = unsafe { context.as_ref() }.callbacks.lock() {
        if let Some(callback) = callbacks.on_did_dismiss_invitation.as_mut() {
            callback();
        }
    }
}
