#[test]
fn external_auth_trace_never_records_callback_credentials_or_state() {
    for path in [
        "/api/public/auth/external/callback",
        "/api/public/auth/external/start",
    ] {
        let uri = format!("{path}?code=fixture-secret-code&state=fixture-secret-state")
            .parse()
            .unwrap();
        assert_eq!(crate::safe_http_trace_uri(&uri), path);
    }
    let uri = "/api/console/settings/auth-center/overview?page=2"
        .parse()
        .unwrap();
    assert_eq!(
        crate::safe_http_trace_uri(&uri),
        "/api/console/settings/auth-center/overview?page=2"
    );
}
