use std::collections::HashMap;

use multipeerconnectivity::prelude::*;

fn main() -> Result<()> {
    let peer = PeerId::new("doom-fish-assistant")?;
    let session = Session::new(&peer, EncryptionPreference::Optional)?;
    let mut discovery = HashMap::new();
    discovery.insert("mode".to_string(), "assistant".to_string());
    let assistant = AdvertiserAssistant::new("doom-chat", Some(&discovery), &session)?;

    println!("assistant service type: {}", assistant.service_type());
    println!(
        "assistant session peer: {}",
        assistant.session().my_peer_id().display_name()
    );
    Ok(())
}
