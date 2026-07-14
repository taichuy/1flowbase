use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;

use crate::_tests::support::{login_and_capture_cookie, test_app_with_database_url};

async fn response_json(response: axum::response::Response) -> Value {
    serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap()
}

async fn create_mcp_instance(app: &axum::Router, cookie: &str, csrf: &str) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/mcp/instances")
                .header("cookie", cookie)
                .header("x-csrf-token", csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "instance_id":"taichuy",
                        "name":"1flowbase",
                        "description_short":null,
                        "status":"enabled",
                        "default_entry_path":"/"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
}

#[tokio::test]
async fn mcp_client_credential_is_encrypted_restored_and_deleted_for_current_user() {
    let (app, database_url) = test_app_with_database_url().await;
    let pool = PgPool::connect(&database_url).await.unwrap();
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    create_mcp_instance(&app, &cookie, &csrf).await;
    let api_key = "pat_saved_mcp_client_secret";

    let save_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/console/mcp/instances/taichuy/client-credential")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(json!({ "api_key": api_key }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(save_response.status(), StatusCode::OK);
    assert_eq!(
        response_json(save_response).await["data"]["saved"],
        json!(true)
    );

    let stored_secret: Value =
        sqlx::query_scalar("select encrypted_secret_json from mcp_client_credentials limit 1")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(!stored_secret.to_string().contains(api_key));

    let get_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/console/mcp/instances/taichuy/client-credential")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(get_response.status(), StatusCode::OK);
    let get_payload = response_json(get_response).await;
    assert_eq!(get_payload["data"]["saved"], json!(true));
    assert_eq!(get_payload["data"]["api_key"], json!(api_key));

    let delete_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/console/mcp/instances/taichuy/client-credential")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(delete_response.status(), StatusCode::NO_CONTENT);

    let missing_response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/console/mcp/instances/taichuy/client-credential")
                .header("cookie", cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(missing_response.status(), StatusCode::OK);
    let missing_payload = response_json(missing_response).await;
    assert_eq!(missing_payload["data"]["saved"], json!(false));
    assert!(missing_payload["data"]["api_key"].is_null());
}
