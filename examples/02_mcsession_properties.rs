use multipeerconnectivity::prelude::*;

fn main() -> Result<()> {
    let peer = PeerId::new("doom-fish-session")?;
    let session = Session::new(&peer, EncryptionPreference::Optional)?;

    println!("session peer: {}", session.my_peer_id().display_name());
    println!(
        "session peers min/max: {}/{}",
        session_minimum_number_of_peers(),
        session_maximum_number_of_peers()
    );
    println!("connected peers: {}", session.connected_peers().len());
    Ok(())
}
