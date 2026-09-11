use super::*;

// #2027 AC-009: only stable codes and explicitly public structured details survive MCP projection.
#[tokio::test]
async fn issue_2027_mcp_target_failure_preserves_bounded_public_diagnostics() {
    for status in [StatusCode::BAD_REQUEST, StatusCode::CONFLICT] {
        let response = Response::builder().status(status).body(axum::body::Body::from(json!({
            "status": status.as_u16(), "code":"source_text_ambiguous", "message":"secret raw message",
            "details":{"edit_index":2,"candidates":[{"line":3,"column":1}]},
            "internal_trace":"must never be forwarded"
        }).to_string())).unwrap();
        let VirtualToolOutcome::Error {
            data: Some(data), ..
        } = target_interface_failure(response).await
        else {
            panic!("expected MCP error")
        };
        assert_eq!(data["target_code"], "source_text_ambiguous");
        assert_eq!(data["target_details"]["edit_index"], 2);
        assert!(data.get("message").is_none());
        assert!(!data.to_string().contains("secret"));
        assert!(!data.to_string().contains("internal_trace"));
        assert_eq!(data["retry_original"], false);
    }
}

#[tokio::test]
async fn issue_2027_mcp_target_failure_drops_invalid_or_oversized_diagnostics() {
    for body in [
        "not json".to_owned(),
        json!({"code":"bad code with spaces", "details":{"text":"x".repeat(20000)}}).to_string(),
    ] {
        let response = Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body(axum::body::Body::from(body))
            .unwrap();
        let VirtualToolOutcome::Error {
            data: Some(data), ..
        } = target_interface_failure(response).await
        else {
            panic!("expected error")
        };
        assert!(data.get("target_details").is_none());
        assert!(data.get("target_code").is_none());
        assert_eq!(data["http_status"], 400);
    }
}

#[tokio::test]
async fn issue_2027_mcp_diagnostics_bound_and_auth_server_exclusion_are_independent() {
    for status in [
        StatusCode::UNAUTHORIZED,
        StatusCode::FORBIDDEN,
        StatusCode::INTERNAL_SERVER_ERROR,
        StatusCode::BAD_REQUEST,
    ] {
        let detail = if status == StatusCode::BAD_REQUEST {
            "x".repeat(5000)
        } else {
            "secret".into()
        };
        let response = Response::builder().status(status).body(axum::body::Body::from(json!({
            "status":status.as_u16(),"code":"valid_code","details":{"value":detail},"message":"private"
        }).to_string())).unwrap();
        let VirtualToolOutcome::Error {
            data: Some(data), ..
        } = target_interface_failure(response).await
        else {
            panic!("expected error")
        };
        assert!(data.get("target_details").is_none());
        assert!(!data.to_string().contains("secret"));
        assert!(!data.to_string().contains("private"));
        if status == StatusCode::INTERNAL_SERVER_ERROR {
            assert!(data.get("target_code").is_none());
        } else {
            assert_eq!(data["target_code"], "valid_code");
        }
    }
}
