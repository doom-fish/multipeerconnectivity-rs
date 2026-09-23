use std::collections::HashMap;

use multipeerconnectivity::{
    InvitationResponse, MultipeerError, NearbyServiceAdvertiser, NearbyServiceAdvertiserDelegate,
    PeerId, Result,
};

#[test]
fn advertiser_creation_and_delegate_setup_work() -> Result<()> {
    let peer = PeerId::new("doom-fish-advertiser")?;
    let mut discovery = HashMap::new();
    discovery.insert("role".to_string(), "host".to_string());
    let mut advertiser = NearbyServiceAdvertiser::new(&peer, Some(&discovery), "doom-chat")?;

    assert_eq!(
        advertiser.my_peer_id().display_name(),
        "doom-fish-advertiser"
    );
    assert_eq!(advertiser.service_type(), "doom-chat");
    assert_eq!(
        advertiser.discovery_info().unwrap().get("role"),
        Some(&"host".to_string())
    );
    advertiser.set_callbacks(
        NearbyServiceAdvertiserDelegate::new()
            .on_invitation(|_peer, _context| InvitationResponse::Decline)
            .on_error(|_error| {}),
    );
    advertiser.clear_delegate();
    Ok(())
}

#[test]
fn advertiser_rejects_inputs_the_framework_would_abort_on() -> Result<()> {
    let peer = PeerId::new("doom-fish-advertiser-invalid")?;
    for service_type in ["-chat", "chat-", "doom--chat", "1234"] {
        let error =
            NearbyServiceAdvertiser::new(&peer, None, service_type).expect_err(service_type);
        assert!(
            matches!(error, MultipeerError::InvalidArgument(_)),
            "{service_type}: {error}"
        );
    }
    for (key, value) in [
        ("", "v".to_string()),
        ("a=b", "v".to_string()),
        ("caf\u{e9}", "v".to_string()),
        ("k", "v".repeat(253)),
    ] {
        let info = HashMap::from([(key.to_string(), value)]);
        let error = NearbyServiceAdvertiser::new(&peer, Some(&info), "doom-chat").expect_err(key);
        assert!(
            matches!(error, MultipeerError::InvalidArgument(_)),
            "{key:?}: {error}"
        );
    }
    let info = HashMap::from([("k".to_string(), "v".repeat(252))]);
    let advertiser = NearbyServiceAdvertiser::new(&peer, Some(&info), "doom-chat")?;
    assert_eq!(advertiser.discovery_info(), Some(info));
    Ok(())
}
