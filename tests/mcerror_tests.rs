use multipeerconnectivity::{mc_error_domain, FrameworkError, MCErrorCode, MultipeerError};

#[test]
fn mc_error_codes_roundtrip() {
    assert_eq!(MCErrorCode::from_raw(4), MCErrorCode::TimedOut);
    assert_eq!(MCErrorCode::Unavailable.as_raw(), 6);
}

#[test]
fn framework_error_maps_multipeer_domain() {
    let error = FrameworkError::new(mc_error_domain(), 2, "bad parameter".into());
    assert_eq!(error.mc_error_code(), Some(MCErrorCode::InvalidParameter));
    assert_eq!(
        MultipeerError::Framework(error.clone()).to_string(),
        format!("framework error: {error}")
    );
}
