use core::ffi::c_void;
use core::ptr::{self, NonNull};
use std::ffi::CString;
use std::fmt;
use std::path::Path;
use std::sync::Mutex;

use crate::error::{MultipeerError, Result};
use crate::ffi;
use crate::peer::{last_error, PeerId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionSendDataMode {
    Reliable,
    Unreliable,
}

impl SessionSendDataMode {
    const fn as_raw(self) -> i32 {
        match self {
            Self::Reliable => 0,
            Self::Unreliable => 1,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncryptionPreference {
    Optional,
    Required,
    None,
}

impl EncryptionPreference {
    const fn as_raw(self) -> i32 {
        match self {
            Self::Optional => 0,
            Self::Required => 1,
            Self::None => 2,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionState {
    NotConnected,
    Connecting,
    Connected,
    Unknown(i32),
}

impl SessionState {
    const fn from_raw(value: i32) -> Self {
        match value {
            0 => Self::NotConnected,
            1 => Self::Connecting,
            2 => Self::Connected,
            other => Self::Unknown(other),
        }
    }
}

type SessionStateHandler = dyn FnMut(PeerId, SessionState) + Send;
type SessionDataHandler = dyn FnMut(PeerId, Vec<u8>) + Send;

struct SessionDelegateState {
    on_state: Mutex<Box<SessionStateHandler>>,
    on_data: Mutex<Box<SessionDataHandler>>,
}

pub struct Session {
    raw: NonNull<c_void>,
    delegate_state: Option<NonNull<SessionDelegateState>>,
}

impl Session {
    /// Create a new session for the local peer.
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying framework rejects the session creation.
    pub fn new(peer: &PeerId, encryption_preference: EncryptionPreference) -> Result<Self> {
        unsafe { Self::with_security_identity(peer, None, encryption_preference) }
    }

    /// `security_identity` mirrors Apple's raw `[SecIdentityRef, certs...]` array.
    /// Pass `None` unless you already have Security framework objects.
    ///
    /// # Safety
    ///
    /// Any raw pointers inside `security_identity` must remain valid for the duration of the call and
    /// must point to the CoreFoundation / Security objects expected by `MCSession`.
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying framework rejects the session creation.
    pub unsafe fn with_security_identity(
        peer: &PeerId,
        security_identity: Option<&[*mut c_void]>,
        encryption_preference: EncryptionPreference,
    ) -> Result<Self> {
        let mut error = ptr::null_mut();
        let (identity_ptr, identity_len) = security_identity
            .map_or((ptr::null(), 0), |items| (items.as_ptr(), items.len()));
        let raw = ffi::mpc_session_create_with_identity(
            peer.as_ptr(),
            identity_ptr,
            identity_len,
            encryption_preference.as_raw(),
            &mut error,
        );
        let raw = NonNull::new(raw).ok_or_else(|| last_error(error))?;
        Ok(Self {
            raw,
            delegate_state: None,
        })
    }

    #[must_use]
    pub fn connected_peers(&self) -> Vec<PeerId> {
        let mut array = ptr::null_mut();
        let mut count = 0usize;
        unsafe { ffi::mpc_session_copy_connected_peers(self.raw.as_ptr(), &mut array, &mut count) };
        if array.is_null() || count == 0 {
            return Vec::new();
        }
        let items = unsafe { std::slice::from_raw_parts(array.cast::<*mut c_void>(), count) };
        let peers = items
            .iter()
            .copied()
            .filter(|ptr| !ptr.is_null())
            .map(|raw| unsafe { PeerId::from_owned_raw(raw) })
            .collect();
        unsafe { ffi::mpc_ptr_array_free(array) };
        peers
    }

    /// Send a data payload to one or more connected peers.
    ///
    /// # Errors
    ///
    /// Returns an error if no destination peers are supplied or if the framework send fails.
    pub fn send(
        &self,
        data: &[u8],
        peers: &[&PeerId],
        mode: SessionSendDataMode,
    ) -> Result<()> {
        if peers.is_empty() {
            return Err(MultipeerError::InvalidArgument(
                "send requires at least one destination peer".into(),
            ));
        }
        let peer_ptrs: Vec<*mut c_void> = peers.iter().map(|peer| peer.as_ptr()).collect();
        let mut error = ptr::null_mut();
        let status = unsafe {
            ffi::mpc_session_send_data(
                self.raw.as_ptr(),
                data.as_ptr().cast::<c_void>(),
                data.len(),
                peer_ptrs.as_ptr(),
                peer_ptrs.len(),
                mode.as_raw(),
                &mut error,
            )
        };
        if status == ffi::MPC_OK {
            Ok(())
        } else {
            Err(last_error(error))
        }
    }

    /// Send a file-backed resource to a connected peer.
    ///
    /// # Errors
    ///
    /// Returns an error if the path/resource name are invalid for FFI or if the framework refuses the transfer.
    pub fn send_resource(
        &self,
        path: impl AsRef<Path>,
        resource_name: impl AsRef<str>,
        peer: &PeerId,
    ) -> Result<ResourceTransfer> {
        let path = path.as_ref();
        let path = path.to_str().ok_or_else(|| {
            MultipeerError::InvalidArgument("resource path must be valid UTF-8".into())
        })?;
        let path = CString::new(path).map_err(|_| {
            MultipeerError::InvalidArgument("resource path must not contain NUL bytes".into())
        })?;
        let resource_name = CString::new(resource_name.as_ref()).map_err(|_| {
            MultipeerError::InvalidArgument("resource name must not contain NUL bytes".into())
        })?;
        let mut error = ptr::null_mut();
        let raw = unsafe {
            ffi::mpc_session_send_resource(
                self.raw.as_ptr(),
                path.as_ptr(),
                resource_name.as_ptr(),
                peer.as_ptr(),
                &mut error,
            )
        };
        let raw = NonNull::new(raw).ok_or_else(|| last_error(error))?;
        Ok(ResourceTransfer { raw })
    }

    /// Start a named byte stream to a connected peer.
    ///
    /// # Errors
    ///
    /// Returns an error if the stream name is not valid for FFI or the framework cannot create the stream.
    pub fn start_stream(
        &self,
        stream_name: impl AsRef<str>,
        peer: &PeerId,
    ) -> Result<OutputStream> {
        let stream_name = CString::new(stream_name.as_ref()).map_err(|_| {
            MultipeerError::InvalidArgument("stream name must not contain NUL bytes".into())
        })?;
        let mut error = ptr::null_mut();
        let raw = unsafe {
            ffi::mpc_session_start_stream(
                self.raw.as_ptr(),
                stream_name.as_ptr(),
                peer.as_ptr(),
                &mut error,
            )
        };
        let raw = NonNull::new(raw).ok_or_else(|| last_error(error))?;
        Ok(OutputStream { raw })
    }

    pub fn disconnect(&self) {
        unsafe { ffi::mpc_session_disconnect(self.raw.as_ptr()) };
    }

    pub fn set_delegate<F, G>(&mut self, on_state: F, on_data: G)
    where
        F: FnMut(PeerId, SessionState) + Send + 'static,
        G: FnMut(PeerId, Vec<u8>) + Send + 'static,
    {
        self.clear_delegate();
        let state = Box::new(SessionDelegateState {
            on_state: Mutex::new(Box::new(on_state)),
            on_data: Mutex::new(Box::new(on_data)),
        });
        let ptr = NonNull::from(Box::leak(state));
        unsafe {
            ffi::mpc_session_set_delegate(
                self.raw.as_ptr(),
                ptr.as_ptr().cast::<c_void>(),
                session_state_trampoline,
                session_data_trampoline,
            );
        }
        self.delegate_state = Some(ptr);
    }

    pub fn clear_delegate(&mut self) {
        if let Some(state) = self.delegate_state.take() {
            unsafe {
                ffi::mpc_session_clear_delegate(self.raw.as_ptr());
                drop(Box::from_raw(state.as_ptr()));
            }
        }
    }

    pub(crate) const fn as_ptr(&self) -> *mut c_void {
        self.raw.as_ptr()
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        self.clear_delegate();
        unsafe { ffi::mpc_object_release(self.raw.as_ptr()) };
    }
}

impl fmt::Debug for Session {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Session")
            .field("connected_peers", &self.connected_peers())
            .finish()
    }
}

pub struct ResourceTransfer {
    raw: NonNull<c_void>,
}

impl ResourceTransfer {
    #[must_use]
    pub fn fraction_completed(&self) -> f64 {
        unsafe { ffi::mpc_progress_fraction_completed(self.raw.as_ptr()) }
    }

    #[must_use]
    pub fn is_finished(&self) -> bool {
        unsafe { ffi::mpc_progress_is_finished(self.raw.as_ptr()) }
    }
}

impl Drop for ResourceTransfer {
    fn drop(&mut self) {
        unsafe { ffi::mpc_object_release(self.raw.as_ptr()) };
    }
}

pub struct OutputStream {
    raw: NonNull<c_void>,
}

impl OutputStream {
    pub fn open(&self) {
        unsafe { ffi::mpc_output_stream_open(self.raw.as_ptr()) };
    }

    pub fn close(&self) {
        unsafe { ffi::mpc_output_stream_close(self.raw.as_ptr()) };
    }

    /// Write bytes into the underlying `NSOutputStream`.
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying stream reports a write failure.
    pub fn write(&self, bytes: &[u8]) -> Result<usize> {
        let written = unsafe {
            ffi::mpc_output_stream_write(
                self.raw.as_ptr(),
                bytes.as_ptr().cast::<c_void>(),
                bytes.len(),
            )
        };
        if written >= 0 {
            usize::try_from(written).map_err(|_| {
                MultipeerError::OperationFailed(
                    "NSOutputStream returned a byte count that could not fit in usize".into(),
                )
            })
        } else {
            Err(MultipeerError::OperationFailed(
                "NSOutputStream write returned an error".into(),
            ))
        }
    }
}

impl Drop for OutputStream {
    fn drop(&mut self) {
        unsafe { ffi::mpc_object_release(self.raw.as_ptr()) };
    }
}

unsafe extern "C" fn session_state_trampoline(
    context: *mut c_void,
    peer: *mut c_void,
    state: i32,
) {
    let Some(context) = NonNull::new(context.cast::<SessionDelegateState>()) else {
        return;
    };
    let peer = PeerId::from_owned_raw(peer);
    if let Ok(mut callback) = context.as_ref().on_state.lock() {
        callback(peer, SessionState::from_raw(state));
    }
}

unsafe extern "C" fn session_data_trampoline(
    context: *mut c_void,
    peer: *mut c_void,
    data: *const c_void,
    length: usize,
) {
    let Some(context) = NonNull::new(context.cast::<SessionDelegateState>()) else {
        return;
    };
    let peer = PeerId::from_owned_raw(peer);
    let payload = if data.is_null() || length == 0 {
        Vec::new()
    } else {
        unsafe { std::slice::from_raw_parts(data.cast::<u8>(), length) }.to_vec()
    };
    if let Ok(mut callback) = context.as_ref().on_data.lock() {
        callback(peer, payload);
    }
}
