use std::collections::HashMap;
use std::ffi::CString;

use crate::error::{MultipeerError, Result};

const MAX_SERVICE_TYPE_LEN: usize = 15;
const MAX_DISCOVERY_PAIR_LEN: usize = 254;

pub fn service_type_cstring(service_type: &str) -> Result<CString> {
    let invalid = |message: &str| Err(MultipeerError::InvalidArgument(message.into()));
    if service_type.is_empty() {
        return invalid("service type must not be empty");
    }
    if service_type.len() > MAX_SERVICE_TYPE_LEN {
        return invalid("service type must be at most 15 ASCII characters");
    }
    if !service_type
        .bytes()
        .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    {
        return invalid(
            "service type must contain only lowercase ASCII letters, digits, or hyphens",
        );
    }
    if !service_type.bytes().any(|byte| byte.is_ascii_lowercase()) {
        return invalid("service type must contain at least one letter");
    }
    if service_type.starts_with('-') || service_type.ends_with('-') {
        return invalid("service type must not begin or end with a hyphen");
    }
    if service_type.contains("--") {
        return invalid("service type must not contain consecutive hyphens");
    }
    CString::new(service_type).map_err(|_| {
        MultipeerError::InvalidArgument("service type must not contain NUL bytes".into())
    })
}

pub fn discovery_info_cstring(
    discovery_info: Option<&HashMap<String, String>>,
) -> Result<Option<CString>> {
    let Some(info) = discovery_info else {
        return Ok(None);
    };
    for (key, value) in info {
        if key.is_empty() {
            return Err(MultipeerError::InvalidArgument(
                "discovery info keys must not be empty".into(),
            ));
        }
        if !key
            .bytes()
            .all(|byte| (b' '..=b'~').contains(&byte) && byte != b'=')
        {
            return Err(MultipeerError::InvalidArgument(format!(
                "discovery info key {key:?} must contain only printable ASCII characters other than '='"
            )));
        }
        let pair_len = key.len() + 1 + value.len();
        if pair_len > MAX_DISCOVERY_PAIR_LEN {
            return Err(MultipeerError::InvalidArgument(format!(
                "discovery info entry {key:?} is {pair_len} bytes as key=value; at most {MAX_DISCOVERY_PAIR_LEN} are allowed"
            )));
        }
    }
    let json = serde_json::to_string(info)
        .map_err(|err| MultipeerError::InvalidArgument(err.to_string()))?;
    CString::new(json).map(Some).map_err(|_| {
        MultipeerError::InvalidArgument("discovery info JSON must not contain NUL bytes".into())
    })
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::{discovery_info_cstring, service_type_cstring};
    use crate::error::MultipeerError;

    fn service_type_error(service_type: &str) -> String {
        match service_type_cstring(service_type) {
            Err(MultipeerError::InvalidArgument(message)) => message,
            other => {
                panic!("expected an invalid-argument error for {service_type:?}, got {other:?}")
            }
        }
    }

    fn discovery_error(pairs: &[(&str, &str)]) -> String {
        let info: HashMap<String, String> = pairs
            .iter()
            .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
            .collect();
        match discovery_info_cstring(Some(&info)) {
            Err(MultipeerError::InvalidArgument(message)) => message,
            other => panic!("expected an invalid-argument error for {pairs:?}, got {other:?}"),
        }
    }

    #[test]
    fn accepts_rfc_6335_service_types() {
        for service_type in ["a", "a1", "1a", "a-b", "doom-chat", "abcdefghijklmno"] {
            let value = service_type_cstring(service_type).expect(service_type);
            assert_eq!(value.to_str(), Ok(service_type));
        }
    }

    #[test]
    fn rejects_service_types_the_framework_throws_on() {
        assert_eq!(service_type_error(""), "service type must not be empty");
        assert_eq!(
            service_type_error("abcdefghijklmnop"),
            "service type must be at most 15 ASCII characters"
        );
        for service_type in ["a_b", "a.b", "ABC", "a b", "caf\u{e9}", "a\0b"] {
            assert_eq!(
                service_type_error(service_type),
                "service type must contain only lowercase ASCII letters, digits, or hyphens"
            );
        }
        assert_eq!(
            service_type_error("123"),
            "service type must contain at least one letter"
        );
        assert_eq!(
            service_type_error("-"),
            "service type must contain at least one letter"
        );
        for service_type in ["-abc", "abc-"] {
            assert_eq!(
                service_type_error(service_type),
                "service type must not begin or end with a hyphen"
            );
        }
        assert_eq!(
            service_type_error("a--b"),
            "service type must not contain consecutive hyphens"
        );
    }

    #[test]
    fn missing_discovery_info_stays_absent() {
        assert!(discovery_info_cstring(None).expect("none").is_none());
    }

    #[test]
    fn valid_discovery_info_round_trips_as_json() {
        let mut info = HashMap::new();
        info.insert("role".to_owned(), "host=1 caf\u{e9}".to_owned());
        info.insert("a b".to_owned(), String::new());
        info.insert("k".to_owned(), "v".repeat(252));
        let json = discovery_info_cstring(Some(&info))
            .expect("valid")
            .expect("present");
        let decoded: HashMap<String, String> =
            serde_json::from_str(json.to_str().expect("utf-8")).expect("json");
        assert_eq!(decoded, info);
    }

    #[test]
    fn rejects_discovery_keys_the_framework_throws_on() {
        assert_eq!(
            discovery_error(&[("", "v")]),
            "discovery info keys must not be empty"
        );
        for key in ["a=b", "caf\u{e9}", "a\tb", "a\u{7f}", "a\0b"] {
            assert_eq!(
                discovery_error(&[(key, "v")]),
                format!(
                    "discovery info key {key:?} must contain only printable ASCII characters other than '='"
                )
            );
        }
    }

    #[test]
    fn rejects_discovery_pairs_longer_than_254_bytes() {
        let at_limit = "v".repeat(252);
        assert!(discovery_info_cstring(Some(&HashMap::from([("k".to_owned(), at_limit)]))).is_ok());

        let over_limit = "v".repeat(253);
        assert_eq!(
            discovery_error(&[("k", &over_limit)]),
            "discovery info entry \"k\" is 255 bytes as key=value; at most 254 are allowed"
        );

        let multibyte = "\u{e9}".repeat(127);
        assert_eq!(
            discovery_error(&[("k", &multibyte)]),
            "discovery info entry \"k\" is 256 bytes as key=value; at most 254 are allowed"
        );
    }
}
