//! Example: subscribe to `MCNearbyServiceBrowser` events as an async stream.
#[cfg(feature = "async")]
use multipeerconnectivity::async_api::BrowserEventStream;
#[cfg(feature = "async")]
use multipeerconnectivity::{NearbyServiceBrowser, PeerId};

#[cfg(feature = "async")]
fn main() -> multipeerconnectivity::Result<()> {
    let peer = PeerId::new("async-browser-example")?;
    let browser = NearbyServiceBrowser::new(&peer, "test-svc")?;
    let stream = BrowserEventStream::subscribe_default(&browser);

    println!(
        "Subscribed to browser events (buffered={})",
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
