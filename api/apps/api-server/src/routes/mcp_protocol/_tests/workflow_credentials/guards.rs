use super::*;

#[test]
fn console_workflow_request_rejects_external_urls_and_identity_headers() {
    assert!(console_request(request("/api/console/me?view=short")).is_ok());
    for url in [
        "https://localhost/api/console/me",
        "//localhost/api/console/me",
        "/api/public/auth/sign-in",
        "/api/console/../public/auth",
        "/api/console/%2e%2e/public",
        "/api/console/me#fragment",
    ] {
        assert!(console_request(request(url)).is_err(), "{url}");
    }
    for name in [
        "Authorization",
        "Cookie",
        "X-CSRF-Token",
        "Host",
        "Content-Length",
        "Proxy-Authorization",
    ] {
        let mut input = request("/api/console/me");
        input.headers.push((name.into(), "forged".into()));
        assert!(console_request(input).is_err(), "{name}");
    }
}

#[tokio::test]
async fn console_workflow_dispatch_does_not_trust_run_identity_headers() {
    let mut input = request("/api/console/me");
    input
        .headers
        .push(("x-workflow-run-id".into(), Uuid::now_v7().to_string()));
    input
        .headers
        .push(("x-server-delegated-user".into(), "root".into()));
    let error = dispatch_console_http(Router::new(), input)
        .await
        .unwrap_err();
    assert_eq!(error.to_string(), "workflow_console_delegation_required");
}
