use multipeerconnectivity::{
    session_maximum_number_of_peers, session_minimum_number_of_peers, BrowserViewController,
    BrowserViewControllerDelegate, CertificatePolicy, CertificateRequest, EncryptionPreference,
    MultipeerError, NearbyServiceBrowser, PeerId, Result, Session,
};

fn session() -> Result<Session> {
    let peer = PeerId::new("doom-fish-ui")?;
    Session::new(
        &peer,
        EncryptionPreference::Required,
        CertificatePolicy::Verify(Box::new(CertificateRequest::reject)),
    )
}

fn main() -> Result<()> {
    let off_main = std::thread::spawn(|| {
        let session = session()?;
        let browser = NearbyServiceBrowser::new(&session.my_peer_id(), "doom-chat")?;
        let is_main_thread_error =
            |error: MultipeerError| matches!(error, MultipeerError::MainThreadRequired(_));
        Ok::<_, MultipeerError>((
            BrowserViewController::new_with_service_type("doom-chat", &session)
                .is_err_and(is_main_thread_error),
            BrowserViewController::new_with_browser(&browser, &session)
                .is_err_and(is_main_thread_error),
        ))
    })
    .join()
    .expect("off-main thread")?;
    assert_eq!(off_main, (true, true), "off-main construction must fail");

    let session = session()?;
    let mut controller = BrowserViewController::new_with_service_type("doom-chat", &session)?;

    controller.set_callbacks(
        BrowserViewControllerDelegate::new()
            .on_finish(|| {})
            .on_cancel(|| {})
            .should_present_peer(|_peer, _info| true),
    );
    controller.set_minimum_number_of_peers(session_minimum_number_of_peers());
    controller.set_maximum_number_of_peers(session_maximum_number_of_peers());
    assert_eq!(controller.browser().service_type(), "doom-chat");
    assert_eq!(
        controller.session().my_peer_id().display_name(),
        "doom-fish-ui"
    );
    assert_eq!(
        controller.minimum_number_of_peers(),
        session_minimum_number_of_peers()
    );
    assert_eq!(
        controller.maximum_number_of_peers(),
        session_maximum_number_of_peers()
    );
    controller.clear_delegate();

    let browser = NearbyServiceBrowser::new(&session.my_peer_id(), "doom-chat")?;
    let from_browser = BrowserViewController::new_with_browser(&browser, &session)?;
    assert_eq!(from_browser.browser().service_type(), "doom-chat");
    println!("browser view controller checks passed on the main thread");
    Ok(())
}
