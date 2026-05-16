//! Errors returned by the `multipeerconnectivity` crate.

use core::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum MultipeerError {
    InvalidArgument(String),
    OperationFailed(String),
}

impl fmt::Display for MultipeerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidArgument(message) => write!(f, "invalid argument: {message}"),
            Self::OperationFailed(message) => write!(f, "operation failed: {message}"),
        }
    }
}

impl std::error::Error for MultipeerError {}

pub type Result<T> = std::result::Result<T, MultipeerError>;
