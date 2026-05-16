use std::collections::HashMap;

use multipeerconnectivity::{EncryptionPreference, NearbyServiceAdvertiser, PeerId, Session, Result};

#[test]
fn advertiser_creation_and_delegate_setup_work() -> Result<()> {
    let peer = PeerId::new("doom-fish-advertiser")?;
    let session = Session::new(&peer, EncryptionPreference::Optional)?;
    let mut discovery = HashMap::new();
    discovery.insert("role".to_string(), "host".to_string());
    let mut advertiser = NearbyServiceAdvertiser::new(&peer, Some(&discovery), "doom-chat")?;

    assert_eq!(advertiser.my_peer_id().display_name(), "doom-fish-advertiser");
    assert_eq!(advertiser.service_type(), "doom-chat");
    assert_eq!(advertiser.discovery_info().unwrap().get("role"), Some(&"host".to_string()));
    advertiser.set_delegate(Some(&session), |_peer, _context| false);
    advertiser.clear_delegate();
    Ok(())
}
