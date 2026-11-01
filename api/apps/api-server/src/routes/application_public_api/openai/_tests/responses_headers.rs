use super::*;
use axum::{body::Body, http::header};

#[test]
fn unary_and_sse_usage_declare_historical_reasoning_already_counted() {
    for content_type in ["application/json", "text/event-stream"] {
        let original = Response::builder()
            .header(header::CONTENT_TYPE, content_type)
            .header("x-request-id", "request-123")
            .body(Body::from("provider usage payload"))
            .unwrap();
        let response = with_usage_accounting(original);
        assert_eq!(response.headers()["x-reasoning-included"], "true");
        assert_eq!(response.headers()[header::CONTENT_TYPE], content_type);
        assert_eq!(response.headers()["x-request-id"], "request-123");
        assert_eq!(response.status(), StatusCode::OK);
    }
}

#[test]
fn websocket_upgrade_declares_the_same_usage_contract() {
    let original = Response::builder()
        .status(StatusCode::SWITCHING_PROTOCOLS)
        .header(header::CONNECTION, "upgrade")
        .header(header::UPGRADE, "websocket")
        .header("sec-websocket-accept", "handshake-proof")
        .body(Body::empty())
        .unwrap();
    let response = with_usage_accounting(original);
    assert_eq!(response.headers()["x-reasoning-included"], "true");
    assert_eq!(response.status(), StatusCode::SWITCHING_PROTOCOLS);
    assert_eq!(response.headers()[header::CONNECTION], "upgrade");
    assert_eq!(response.headers()[header::UPGRADE], "websocket");
    assert_eq!(
        response.headers()["sec-websocket-accept"],
        "handshake-proof"
    );
}

#[test]
fn rejection_does_not_advertise_successful_usage_accounting() {
    for status in [
        StatusCode::UNAUTHORIZED,
        StatusCode::BAD_REQUEST,
        StatusCode::BAD_GATEWAY,
    ] {
        let response = with_usage_accounting(
            Response::builder()
                .status(status)
                .body(Body::empty())
                .unwrap(),
        );
        assert_eq!(response.status(), status);
        assert!(!response.headers().contains_key("x-reasoning-included"));
    }
}
