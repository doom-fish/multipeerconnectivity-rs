use multipeerconnectivity::{
    session_maximum_number_of_peers, session_minimum_number_of_peers, BrowserViewController,
    EncryptionPreference, PeerId, Session, Result,
};

#[test]
#[ignore = "MCBrowserViewController is UI-driven; run manually from a main-thread harness"]
fn browser_view_controller_roundtrips_properties() -> Result<()> {
    let peer = PeerId::new("doom-fish-ui")?;
    let session = Session::new(&peer, EncryptionPreference::Optional)?;
    let controller = BrowserViewController::new_with_service_type("doom-chat", &session)?;

    controller.set_minimum_number_of_peers(session_minimum_number_of_peers());
    controller.set_maximum_number_of_peers(session_maximum_number_of_peers());
    assert_eq!(controller.browser().service_type(), "doom-chat");
    assert_eq!(controller.session().my_peer_id().display_name(), "doom-fish-ui");
    assert!(controller.maximum_number_of_peers() >= controller.minimum_number_of_peers());
    Ok(())
}
