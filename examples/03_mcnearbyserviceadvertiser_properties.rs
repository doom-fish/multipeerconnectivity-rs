use std::collections::HashMap;

use multipeerconnectivity::prelude::*;

fn main() -> Result<()> {
    let peer = PeerId::new("doom-fish-advertiser")?;
    let mut discovery = HashMap::new();
    discovery.insert("role".to_string(), "host".to_string());
    let advertiser = NearbyServiceAdvertiser::new(&peer, Some(&discovery), "doom-chat")?;

    println!("advertiser peer: {}", advertiser.my_peer_id().display_name());
    println!("advertiser service type: {}", advertiser.service_type());
    println!(
        "advertiser discovery keys: {}",
        advertiser.discovery_info().unwrap_or_default().len()
    );
    Ok(())
}
