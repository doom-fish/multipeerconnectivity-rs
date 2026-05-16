#![doc = include_str!("../README.md")]
#![cfg_attr(docsrs, feature(doc_cfg))]

#[cfg(all(not(target_os = "macos"), not(docsrs)))]
compile_error!("multipeerconnectivity only supports macOS");

pub mod advertiser;
pub mod advertiser_assistant;
pub mod browser;
pub mod browser_view_controller;
pub mod error;
pub mod ffi;
pub mod peer;
pub mod session;

pub use advertiser::{InvitationResponse, NearbyServiceAdvertiser, NearbyServiceAdvertiserDelegate};
pub use advertiser_assistant::{AdvertiserAssistant, AdvertiserAssistantDelegate};
pub use browser::{NearbyServiceBrowser, NearbyServiceBrowserDelegate};
pub use browser_view_controller::{BrowserViewController, BrowserViewControllerDelegate};
pub use error::{mc_error_domain, FrameworkError, MCErrorCode, MultipeerError, Result};
pub use peer::PeerId;
pub use session::{
    session_maximum_number_of_peers, session_minimum_number_of_peers, EncryptionPreference,
    InputStream, OutputStream, ResourceTransfer, SecurityIdentityItem, Session, SessionDelegate,
    SessionSendDataMode, SessionState,
};

pub mod prelude {
    pub use crate::{
        mc_error_domain, session_maximum_number_of_peers, session_minimum_number_of_peers,
        AdvertiserAssistant, AdvertiserAssistantDelegate, BrowserViewController,
        BrowserViewControllerDelegate, EncryptionPreference, FrameworkError, InputStream,
        InvitationResponse, MCErrorCode, MultipeerError, NearbyServiceAdvertiser,
        NearbyServiceAdvertiserDelegate, NearbyServiceBrowser, NearbyServiceBrowserDelegate,
        OutputStream, PeerId, ResourceTransfer, Result, SecurityIdentityItem, Session,
        SessionDelegate, SessionSendDataMode, SessionState,
    };
}
