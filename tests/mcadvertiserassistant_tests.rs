use std::collections::HashMap;

use multipeerconnectivity::{
    AdvertiserAssistant, AdvertiserAssistantDelegate, CertificatePolicy, CertificateRequest,
    EncryptionPreference, MultipeerError, PeerId, Result, Session,
};

fn session() -> Result<Session> {
    let peer = PeerId::new("doom-fish-assistant")?;
    Session::new(
        &peer,
        EncryptionPreference::Required,
        CertificatePolicy::Verify(Box::new(CertificateRequest::reject)),
    )
}

fn main() -> Result<()> {
    let off_main = std::thread::spawn(|| {
        let session = session()?;
        Ok::<_, MultipeerError>(
            AdvertiserAssistant::new("doom-chat", None, &session)
                .is_err_and(|error| matches!(error, MultipeerError::MainThreadRequired(_))),
        )
    })
    .join()
    .expect("off-main thread")?;
    assert!(
        off_main,
        "off-main construction must fail with MainThreadRequired"
    );

    let session = session()?;
    let mut discovery = HashMap::new();
    discovery.insert("mode".to_string(), "assistant".to_string());
    let mut assistant = AdvertiserAssistant::new("doom-chat", Some(&discovery), &session)?;

    assert_eq!(assistant.service_type(), "doom-chat");
    assert_eq!(assistant.discovery_info(), Some(discovery));
    assert_eq!(
        assistant.session().my_peer_id().display_name(),
        "doom-fish-assistant"
    );
    assistant.set_callbacks(
        AdvertiserAssistantDelegate::new()
            .on_will_present_invitation(|| {})
            .on_did_dismiss_invitation(|| {}),
    );
    assistant.clear_delegate();
    println!("advertiser assistant checks passed on the main thread");
    Ok(())
}
