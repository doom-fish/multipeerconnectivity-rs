//! Example: subscribe to `MCNearbyServiceAdvertiser` events as an async stream.
#[cfg(feature = "async")]
use multipeerconnectivity::async_api::AdvertiserEventStream;
#[cfg(feature = "async")]
use multipeerconnectivity::{NearbyServiceAdvertiser, PeerId};

#[cfg(feature = "async")]
fn main() -> multipeerconnectivity::Result<()> {
    let peer = PeerId::new("async-advertiser-example")?;
    let advertiser = NearbyServiceAdvertiser::new(&peer, None, "test-svc")?;
    let stream = AdvertiserEventStream::subscribe_default(&advertiser);

    println!(
        "Subscribed to advertiser events (buffered={})",
        stream.buffered_count()
    );

    assert!(!stream.is_closed());
    println!("No peers in headless mode — exiting cleanly.");
    Ok(())
}

#[cfg(not(feature = "async"))]
fn main() {
    println!("Enable the `async` feature to run this example.");
}
