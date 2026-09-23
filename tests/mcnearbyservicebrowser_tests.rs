use multipeerconnectivity::{
    MultipeerError, NearbyServiceBrowser, NearbyServiceBrowserDelegate, PeerId, Result,
};

#[test]
fn browser_creation_and_delegate_setup_work() -> Result<()> {
    let peer = PeerId::new("doom-fish-browser")?;
    let mut browser = NearbyServiceBrowser::new(&peer, "doom-chat")?;

    assert_eq!(browser.my_peer_id().display_name(), "doom-fish-browser");
    assert_eq!(browser.service_type(), "doom-chat");
    browser.set_callbacks(
        NearbyServiceBrowserDelegate::new()
            .on_found(|_peer, _info| {})
            .on_lost(|_peer| {})
            .on_error(|_error| {}),
    );
    browser.clear_delegate();
    Ok(())
}

#[test]
fn browser_rejects_service_types_the_framework_would_abort_on() -> Result<()> {
    let peer = PeerId::new("doom-fish-browser-invalid")?;
    for service_type in [
        "",
        "-chat",
        "chat-",
        "doom--chat",
        "1234",
        "doom_chat",
        "a-very-long-service",
    ] {
        let error = NearbyServiceBrowser::new(&peer, service_type).expect_err(service_type);
        assert!(
            matches!(error, MultipeerError::InvalidArgument(_)),
            "{service_type:?}: {error}"
        );
    }
    Ok(())
}
