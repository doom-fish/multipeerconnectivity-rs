use core::ffi::c_void;
use core::ptr::{self, NonNull};
use std::ffi::CString;
use std::fmt;

use crate::error::{copy_and_free_string, take_error, MultipeerError, Result};
use crate::ffi;

pub struct PeerId {
    raw: NonNull<c_void>,
}

impl PeerId {
    /// Create a new local peer identifier.
    ///
    /// # Errors
    ///
    /// Returns an error if the display name is empty, contains an embedded NUL byte,
    /// or exceeds Apple's 63-byte UTF-8 limit.
    pub fn new(display_name: impl AsRef<str>) -> Result<Self> {
        let display_name = display_name.as_ref();
        if display_name.is_empty() {
            return Err(MultipeerError::InvalidArgument(
                "display name must not be empty".into(),
            ));
        }
        if display_name.len() > 63 {
            return Err(MultipeerError::InvalidArgument(
                "display name must be at most 63 UTF-8 bytes".into(),
            ));
        }
        let c_name = CString::new(display_name).map_err(|_| {
            MultipeerError::InvalidArgument("display name must not contain NUL bytes".into())
        })?;
        let mut error = ptr::null_mut();
        let raw = unsafe { ffi::peer::mpc_peer_id_create(c_name.as_ptr(), &mut error) };
        NonNull::new(raw).map_or_else(|| Err(take_error(error)), |raw| Ok(Self { raw }))
    }

    #[must_use]
    pub fn display_name(&self) -> String {
        let string = unsafe { ffi::peer::mpc_peer_id_display_name(self.raw.as_ptr()) };
        copy_and_free_string(string)
    }

    /// Serialize this peer ID using `NSSecureCoding`, suitable for custom-discovery flows.
    ///
    /// # Errors
    ///
    /// Returns an error if the framework cannot archive the peer ID.
    pub fn archived_data(&self) -> Result<Vec<u8>> {
        let mut bytes = ptr::null_mut();
        let mut length = 0usize;
        let mut error = ptr::null_mut();
        let status = unsafe {
            ffi::peer::mpc_peer_id_archive(self.raw.as_ptr(), &mut bytes, &mut length, &mut error)
        };
        if status != ffi::core::MPC_OK {
            return Err(take_error(error));
        }
        if bytes.is_null() || length == 0 {
            return Ok(Vec::new());
        }
        let data = unsafe { std::slice::from_raw_parts(bytes.cast::<u8>(), length) }.to_vec();
        unsafe { ffi::core::mpc_bytes_free(bytes) };
        Ok(data)
    }

    /// Reconstruct a peer ID previously produced by [`Self::archived_data`].
    ///
    /// # Errors
    ///
    /// Returns an error if the archived bytes cannot be decoded as an `MCPeerID`.
    pub fn from_archived_data(bytes: &[u8]) -> Result<Self> {
        let mut error = ptr::null_mut();
        let raw = unsafe {
            ffi::peer::mpc_peer_id_from_archived_data(
                bytes.as_ptr().cast::<c_void>(),
                bytes.len(),
                &mut error,
            )
        };
        NonNull::new(raw).map_or_else(|| Err(take_error(error)), |raw| Ok(Self { raw }))
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
        let raw = unsafe { ffi::core::mpc_object_retain(self.raw.as_ptr()) };
        unsafe { Self::from_owned_raw(raw) }
    }
}

impl Drop for PeerId {
    fn drop(&mut self) {
        unsafe { ffi::core::mpc_object_release(self.raw.as_ptr()) };
    }
}

impl fmt::Debug for PeerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PeerId")
            .field("display_name", &self.display_name())
            .finish()
    }
}
