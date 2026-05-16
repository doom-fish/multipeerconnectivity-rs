use core::ffi::c_void;
use core::ptr::NonNull;
use std::ffi::{CStr, CString};
use std::fmt;

use crate::error::{MultipeerError, Result};
use crate::ffi;

pub struct PeerId {
    raw: NonNull<c_void>,
}

impl PeerId {
    /// Create a new local peer identifier.
    ///
    /// # Errors
    ///
    /// Returns an error if the display name is empty or contains an embedded NUL byte.
    pub fn new(display_name: impl AsRef<str>) -> Result<Self> {
        let display_name = display_name.as_ref();
        if display_name.is_empty() {
            return Err(MultipeerError::InvalidArgument(
                "display name must not be empty".into(),
            ));
        }
        let c_name = CString::new(display_name).map_err(|_| {
            MultipeerError::InvalidArgument("display name must not contain NUL bytes".into())
        })?;
        let mut error = std::ptr::null_mut();
        let raw = unsafe { ffi::mpc_peer_id_create(c_name.as_ptr(), &mut error) };
        NonNull::new(raw).map_or_else(|| Err(last_error(error)), |raw| Ok(Self { raw }))
    }

    #[must_use]
    pub fn display_name(&self) -> String {
        let string = unsafe { ffi::mpc_peer_id_display_name(self.raw.as_ptr()) };
        copy_and_free_string(string)
    }

    pub(crate) const fn as_ptr(&self) -> *mut c_void {
        self.raw.as_ptr()
    }

    pub(crate) unsafe fn from_owned_raw(raw: *mut c_void) -> Self {
        Self {
            raw: NonNull::new(raw).expect("peer raw pointer must not be null"),
        }
    }
}

impl Clone for PeerId {
    fn clone(&self) -> Self {
        let raw = unsafe { ffi::mpc_object_retain(self.raw.as_ptr()) };
        unsafe { Self::from_owned_raw(raw) }
    }
}

impl Drop for PeerId {
    fn drop(&mut self) {
        unsafe { ffi::mpc_object_release(self.raw.as_ptr()) };
    }
}

impl fmt::Debug for PeerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PeerId")
            .field("display_name", &self.display_name())
            .finish()
    }
}

pub(crate) fn copy_and_free_string(ptr: *mut std::ffi::c_char) -> String {
    if ptr.is_null() {
        return String::new();
    }
    let value = unsafe { CStr::from_ptr(ptr) }.to_string_lossy().into_owned();
    unsafe { ffi::mpc_string_free(ptr) };
    value
}

pub(crate) fn last_error(ptr: *mut std::ffi::c_char) -> MultipeerError {
    let message = if ptr.is_null() {
        "unknown MultipeerConnectivity failure".to_string()
    } else {
        copy_and_free_string(ptr)
    };
    MultipeerError::OperationFailed(message)
}
