use multipeerconnectivity::prelude::*;

fn main() -> Result<()> {
    let peer = PeerId::new("doom-fish-browser")?;
    let browser = NearbyServiceBrowser::new(&peer, "doom-chat")?;

    println!("browser peer: {}", browser.my_peer_id().display_name());
    println!("browser service type: {}", browser.service_type());
    Ok(())
}
