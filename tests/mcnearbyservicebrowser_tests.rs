use multipeerconnectivity::{NearbyServiceBrowser, NearbyServiceBrowserDelegate, PeerId, Result};

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
