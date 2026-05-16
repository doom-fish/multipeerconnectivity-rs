use multipeerconnectivity::prelude::*;

fn main() -> Result<()> {
    let peer = PeerId::new("doom-fish-smoke")?;
    let _session = Session::new(&peer, EncryptionPreference::Optional)?;
    let _browser = NearbyServiceBrowser::new(&peer, "doomfish-smoke")?;

    println!("peer display name: {}", peer.display_name());
    println!("✅ multipeer peer + session OK");
    Ok(())
}
