use core::ffi::c_void;
use core::ptr::{self, NonNull};
use std::collections::HashMap;
use std::ffi::CString;
use std::sync::Mutex;

use crate::error::{MultipeerError, Result};
use crate::ffi;
use crate::peer::{last_error, PeerId};
use crate::session::Session;

type InvitationHandler = dyn FnMut(PeerId, Option<Vec<u8>>) -> bool + Send;

struct AdvertiserDelegateState {
    on_invitation: Mutex<Box<InvitationHandler>>,
}

pub struct NearbyServiceAdvertiser {
    raw: NonNull<c_void>,
    delegate_state: Option<NonNull<AdvertiserDelegateState>>,
}

impl NearbyServiceAdvertiser {
    /// Create an advertiser for a service type.
    ///
    /// # Errors
    ///
    /// Returns an error if the service type or discovery info cannot be converted for the framework.
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
        let service_type = CString::new(service_type.as_ref()).map_err(|_| {
            MultipeerError::InvalidArgument("service type must not contain NUL bytes".into())
        })?;
        let mut error = ptr::null_mut();
        let raw = unsafe {
            ffi::mpc_advertiser_create(
                peer.as_ptr(),
                discovery_info_json.as_ref().map_or(ptr::null(), |value| value.as_ptr()),
                service_type.as_ptr(),
                &mut error,
            )
        };
        let raw = NonNull::new(raw).ok_or_else(|| last_error(error))?;
        Ok(Self {
            raw,
            delegate_state: None,
        })
    }

    pub fn start_advertising_peer(&self) {
        unsafe { ffi::mpc_advertiser_start(self.raw.as_ptr()) };
    }

    pub fn stop_advertising_peer(&self) {
        unsafe { ffi::mpc_advertiser_stop(self.raw.as_ptr()) };
    }

    pub fn set_delegate<F>(&mut self, invitation_session: Option<&Session>, on_invitation: F)
    where
        F: FnMut(PeerId, Option<Vec<u8>>) -> bool + Send + 'static,
    {
        self.clear_delegate();
        let state = Box::new(AdvertiserDelegateState {
            on_invitation: Mutex::new(Box::new(on_invitation)),
        });
        let ptr = NonNull::from(Box::leak(state));
        unsafe {
            ffi::mpc_advertiser_set_delegate(
                self.raw.as_ptr(),
                invitation_session.map_or(ptr::null_mut(), Session::as_ptr),
                ptr.as_ptr().cast::<c_void>(),
                advertiser_invitation_trampoline,
            );
        }
        self.delegate_state = Some(ptr);
    }

    pub fn clear_delegate(&mut self) {
        if let Some(state) = self.delegate_state.take() {
            unsafe {
                ffi::mpc_advertiser_clear_delegate(self.raw.as_ptr());
                drop(Box::from_raw(state.as_ptr()));
            }
        }
    }
}

impl Drop for NearbyServiceAdvertiser {
    fn drop(&mut self) {
        self.clear_delegate();
        unsafe { ffi::mpc_object_release(self.raw.as_ptr()) };
    }
}

unsafe extern "C" fn advertiser_invitation_trampoline(
    context: *mut c_void,
    peer: *mut c_void,
    context_bytes: *const c_void,
    context_length: usize,
) -> bool {
    let Some(context) = NonNull::new(context.cast::<AdvertiserDelegateState>()) else {
        return false;
    };
    let peer = PeerId::from_owned_raw(peer);
    let payload = if context_bytes.is_null() || context_length == 0 {
        None
    } else {
        Some(unsafe { std::slice::from_raw_parts(context_bytes.cast::<u8>(), context_length) }.to_vec())
    };
    context
        .as_ref()
        .on_invitation
        .lock()
        .is_ok_and(|mut callback| callback(peer, payload))
}
