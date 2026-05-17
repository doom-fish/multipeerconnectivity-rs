use multipeerconnectivity::prelude::*;

fn main() -> Result<()> {
    let peer = PeerId::new("doom-fish-ui")?;
    let session = Session::new(&peer, EncryptionPreference::Optional)?;
    let controller = BrowserViewController::new_with_service_type("doom-chat", &session)?;
    controller.set_minimum_number_of_peers(session_minimum_number_of_peers());
    controller.set_maximum_number_of_peers(session_maximum_number_of_peers());

    println!(
        "controller browser service: {}",
        controller.browser().service_type()
    );
    println!(
        "controller min/max: {}/{}",
        controller.minimum_number_of_peers(),
        controller.maximum_number_of_peers()
    );
    Ok(())
}
