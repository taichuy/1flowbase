use super::*;

#[test]
fn derives_public_https_and_loopback_origins_without_configuration() {
    for (host, proto, expected) in [
        ("demo.example.com", None, "https://demo.example.com"),
        (
            "demo.example.com:8443",
            Some("https"),
            "https://demo.example.com:8443",
        ),
        ("localhost:3100", None, "http://localhost:3100"),
        ("127.0.0.1:7800", Some("http"), "http://127.0.0.1:7800"),
        ("[::1]:3100", None, "http://[::1]:3100"),
    ] {
        let mut headers = HeaderMap::new();
        headers.insert(HOST, host.parse().unwrap());
        headers.insert("origin", "https://unrelated.example".parse().unwrap());
        headers.insert("x-forwarded-host", "unrelated.example".parse().unwrap());
        if let Some(proto) = proto {
            headers.insert("x-forwarded-proto", proto.parse().unwrap());
        }
        assert_eq!(request_origin(&headers).unwrap(), expected);
    }
}

#[test]
fn rejects_ambiguous_authorities_and_public_http() {
    assert!(request_origin(&HeaderMap::new()).is_err());
    for host in [
        "user@demo.example",
        "demo.example/path",
        "demo.example?query",
        "demo.example#fragment",
        "demo.example,evil.example",
    ] {
        let mut headers = HeaderMap::new();
        headers.insert(HOST, host.parse().unwrap());
        assert!(request_origin(&headers).is_err(), "{host}");
    }
    for proto in ["http", "https,http", "ftp", ""] {
        let mut headers = HeaderMap::new();
        headers.insert(HOST, "demo.example".parse().unwrap());
        headers.insert("x-forwarded-proto", proto.parse().unwrap());
        assert!(request_origin(&headers).is_err(), "{proto}");
    }
    let mut headers = HeaderMap::new();
    headers.append(HOST, "demo.example".parse().unwrap());
    headers.append(HOST, "other.example".parse().unwrap());
    assert!(request_origin(&headers).is_err());
}
