//! Async stream subscription lifecycle tests.
#![cfg(feature = "async")]

use multipeerconnectivity::async_api::{
    AdvertiserEventStream, BrowserEventStream, InvitationHandle, SessionEventStream,
};
use multipeerconnectivity::{
    CertificatePolicy, CertificateRequest, EncryptionPreference, NearbyServiceAdvertiser,
    NearbyServiceBrowser, PeerId, Session,
};

#[test]
fn session_stream_subscribe_and_drop() {
    let peer = PeerId::new("test-session-stream").expect("peer creation must succeed");
    let session = Session::new(
        &peer,
        EncryptionPreference::Required,
        CertificatePolicy::Verify(Box::new(CertificateRequest::reject)),
    )
    .expect("session creation must succeed");
    let stream = SessionEventStream::subscribe_default(&session);
    assert!(!stream.is_closed(), "stream must be open after subscribe");
    assert_eq!(stream.buffered_count(), 0, "no events on idle stream");
    drop(stream);
    let _ = session.connected_peers();
}

#[test]
fn session_stream_closes_on_drop() {
    let peer = PeerId::new("test-session-close").expect("peer creation must succeed");
    let session = Session::new(
        &peer,
        EncryptionPreference::Required,
        CertificatePolicy::Verify(Box::new(CertificateRequest::reject)),
    )
    .expect("session creation must succeed");
    let stream = SessionEventStream::subscribe_default(&session);
    drop(stream);
}

#[test]
fn session_stream_custom_capacity() {
    let peer = PeerId::new("test-session-cap").expect("peer creation must succeed");
    let session = Session::new(
        &peer,
        EncryptionPreference::Required,
        CertificatePolicy::Verify(Box::new(CertificateRequest::reject)),
    )
    .expect("session creation must succeed");
    let stream = SessionEventStream::subscribe(&session, 128);
    assert!(!stream.is_closed());
    drop(stream);
}

#[test]
fn browser_stream_subscribe_and_drop() {
    let peer = PeerId::new("test-browser-stream").expect("peer creation must succeed");
    let browser =
        NearbyServiceBrowser::new(&peer, "test-svc").expect("browser creation must succeed");
    let stream = BrowserEventStream::subscribe_default(&browser);
    assert!(!stream.is_closed(), "stream must be open after subscribe");
    assert_eq!(stream.buffered_count(), 0);
    drop(stream);
    let _ = browser.service_type();
}

#[test]
fn advertiser_stream_subscribe_and_drop() {
    let peer = PeerId::new("test-advertiser-stream").expect("peer creation must succeed");
    let advertiser = NearbyServiceAdvertiser::new(&peer, None, "test-svc")
        .expect("advertiser creation must succeed");
    let stream = AdvertiserEventStream::subscribe_default(&advertiser);
    assert!(!stream.is_closed(), "stream must be open after subscribe");
    assert_eq!(stream.buffered_count(), 0);
    drop(stream);
    let _ = advertiser.service_type();
}

#[test]
fn invitation_handle_decline_on_drop() {
    let handle = InvitationHandle::default();
    drop(handle);
}

#[test]
fn async_payload_ffi_layout_matches_swift() {
    assert!(
        multipeerconnectivity::async_api::verify_ffi_layout(),
        "Swift async payload layout disagrees with Rust layout (ABI mismatch)"
    );
}
