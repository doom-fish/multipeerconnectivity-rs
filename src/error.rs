#![allow(clippy::missing_const_for_fn)]

//! Errors returned by the `multipeerconnectivity` crate.

use core::ffi::{c_char, c_void};
use core::fmt;

use crate::ffi;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
/// Represents a `MultipeerConnectivity` `MCErrorCode`.
pub enum MCErrorCode {
    /// Represents an unknown `MultipeerConnectivity` error.
    Unknown,
    /// Represents a `MultipeerConnectivity` not-connected error.
    NotConnected,
    /// Represents a `MultipeerConnectivity` invalid-parameter error.
    InvalidParameter,
    /// Represents a `MultipeerConnectivity` unsupported error.
    Unsupported,
    /// Represents a `MultipeerConnectivity` timeout error.
    TimedOut,
    /// Represents a `MultipeerConnectivity` cancelled error.
    Cancelled,
    /// Represents a `MultipeerConnectivity` unavailable error.
    Unavailable,
    /// Preserves an unmapped `MultipeerConnectivity` error code.
    Other(i32),
}

impl MCErrorCode {
    #[must_use]
    /// Converts a raw `MultipeerConnectivity` error code.
    pub const fn from_raw(value: i32) -> Self {
        match value {
            0 => Self::Unknown,
            1 => Self::NotConnected,
            2 => Self::InvalidParameter,
            3 => Self::Unsupported,
            4 => Self::TimedOut,
            5 => Self::Cancelled,
            6 => Self::Unavailable,
            other => Self::Other(other),
        }
    }

    #[must_use]
    /// Converts this `MultipeerConnectivity` error code to its raw value.
    pub const fn as_raw(self) -> i32 {
        match self {
            Self::Unknown => 0,
            Self::NotConnected => 1,
            Self::InvalidParameter => 2,
            Self::Unsupported => 3,
            Self::TimedOut => 4,
            Self::Cancelled => 5,
            Self::Unavailable => 6,
            Self::Other(other) => other,
        }
    }
}

impl fmt::Display for MCErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unknown => write!(f, "Unknown"),
            Self::NotConnected => write!(f, "NotConnected"),
            Self::InvalidParameter => write!(f, "InvalidParameter"),
            Self::Unsupported => write!(f, "Unsupported"),
            Self::TimedOut => write!(f, "TimedOut"),
            Self::Cancelled => write!(f, "Cancelled"),
            Self::Unavailable => write!(f, "Unavailable"),
            Self::Other(value) => write!(f, "Other({value})"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Stores details from a `MultipeerConnectivity` framework error.
pub struct FrameworkError {
    domain: String,
    code: i32,
    description: String,
}

impl FrameworkError {
    #[must_use]
    /// Creates a `MultipeerConnectivity` framework error value.
    pub fn new(domain: String, code: i32, description: String) -> Self {
        Self {
            domain,
            code,
            description,
        }
    }

    #[must_use]
    /// Returns the `MultipeerConnectivity` error domain.
    pub fn domain(&self) -> &str {
        &self.domain
    }

    #[must_use]
    /// Returns the raw `MultipeerConnectivity` error code.
    pub const fn code(&self) -> i32 {
        self.code
    }

    #[must_use]
    /// Returns the `MultipeerConnectivity` error description.
    pub fn description(&self) -> &str {
        &self.description
    }

    #[must_use]
    /// Returns the mapped `MultipeerConnectivity` `MCErrorCode`, if available.
    pub fn mc_error_code(&self) -> Option<MCErrorCode> {
        if self.domain == mc_error_domain() {
            Some(MCErrorCode::from_raw(self.code))
        } else {
            None
        }
    }
}

impl fmt::Display for FrameworkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(code) = self.mc_error_code() {
            write!(f, "{} ({code}): {}", self.domain, self.description)
        } else {
            write!(f, "{} ({}): {}", self.domain, self.code, self.description)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
/// Represents errors surfaced by these `MultipeerConnectivity` wrappers.
pub enum MultipeerError {
    /// Represents an invalid argument passed to a `MultipeerConnectivity` wrapper.
    InvalidArgument(String),
    /// Represents an internal `MultipeerConnectivity` wrapper operation failure.
    OperationFailed(String),
    /// Represents a framework error reported by `MultipeerConnectivity`.
    Framework(FrameworkError),
}

impl fmt::Display for MultipeerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidArgument(message) => write!(f, "invalid argument: {message}"),
            Self::OperationFailed(message) => write!(f, "operation failed: {message}"),
            Self::Framework(error) => write!(f, "framework error: {error}"),
        }
    }
}

impl std::error::Error for MultipeerError {}

/// Alias for results returned by these `MultipeerConnectivity` wrappers.
pub type Result<T> = std::result::Result<T, MultipeerError>;

pub(crate) fn copy_and_free_string(ptr: *mut c_char) -> String {
    if ptr.is_null() {
        return String::new();
    }
    let value = unsafe { std::ffi::CStr::from_ptr(ptr) }
        .to_string_lossy()
        .into_owned();
    unsafe { ffi::core::mpc_string_free(ptr) };
    value
}

struct BridgeErrorInfo {
    kind: i32,
    domain: String,
    code: i32,
    description: String,
}

fn take_bridge_error_info(ptr: *mut c_void) -> BridgeErrorInfo {
    if ptr.is_null() {
        return BridgeErrorInfo {
            kind: ffi::error::MPC_ERROR_KIND_OPERATION_FAILED,
            domain: String::new(),
            code: 0,
            description: "unknown MultipeerConnectivity failure".into(),
        };
    }

    let kind = unsafe { ffi::error::mpc_error_kind(ptr) };
    let code = unsafe { ffi::error::mpc_error_code(ptr) };
    let domain = copy_and_free_string(unsafe { ffi::error::mpc_error_domain(ptr) });
    let description = copy_and_free_string(unsafe { ffi::error::mpc_error_description(ptr) });
    unsafe { ffi::core::mpc_object_release(ptr) };

    BridgeErrorInfo {
        kind,
        domain,
        code,
        description,
    }
}

pub(crate) fn take_error(ptr: *mut c_void) -> MultipeerError {
    let info = take_bridge_error_info(ptr);
    match info.kind {
        ffi::error::MPC_ERROR_KIND_INVALID_ARGUMENT => {
            MultipeerError::InvalidArgument(info.description)
        }
        ffi::error::MPC_ERROR_KIND_FRAMEWORK => MultipeerError::Framework(FrameworkError::new(
            info.domain,
            info.code,
            info.description,
        )),
        _ => MultipeerError::OperationFailed(info.description),
    }
}

pub(crate) fn take_framework_error(ptr: *mut c_void) -> FrameworkError {
    let info = take_bridge_error_info(ptr);
    FrameworkError::new(info.domain, info.code, info.description)
}

pub(crate) fn take_optional_framework_error(ptr: *mut c_void) -> Option<FrameworkError> {
    if ptr.is_null() {
        None
    } else {
        Some(take_framework_error(ptr))
    }
}

#[must_use]
/// Returns the `MultipeerConnectivity` `MCErrorDomain` string.
pub fn mc_error_domain() -> String {
    copy_and_free_string(unsafe { ffi::error::mpc_mc_error_domain() })
}
