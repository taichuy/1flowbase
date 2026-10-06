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

#[test]
fn explicit_browser_origin_survives_internal_host_and_stays_query_free_in_issuer() {
    let mut headers = HeaderMap::new();
    headers.insert(HOST, "127.0.0.1".parse().unwrap());
    headers.insert("x-forwarded-proto", "http".parse().unwrap());
    let uri = "/api/mcp/demo?origin=https%3A%2F%2Fpublic.example%3A8443"
        .parse()
        .unwrap();
    let context = request_public_origin(&headers, &uri).unwrap();
    assert_eq!(context.origin, "https://public.example:8443");
    assert!(context.explicit);
    assert_eq!(
        context.resource("demo").unwrap(),
        "https://public.example:8443/api/mcp/demo?origin=https%3A%2F%2Fpublic.example%3A8443"
    );
    let issuer = context.issuer();
    assert!(url::Url::parse(&issuer).unwrap().query().is_none());
    let metadata_uri = format!("{issuer}/.well-known/openid-configuration")
        .parse()
        .unwrap();
    assert_eq!(
        request_public_origin(&headers, &metadata_uri)
            .unwrap()
            .origin,
        context.origin
    );
}

#[test]
fn rejects_invalid_duplicate_and_conflicting_public_origins() {
    let headers = HeaderMap::new();
    for query in [
        "origin=",
        "origin=public.example",
        "origin=http%3A%2F%2Fpublic.example",
        "origin=https%3A%2F%2Fuser%40public.example",
        "origin=https%3A%2F%2Fpublic.example%2Fpath",
        "origin=https%3A%2F%2Fpublic.example%3Fx%3D1",
        "origin=https%3A%2F%2Fpublic.example&origin=https%3A%2F%2Fother.example",
    ] {
        let uri = format!("/api/mcp/demo?{query}").parse().unwrap();
        assert!(request_public_origin(&headers, &uri).is_err(), "{query}");
    }
    let context = PublicOrigin {
        origin: "https://public.example".into(),
        explicit: true,
    };
    let uri = format!(
        "{}/token?origin=https%3A%2F%2Fother.example",
        context.issuer()
    )
    .parse()
    .unwrap();
    assert!(request_public_origin(&headers, &uri).is_err());
}
