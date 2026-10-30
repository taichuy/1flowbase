use super::*;
use crate::routes::application_public_api::compatibility_interface;
use control_plane::{application_public_api::native::NativeRunError, errors::ControlPlaneError};

#[test]
fn authentication_projection_uses_typed_rejection_and_retains_service_statuses() {
    for context in [false, true] {
        for (error, expected, code) in [
            (
                anyhow::Error::from(ControlPlaneError::NotAuthenticated),
                StatusCode::UNAUTHORIZED,
                "not_authenticated",
            ),
            (
                anyhow::Error::from(ControlPlaneError::UpstreamUnavailable(
                    "authentication_dependency",
                )),
                StatusCode::SERVICE_UNAVAILABLE,
                "authentication_dependency",
            ),
            (
                anyhow::Error::from(std::io::Error::other("not_authenticated")),
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
            ),
        ] {
            let error = if context {
                error.context("application authentication lookup")
            } else {
                error
            };
            let projected = native_error(NativeRunError::Service(error));
            assert_eq!(projected.status, expected);
            assert_eq!(projected.code, code);
        }
    }
}

#[tokio::test]
async fn isolated_closed_pool_is_not_invalid_key_for_compatibility_and_native_routes() {
    use crate::_tests::support::test_api_state_with_database_url;
    use axum::{body::Body, http::Request};
    use tower::ServiceExt;
    let (state, _) = test_api_state_with_database_url().await;
    let app = crate::app_with_state(state.clone());
    // This pool belongs to the fixture's isolated database, never a running service.
    state.store.pool().close().await;
    for (method, path, body) in [
        (
            "POST",
            "/v1/responses",
            serde_json::json!({"model":"probe","input":"probe","stream":true}),
        ),
        (
            "POST",
            "/v1/responses",
            serde_json::json!({"model":"probe","input":"probe"}),
        ),
        ("GET", "/v1/models", serde_json::Value::Null),
        (
            "POST",
            "/api/agent/v1/runs",
            serde_json::json!({"query":"probe"}),
        ),
        ("GET", "/api/agent/v1/models", serde_json::Value::Null),
        (
            "GET",
            "/api/agent/v1/runs/00000000-0000-0000-0000-000000000000",
            serde_json::Value::Null,
        ),
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri(path)
                    .header("content-type", "application/json")
                    .header("authorization", "Bearer sk-isolated-dependency-probe")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            response.status(),
            StatusCode::INTERNAL_SERVER_ERROR,
            "{method} {path}"
        );
    }
    // WS authentication uses this same real activation owner before socket upgrade.
    for binding in [
        compatibility_interface::OPENAI_RESPONSES_WEBSOCKET_STREAM_BINDING_ID,
        compatibility_interface::NATIVE_WEBSOCKET_STREAM_BINDING_ID,
    ] {
        let error = compatibility_interface::authenticate_application_principal(
            state.clone(),
            binding,
            "sk-isolated-dependency-probe".into(),
        )
        .await
        .err()
        .unwrap();
        assert_eq!(error.status, StatusCode::INTERNAL_SERVER_ERROR, "{binding}");
    }
}
