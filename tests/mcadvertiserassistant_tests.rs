use std::collections::HashMap;

use multipeerconnectivity::{
    AdvertiserAssistant, AdvertiserAssistantDelegate, EncryptionPreference, PeerId, Result, Session,
};

#[test]
#[ignore = "MCAdvertiserAssistant is a UI convenience type; run manually from a main-thread harness"]
fn advertiser_assistant_exposes_properties() -> Result<()> {
    let peer = PeerId::new("doom-fish-assistant")?;
    let session = Session::new(&peer, EncryptionPreference::Optional)?;
    let mut discovery = HashMap::new();
    discovery.insert("mode".to_string(), "assistant".to_string());
    let mut assistant = AdvertiserAssistant::new("doom-chat", Some(&discovery), &session)?;

    assert_eq!(assistant.service_type(), "doom-chat");
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
    Ok(())
}
