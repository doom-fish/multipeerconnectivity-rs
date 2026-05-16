use multipeerconnectivity::prelude::*;

fn main() -> Result<()> {
    let peer = PeerId::new("doom-fish-peer")?;
    let archived = peer.archived_data()?;
    let decoded = PeerId::from_archived_data(&archived)?;

    println!("peer display name: {}", decoded.display_name());
    println!("peer archive bytes: {}", archived.len());
    Ok(())
}
