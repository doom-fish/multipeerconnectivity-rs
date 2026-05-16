use multipeerconnectivity::{MultipeerError, PeerId, Result};

#[test]
fn peer_id_roundtrip_archives() -> Result<()> {
    let peer = PeerId::new("doom-fish-peer")?;
    let archived = peer.archived_data()?;
    let decoded = PeerId::from_archived_data(&archived)?;

    assert_eq!(decoded.display_name(), "doom-fish-peer");
    Ok(())
}

#[test]
fn peer_id_rejects_invalid_display_names() {
    assert!(matches!(PeerId::new(""), Err(MultipeerError::InvalidArgument(_))));
    let too_long = "x".repeat(64);
    assert!(matches!(PeerId::new(too_long), Err(MultipeerError::InvalidArgument(_))));
}
