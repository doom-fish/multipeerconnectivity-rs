#![doc = include_str!("../README.md")]
#![cfg_attr(docsrs, feature(doc_cfg))]

#[cfg(all(not(target_os = "macos"), not(docsrs)))]
compile_error!("multipeerconnectivity only supports macOS");

pub mod advertiser;
pub mod browser;
pub mod error;
pub mod ffi;
pub mod peer;
pub mod session;

pub use advertiser::NearbyServiceAdvertiser;
pub use browser::NearbyServiceBrowser;
pub use error::{MultipeerError, Result};
pub use peer::PeerId;
pub use session::{
    EncryptionPreference, OutputStream, ResourceTransfer, Session, SessionSendDataMode,
    SessionState,
};

pub mod prelude {
    pub use crate::{
        EncryptionPreference, MultipeerError, NearbyServiceAdvertiser, NearbyServiceBrowser,
        OutputStream, PeerId, ResourceTransfer, Result, Session, SessionSendDataMode,
        SessionState,
    };
}
