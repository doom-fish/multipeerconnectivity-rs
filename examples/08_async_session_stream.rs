//! Example: subscribe to `MCSession` events as an async stream.
//!
//! This example creates a session, subscribes to its event stream, and
//! exits gracefully after a short timeout (no peers available in headless CI).
#[cfg(feature = "async")]
use multipeerconnectivity::async_api::SessionEventStream;
#[cfg(feature = "async")]
use multipeerconnectivity::{EncryptionPreference, PeerId, Session};

#[cfg(feature = "async")]
fn main() -> multipeerconnectivity::Result<()> {
    let peer = PeerId::new("async-session-example")?;
    let session = Session::new(&peer, EncryptionPreference::Optional)?;
    let stream = SessionEventStream::subscribe_default(&session);

    println!(
        "Subscribed to session events (capacity={}, closed={})",
        multipeerconnectivity::async_api::DEFAULT_CAPACITY,
        stream.is_closed()
    );

    // In a real app you would `await stream.next()` inside an async runtime.
    // Here we just verify the subscription round-trips cleanly.
    assert!(!stream.is_closed());
    println!("No peers in headless mode — exiting cleanly.");
    Ok(())
}

#[cfg(not(feature = "async"))]
fn main() {
    println!("Enable the `async` feature to run this example.");
}
