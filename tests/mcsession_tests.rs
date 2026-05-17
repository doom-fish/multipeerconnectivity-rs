use multipeerconnectivity::{
    session_maximum_number_of_peers, session_minimum_number_of_peers, EncryptionPreference, PeerId,
    Result, Session, SessionDelegate,
};

#[test]
fn session_exposes_properties_and_delegate_setup() -> Result<()> {
    let peer = PeerId::new("doom-fish-session")?;
    let mut session = Session::new(&peer, EncryptionPreference::Optional)?;
    assert_eq!(session.my_peer_id().display_name(), "doom-fish-session");
    assert_eq!(session.security_identity().len(), 0);
    assert_eq!(
        session.encryption_preference(),
        EncryptionPreference::Optional
    );
    assert!(session.connected_peers().is_empty());
    session.set_callbacks(SessionDelegate::new());
    session.clear_delegate();
    Ok(())
}

#[test]
fn session_reports_peer_limits() {
    assert!(session_minimum_number_of_peers() >= 2);
    assert!(session_maximum_number_of_peers() >= session_minimum_number_of_peers());
}
