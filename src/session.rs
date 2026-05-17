#![allow(
    clippy::missing_errors_doc,
    clippy::missing_safety_doc,
    clippy::struct_field_names
)]

use core::ffi::{c_char, c_void};
use core::ptr::{self, NonNull};
use std::ffi::{CStr, CString};
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use doom_fish_utils::panic_safe::catch_user_panic;

use crate::error::{
    take_error, take_framework_error, take_optional_framework_error, FrameworkError,
    MultipeerError, Result,
};
use crate::ffi;
use crate::peer::PeerId;

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

    const fn from_raw(value: i32) -> Self {
        match value {
            1 => Self::Required,
            2 => Self::None,
            _ => Self::Optional,
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
    pub(crate) const fn from_raw(value: i32) -> Self {
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
type SessionStreamHandler = dyn FnMut(PeerId, String, InputStream) + Send;
type SessionResourceStartedHandler = dyn FnMut(PeerId, String, ResourceTransfer) + Send;
type SessionResourceFinishedHandler =
    dyn FnMut(PeerId, String, Option<PathBuf>, Option<FrameworkError>) + Send;
type SessionCertificateHandler = dyn FnMut(PeerId, Vec<SecurityIdentityItem>) -> bool + Send;
type ResourceSendCompletionHandler = dyn FnOnce(Option<FrameworkError>) + Send;

pub struct SecurityIdentityItem {
    raw: NonNull<c_void>,
}

impl SecurityIdentityItem {
    pub(crate) const fn as_ptr(&self) -> *mut c_void {
        self.raw.as_ptr()
    }

    pub(crate) unsafe fn from_owned_raw(raw: *mut c_void) -> Self {
        Self {
            raw: NonNull::new(raw).expect("identity raw pointer must not be null"),
        }
    }
}

impl Clone for SecurityIdentityItem {
    fn clone(&self) -> Self {
        let raw = unsafe { ffi::core::mpc_object_retain(self.raw.as_ptr()) };
        unsafe { Self::from_owned_raw(raw) }
    }
}

impl Drop for SecurityIdentityItem {
    fn drop(&mut self) {
        unsafe { ffi::core::mpc_object_release(self.raw.as_ptr()) };
    }
}

impl fmt::Debug for SecurityIdentityItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SecurityIdentityItem")
            .finish_non_exhaustive()
    }
}

pub struct SessionDelegate {
    on_state: Option<Box<SessionStateHandler>>,
    on_data: Option<Box<SessionDataHandler>>,
    on_stream: Option<Box<SessionStreamHandler>>,
    on_resource_started: Option<Box<SessionResourceStartedHandler>>,
    on_resource_finished: Option<Box<SessionResourceFinishedHandler>>,
    on_certificate: Option<Box<SessionCertificateHandler>>,
}

impl SessionDelegate {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            on_state: None,
            on_data: None,
            on_stream: None,
            on_resource_started: None,
            on_resource_finished: None,
            on_certificate: None,
        }
    }

    #[must_use]
    pub fn on_state<F>(mut self, handler: F) -> Self
    where
        F: FnMut(PeerId, SessionState) + Send + 'static,
    {
        self.on_state = Some(Box::new(handler));
        self
    }

    #[must_use]
    pub fn on_data<F>(mut self, handler: F) -> Self
    where
        F: FnMut(PeerId, Vec<u8>) + Send + 'static,
    {
        self.on_data = Some(Box::new(handler));
        self
    }

    #[must_use]
    pub fn on_stream<F>(mut self, handler: F) -> Self
    where
        F: FnMut(PeerId, String, InputStream) + Send + 'static,
    {
        self.on_stream = Some(Box::new(handler));
        self
    }

    #[must_use]
    pub fn on_resource_started<F>(mut self, handler: F) -> Self
    where
        F: FnMut(PeerId, String, ResourceTransfer) + Send + 'static,
    {
        self.on_resource_started = Some(Box::new(handler));
        self
    }

    #[must_use]
    pub fn on_resource_finished<F>(mut self, handler: F) -> Self
    where
        F: FnMut(PeerId, String, Option<PathBuf>, Option<FrameworkError>) + Send + 'static,
    {
        self.on_resource_finished = Some(Box::new(handler));
        self
    }

    #[must_use]
    pub fn on_certificate<F>(mut self, handler: F) -> Self
    where
        F: FnMut(PeerId, Vec<SecurityIdentityItem>) -> bool + Send + 'static,
    {
        self.on_certificate = Some(Box::new(handler));
        self
    }
}

impl Default for SessionDelegate {
    fn default() -> Self {
        Self::new()
    }
}

struct SessionDelegateState {
    callbacks: Mutex<SessionDelegate>,
}

struct ResourceSendCompletionState {
    callback: Option<Box<ResourceSendCompletionHandler>>,
}

pub struct Session {
    raw: NonNull<c_void>,
    delegate_state: Option<NonNull<SessionDelegateState>>,
}

impl Session {
    pub fn new(peer: &PeerId, encryption_preference: EncryptionPreference) -> Result<Self> {
        unsafe { Self::with_security_identity(peer, None, encryption_preference) }
    }

    pub fn with_security_identity_items(
        peer: &PeerId,
        security_identity: &[SecurityIdentityItem],
        encryption_preference: EncryptionPreference,
    ) -> Result<Self> {
        let handles: Vec<*mut c_void> = security_identity
            .iter()
            .map(SecurityIdentityItem::as_ptr)
            .collect();
        let mut error = ptr::null_mut();
        let raw = unsafe {
            ffi::session::mpc_session_create_with_identity_handles(
                peer.as_ptr(),
                if handles.is_empty() {
                    ptr::null()
                } else {
                    handles.as_ptr()
                },
                handles.len(),
                encryption_preference.as_raw(),
                &mut error,
            )
        };
        let raw = NonNull::new(raw).ok_or_else(|| take_error(error))?;
        Ok(Self {
            raw,
            delegate_state: None,
        })
    }

    pub unsafe fn with_security_identity(
        peer: &PeerId,
        security_identity: Option<&[*mut c_void]>,
        encryption_preference: EncryptionPreference,
    ) -> Result<Self> {
        let mut error = ptr::null_mut();
        let (identity_ptr, identity_len) =
            security_identity.map_or((ptr::null(), 0), |items| (items.as_ptr(), items.len()));
        let raw = ffi::session::mpc_session_create_with_identity(
            peer.as_ptr(),
            identity_ptr,
            identity_len,
            encryption_preference.as_raw(),
            &mut error,
        );
        let raw = NonNull::new(raw).ok_or_else(|| take_error(error))?;
        Ok(Self {
            raw,
            delegate_state: None,
        })
    }

    pub(crate) unsafe fn from_owned_raw(raw: *mut c_void) -> Self {
        Self {
            raw: NonNull::new(raw).expect("session raw pointer must not be null"),
            delegate_state: None,
        }
    }

    #[must_use]
    pub fn my_peer_id(&self) -> PeerId {
        let raw = unsafe { ffi::session::mpc_session_copy_my_peer(self.raw.as_ptr()) };
        unsafe { PeerId::from_owned_raw(raw) }
    }

    #[must_use]
    pub fn security_identity(&self) -> Vec<SecurityIdentityItem> {
        let mut array = ptr::null_mut();
        let mut count = 0usize;
        unsafe {
            ffi::session::mpc_session_copy_security_identity(
                self.raw.as_ptr(),
                &mut array,
                &mut count,
            );
        };
        take_handle_array(array, count, |raw| unsafe {
            SecurityIdentityItem::from_owned_raw(raw)
        })
    }

    #[must_use]
    pub fn encryption_preference(&self) -> EncryptionPreference {
        EncryptionPreference::from_raw(unsafe {
            ffi::session::mpc_session_encryption_preference(self.raw.as_ptr())
        })
    }

    #[must_use]
    pub fn connected_peers(&self) -> Vec<PeerId> {
        let mut array = ptr::null_mut();
        let mut count = 0usize;
        unsafe {
            ffi::session::mpc_session_copy_connected_peers(
                self.raw.as_ptr(),
                &mut array,
                &mut count,
            );
        };
        take_handle_array(array, count, |raw| unsafe { PeerId::from_owned_raw(raw) })
    }

    pub fn send(&self, data: &[u8], peers: &[&PeerId], mode: SessionSendDataMode) -> Result<()> {
        if peers.is_empty() {
            return Err(MultipeerError::InvalidArgument(
                "send requires at least one destination peer".into(),
            ));
        }
        let peer_ptrs: Vec<*mut c_void> = peers.iter().map(|peer| peer.as_ptr()).collect();
        let mut error = ptr::null_mut();
        let status = unsafe {
            ffi::session::mpc_session_send_data(
                self.raw.as_ptr(),
                data.as_ptr().cast::<c_void>(),
                data.len(),
                peer_ptrs.as_ptr(),
                peer_ptrs.len(),
                mode.as_raw(),
                &mut error,
            )
        };
        if status == ffi::core::MPC_OK {
            Ok(())
        } else {
            Err(take_error(error))
        }
    }

    pub fn send_resource(
        &self,
        path: impl AsRef<Path>,
        resource_name: impl AsRef<str>,
        peer: &PeerId,
    ) -> Result<ResourceTransfer> {
        self.send_resource_impl(path, resource_name, peer, ptr::null_mut(), None)
    }

    pub fn send_resource_with_completion<F>(
        &self,
        path: impl AsRef<Path>,
        resource_name: impl AsRef<str>,
        peer: &PeerId,
        on_complete: F,
    ) -> Result<ResourceTransfer>
    where
        F: FnOnce(Option<FrameworkError>) + Send + 'static,
    {
        let state = Box::new(ResourceSendCompletionState {
            callback: Some(Box::new(on_complete)),
        });
        let context = Box::into_raw(state).cast::<c_void>();
        match self.send_resource_impl(
            path,
            resource_name,
            peer,
            context,
            Some(resource_send_completion_trampoline),
        ) {
            Ok(progress) => Ok(progress),
            Err(error) => {
                unsafe { drop(Box::from_raw(context.cast::<ResourceSendCompletionState>())) };
                Err(error)
            }
        }
    }

    fn send_resource_impl(
        &self,
        path: impl AsRef<Path>,
        resource_name: impl AsRef<str>,
        peer: &PeerId,
        context: *mut c_void,
        completion: Option<ffi::session::ResourceSendCompletionCallback>,
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
            ffi::session::mpc_session_send_resource(
                self.raw.as_ptr(),
                path.as_ptr(),
                resource_name.as_ptr(),
                peer.as_ptr(),
                context,
                completion,
                &mut error,
            )
        };
        let raw = NonNull::new(raw).ok_or_else(|| take_error(error))?;
        Ok(ResourceTransfer { raw })
    }

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
            ffi::session::mpc_session_start_stream(
                self.raw.as_ptr(),
                stream_name.as_ptr(),
                peer.as_ptr(),
                &mut error,
            )
        };
        let raw = NonNull::new(raw).ok_or_else(|| take_error(error))?;
        Ok(OutputStream { raw })
    }

    pub fn disconnect(&self) {
        unsafe { ffi::session::mpc_session_disconnect(self.raw.as_ptr()) };
    }

    pub fn nearby_connection_data_for_peer(&self, peer: &PeerId) -> Result<Vec<u8>> {
        let mut bytes = ptr::null_mut();
        let mut length = 0usize;
        let mut error = ptr::null_mut();
        let status = unsafe {
            ffi::session::mpc_session_nearby_connection_data_for_peer(
                self.raw.as_ptr(),
                peer.as_ptr(),
                &mut bytes,
                &mut length,
                &mut error,
            )
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

    pub fn connect_peer(&self, peer: &PeerId, nearby_connection_data: &[u8]) {
        unsafe {
            ffi::session::mpc_session_connect_peer(
                self.raw.as_ptr(),
                peer.as_ptr(),
                nearby_connection_data.as_ptr().cast::<c_void>(),
                nearby_connection_data.len(),
            );
        };
    }

    pub fn cancel_connect_peer(&self, peer: &PeerId) {
        unsafe { ffi::session::mpc_session_cancel_connect_peer(self.raw.as_ptr(), peer.as_ptr()) };
    }

    pub fn set_delegate<F, G>(&mut self, on_state: F, on_data: G)
    where
        F: FnMut(PeerId, SessionState) + Send + 'static,
        G: FnMut(PeerId, Vec<u8>) + Send + 'static,
    {
        self.set_callbacks(SessionDelegate::new().on_state(on_state).on_data(on_data));
    }

    pub fn set_callbacks(&mut self, callbacks: SessionDelegate) {
        self.clear_delegate();
        let has_certificate = callbacks.on_certificate.is_some();
        let state = Box::new(SessionDelegateState {
            callbacks: Mutex::new(callbacks),
        });
        let ptr = NonNull::from(Box::leak(state));
        unsafe {
            ffi::session::mpc_session_set_delegate(
                self.raw.as_ptr(),
                ptr.as_ptr().cast::<c_void>(),
                Some(session_state_trampoline),
                Some(session_data_trampoline),
                Some(session_stream_trampoline),
                Some(session_resource_start_trampoline),
                Some(session_resource_finish_trampoline),
                if has_certificate {
                    Some(session_certificate_trampoline)
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
                ffi::session::mpc_session_clear_delegate(self.raw.as_ptr());
                drop(Box::from_raw(state.as_ptr()));
            }
        }
    }

    pub(crate) const fn as_ptr(&self) -> *mut c_void {
        self.raw.as_ptr()
    }
}

impl Clone for Session {
    fn clone(&self) -> Self {
        let raw = unsafe { ffi::core::mpc_object_retain(self.raw.as_ptr()) };
        unsafe { Self::from_owned_raw(raw) }
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        self.clear_delegate();
        unsafe { ffi::core::mpc_object_release(self.raw.as_ptr()) };
    }
}

impl fmt::Debug for Session {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Session")
            .field("my_peer_id", &self.my_peer_id())
            .field("encryption_preference", &self.encryption_preference())
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
        unsafe { ffi::session::mpc_progress_fraction_completed(self.raw.as_ptr()) }
    }

    #[must_use]
    pub fn is_finished(&self) -> bool {
        unsafe { ffi::session::mpc_progress_is_finished(self.raw.as_ptr()) }
    }

    #[must_use]
    pub fn completed_unit_count(&self) -> i64 {
        unsafe { ffi::session::mpc_progress_completed_unit_count(self.raw.as_ptr()) }
    }

    #[must_use]
    pub fn total_unit_count(&self) -> i64 {
        unsafe { ffi::session::mpc_progress_total_unit_count(self.raw.as_ptr()) }
    }

    pub(crate) unsafe fn from_owned_raw(raw: *mut c_void) -> Self {
        Self {
            raw: NonNull::new(raw).expect("progress raw pointer must not be null"),
        }
    }
}

impl Drop for ResourceTransfer {
    fn drop(&mut self) {
        unsafe { ffi::core::mpc_object_release(self.raw.as_ptr()) };
    }
}

impl fmt::Debug for ResourceTransfer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ResourceTransfer")
            .field("fraction_completed", &self.fraction_completed())
            .field("is_finished", &self.is_finished())
            .finish()
    }
}

pub struct OutputStream {
    raw: NonNull<c_void>,
}

impl OutputStream {
    pub fn open(&self) {
        unsafe { ffi::session::mpc_output_stream_open(self.raw.as_ptr()) };
    }

    pub fn close(&self) {
        unsafe { ffi::session::mpc_output_stream_close(self.raw.as_ptr()) };
    }

    pub fn write(&self, bytes: &[u8]) -> Result<usize> {
        let mut error = ptr::null_mut();
        let written = unsafe {
            ffi::session::mpc_output_stream_write(
                self.raw.as_ptr(),
                bytes.as_ptr().cast::<c_void>(),
                bytes.len(),
                &mut error,
            )
        };
        if written >= 0 {
            usize::try_from(written).map_err(|_| {
                MultipeerError::OperationFailed(
                    "NSOutputStream returned a byte count that could not fit in usize".into(),
                )
            })
        } else {
            Err(take_error(error))
        }
    }
}

impl Drop for OutputStream {
    fn drop(&mut self) {
        unsafe { ffi::core::mpc_object_release(self.raw.as_ptr()) };
    }
}

pub struct InputStream {
    raw: NonNull<c_void>,
}

impl InputStream {
    pub fn open(&self) {
        unsafe { ffi::session::mpc_input_stream_open(self.raw.as_ptr()) };
    }

    pub fn close(&self) {
        unsafe { ffi::session::mpc_input_stream_close(self.raw.as_ptr()) };
    }

    #[must_use]
    pub fn has_bytes_available(&self) -> bool {
        unsafe { ffi::session::mpc_input_stream_has_bytes_available(self.raw.as_ptr()) }
    }

    pub fn read(&self, max_len: usize) -> Result<Vec<u8>> {
        let mut buffer = vec![0_u8; max_len];
        let mut error = ptr::null_mut();
        let read = unsafe {
            ffi::session::mpc_input_stream_read(
                self.raw.as_ptr(),
                buffer.as_mut_ptr().cast::<c_void>(),
                buffer.len(),
                &mut error,
            )
        };
        if read < 0 {
            return Err(take_error(error));
        }
        buffer.truncate(usize::try_from(read).unwrap_or(0));
        Ok(buffer)
    }

    pub(crate) unsafe fn from_owned_raw(raw: *mut c_void) -> Self {
        Self {
            raw: NonNull::new(raw).expect("input stream raw pointer must not be null"),
        }
    }
}

impl Drop for InputStream {
    fn drop(&mut self) {
        unsafe { ffi::core::mpc_object_release(self.raw.as_ptr()) };
    }
}

impl fmt::Debug for InputStream {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("InputStream")
            .field("has_bytes_available", &self.has_bytes_available())
            .finish()
    }
}

#[must_use]
pub fn session_minimum_number_of_peers() -> usize {
    unsafe { ffi::session::mpc_session_minimum_number_of_peers() }
}

#[must_use]
pub fn session_maximum_number_of_peers() -> usize {
    unsafe { ffi::session::mpc_session_maximum_number_of_peers() }
}

fn take_handle_array<T>(
    array: *mut c_void,
    count: usize,
    mut make: impl FnMut(*mut c_void) -> T,
) -> Vec<T> {
    if array.is_null() || count == 0 {
        return Vec::new();
    }
    let items = unsafe { std::slice::from_raw_parts(array.cast::<*mut c_void>(), count) };
    let values = items
        .iter()
        .copied()
        .filter(|ptr| !ptr.is_null())
        .map(&mut make)
        .collect();
    unsafe { ffi::core::mpc_ptr_array_free(array) };
    values
}

unsafe extern "C" fn resource_send_completion_trampoline(context: *mut c_void, error: *mut c_void) {
    let Some(context) = NonNull::new(context.cast::<ResourceSendCompletionState>()) else {
        if !error.is_null() {
            let _ = take_framework_error(error);
        }
        return;
    };
    let mut state = unsafe { Box::from_raw(context.as_ptr()) };
    let error = take_optional_framework_error(error);
    if let Some(callback) = state.callback.take() {
        catch_user_panic("resource_send_completion_trampoline", || callback(error));
    }
}

unsafe extern "C" fn session_state_trampoline(context: *mut c_void, peer: *mut c_void, state: i32) {
    let Some(context) = NonNull::new(context.cast::<SessionDelegateState>()) else {
        return;
    };
    let peer = unsafe { PeerId::from_owned_raw(peer) };
    if let Ok(mut callbacks) = unsafe { context.as_ref() }.callbacks.lock() {
        if let Some(callback) = callbacks.on_state.as_mut() {
            let state = SessionState::from_raw(state);
            catch_user_panic("session_state_trampoline", || callback(peer, state));
        }
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
    let peer = unsafe { PeerId::from_owned_raw(peer) };
    let payload = if data.is_null() || length == 0 {
        Vec::new()
    } else {
        unsafe { std::slice::from_raw_parts(data.cast::<u8>(), length) }.to_vec()
    };
    if let Ok(mut callbacks) = unsafe { context.as_ref() }.callbacks.lock() {
        if let Some(callback) = callbacks.on_data.as_mut() {
            catch_user_panic("session_data_trampoline", || callback(peer, payload));
        }
    }
}

unsafe extern "C" fn session_stream_trampoline(
    context: *mut c_void,
    peer: *mut c_void,
    stream_name: *const c_char,
    stream: *mut c_void,
) {
    let Some(context) = NonNull::new(context.cast::<SessionDelegateState>()) else {
        return;
    };
    let peer = unsafe { PeerId::from_owned_raw(peer) };
    let name = unsafe { CStr::from_ptr(stream_name) }
        .to_string_lossy()
        .into_owned();
    let stream = unsafe { InputStream::from_owned_raw(stream) };
    if let Ok(mut callbacks) = unsafe { context.as_ref() }.callbacks.lock() {
        if let Some(callback) = callbacks.on_stream.as_mut() {
            catch_user_panic("session_stream_trampoline", || callback(peer, name, stream));
        }
    }
}

unsafe extern "C" fn session_resource_start_trampoline(
    context: *mut c_void,
    peer: *mut c_void,
    resource_name: *const c_char,
    progress: *mut c_void,
) {
    let Some(context) = NonNull::new(context.cast::<SessionDelegateState>()) else {
        return;
    };
    let peer = unsafe { PeerId::from_owned_raw(peer) };
    let name = unsafe { CStr::from_ptr(resource_name) }
        .to_string_lossy()
        .into_owned();
    let progress = unsafe { ResourceTransfer::from_owned_raw(progress) };
    if let Ok(mut callbacks) = unsafe { context.as_ref() }.callbacks.lock() {
        if let Some(callback) = callbacks.on_resource_started.as_mut() {
            catch_user_panic("session_resource_start_trampoline", || {
                callback(peer, name, progress);
            });
        }
    }
}

unsafe extern "C" fn session_resource_finish_trampoline(
    context: *mut c_void,
    peer: *mut c_void,
    resource_name: *const c_char,
    local_path: *const c_char,
    error: *mut c_void,
) {
    let Some(context) = NonNull::new(context.cast::<SessionDelegateState>()) else {
        if !error.is_null() {
            let _ = take_framework_error(error);
        }
        return;
    };
    let peer = unsafe { PeerId::from_owned_raw(peer) };
    let name = unsafe { CStr::from_ptr(resource_name) }
        .to_string_lossy()
        .into_owned();
    let path = if local_path.is_null() {
        None
    } else {
        Some(PathBuf::from(
            unsafe { CStr::from_ptr(local_path) }
                .to_string_lossy()
                .into_owned(),
        ))
    };
    let error = take_optional_framework_error(error);
    if let Ok(mut callbacks) = unsafe { context.as_ref() }.callbacks.lock() {
        if let Some(callback) = callbacks.on_resource_finished.as_mut() {
            catch_user_panic("session_resource_finish_trampoline", || {
                callback(peer, name, path, error);
            });
        }
    }
}

unsafe extern "C" fn session_certificate_trampoline(
    context: *mut c_void,
    peer: *mut c_void,
    certificate_items: *mut c_void,
    certificate_count: usize,
) -> bool {
    let Some(context) = NonNull::new(context.cast::<SessionDelegateState>()) else {
        if !certificate_items.is_null() {
            unsafe { ffi::core::mpc_ptr_array_free(certificate_items) };
        }
        return false;
    };
    let peer = unsafe { PeerId::from_owned_raw(peer) };
    let certificate = take_handle_array(certificate_items, certificate_count, |raw| unsafe {
        SecurityIdentityItem::from_owned_raw(raw)
    });
    let mut result = false;
    if let Ok(mut callbacks) = unsafe { context.as_ref() }.callbacks.lock() {
        if let Some(callback) = callbacks.on_certificate.as_mut() {
            catch_user_panic("session_certificate_trampoline", || {
                result = callback(peer, certificate);
            });
        }
    }
    result
}
