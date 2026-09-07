use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use serde_json::{json, Value};
use tower::ServiceExt;

use crate::_tests::support::{login_and_capture_cookie, test_app};

async fn response_json(response: axum::response::Response) -> Value {
    serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap()
}

async fn create_api_key(app: &axum::Router, cookie: &str, csrf: &str) -> String {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/user-api-keys")
                .header("cookie", cookie)
                .header("x-csrf-token", csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"name":"webmcp isolation","expiration_policy":"never"}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    response_json(response).await["data"]["token"]
        .as_str()
        .unwrap()
        .to_string()
}

#[tokio::test]
async fn root_1998_cookie_csrf_enabled_and_exposure_boundaries() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;

    let invalid_exposure = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/mcp/instances")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "instance_id": "invalid_webmcp",
                        "name": "invalid_webmcp",
                        "description_short": null,
                        "status": "enabled",
                        "default_entry_path": "/",
                        "webmcp_exposure": "public"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(invalid_exposure.status(), StatusCode::BAD_REQUEST);

    for (instance_id, webmcp_exposure, status) in [
        ("browser_visible", "authenticated_session", "enabled"),
        ("browser_hidden", "disabled", "enabled"),
        ("browser_disabled", "authenticated_session", "disabled"),
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/console/mcp/instances")
                    .header("cookie", &cookie)
                    .header("x-csrf-token", &csrf)
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "instance_id": instance_id,
                            "name": instance_id,
                            "description_short": null,
                            "status": status,
                            "default_entry_path": "/",
                            "webmcp_exposure": webmcp_exposure
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
        assert_eq!(
            response_json(response).await["data"]["webmcp_exposure"],
            json!(webmcp_exposure)
        );
    }

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/webmcp/registrations")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let payload = response_json(response).await;
    assert_eq!(payload["data"].as_array().unwrap().len(), 1);
    assert_eq!(payload["data"][0]["instance_id"], json!("browser_visible"));
    assert_eq!(payload["data"][0]["tools"].as_array().unwrap().len(), 4);
    assert_eq!(
        payload["data"][0]["tools"][0]["name"],
        json!("browser_visible_mcp_list")
    );

    let anonymous = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/webmcp/registrations")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(anonymous.status(), StatusCode::UNAUTHORIZED);

    let api_key = create_api_key(&app, &cookie, &csrf).await;
    let api_key_registration = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/webmcp/registrations")
                .header("authorization", format!("Bearer {api_key}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(api_key_registration.status(), StatusCode::FORBIDDEN);
    let receipt = api_key_registration
        .extensions()
        .get::<interface_runtime::InterfaceInvocationReceipt>()
        .expect("GET API key rejection belongs to the outer typed authorization");
    assert_eq!(
        receipt.resolved().unwrap().binding_id().as_str(),
        "http.webmcp.registrations.v1"
    );
    assert_eq!(
        receipt
            .stages()
            .filter(|stage| *stage == interface_runtime::InterfaceInvocationStage::Executing)
            .count(),
        0
    );
    let api_key_invocation = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/webmcp/browser_visible/tools/list")
                .header("authorization", format!("Bearer {api_key}"))
                .header("content-type", "application/json")
                .body(Body::from(json!({"arguments":{}}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(api_key_invocation.status(), StatusCode::FORBIDDEN);
    assert_eq!(
        response_json(api_key_invocation).await["code"],
        json!("cookie_session_required")
    );

    let invocation = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/webmcp/browser_visible/tools/list")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(json!({"arguments": {"path": "/"}}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(invocation.status(), StatusCode::OK);
    let invocation_payload = response_json(invocation).await;
    assert_eq!(invocation_payload["data"]["is_error"], json!(false));

    let missing_csrf = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/webmcp/browser_visible/tools/list")
                .header("cookie", &cookie)
                .header("content-type", "application/json")
                .body(Body::from(json!({"arguments": {}}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(missing_csrf.status(), StatusCode::UNAUTHORIZED);

    let hidden = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/webmcp/browser_hidden/tools/list")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(json!({"arguments": {}}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(hidden.status(), StatusCode::NOT_FOUND);

    let disabled_instance = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/webmcp/browser_disabled/tools/list")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(json!({"arguments":{}}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(disabled_instance.status(), StatusCode::NOT_FOUND);

    let disable = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/console/mcp/instances/browser_visible")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "instance_id": "browser_visible",
                        "name": "browser_visible",
                        "description_short": null,
                        "status": "enabled",
                        "default_entry_path": "/",
                        "webmcp_exposure": "disabled"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(disable.status(), StatusCode::OK);
    assert_eq!(
        response_json(disable).await["data"]["webmcp_exposure"],
        json!("disabled")
    );

    let registrations_after_disable = app
        .oneshot(
            Request::builder()
                .uri("/api/webmcp/registrations")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(registrations_after_disable.status(), StatusCode::OK);
    assert_eq!(
        response_json(registrations_after_disable).await["data"],
        json!([])
    );
}

mod lifecycle;
