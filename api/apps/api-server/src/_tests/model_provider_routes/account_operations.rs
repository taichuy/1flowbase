use super::*;

async fn response_json(response: axum::response::Response) -> serde_json::Value {
    serde_json::from_slice(
        &axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap()
}

#[tokio::test]
async fn provider_account_routes_project_capabilities_and_protect_consume_with_csrf() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let installation_id = install_enable_assign(&app, &cookie, &csrf).await;

    let create = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/settings/model-providers/instances")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "installation_id": installation_id,
                        "display_name": "Account operation fixture",
                        "enabled_model_ids": [],
                        "config": {
                            "base_url": "https://api.example.com",
                            "api_key": "super-secret"
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create.status(), StatusCode::CREATED);
    let created = response_json(create).await;
    let instance_id = created["data"]["id"].as_str().unwrap();

    let catalog = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/console/settings/model-providers/catalog")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(catalog.status(), StatusCode::OK);
    let catalog_payload = response_json(catalog).await;
    assert_eq!(
        catalog_payload["data"]["entries"][0]["operational_capabilities"],
        json!([
            "validate_config",
            "list_models",
            "usage_windows",
            "reset_credits"
        ])
    );

    let usage = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/settings/model-providers/instances/{instance_id}/usage"
                ))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(usage.status(), StatusCode::OK);
    let usage_payload = response_json(usage).await;
    assert_eq!(
        usage_payload["data"]["windows"][0]["limit_window_seconds"],
        18_000
    );
    assert_eq!(usage_payload["data"]["windows"][1]["used_percent"], 61.0);
    assert!(!usage_payload.to_string().contains("super-secret"));

    let count = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/settings/model-providers/instances/{instance_id}/reset-credits"
                ))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(count.status(), StatusCode::OK);
    assert_eq!(response_json(count).await["data"]["available_count"], 2);

    let missing_csrf = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/console/settings/model-providers/instances/{instance_id}/reset-credits/consume"
                ))
                .header("cookie", &cookie)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "idempotency_key": "attempt-missing-csrf" }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(missing_csrf.status(), StatusCode::UNAUTHORIZED);

    let consumed = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/console/settings/model-providers/instances/{instance_id}/reset-credits/consume"
                ))
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "idempotency_key": "attempt-123" }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(consumed.status(), StatusCode::OK);
    assert_eq!(response_json(consumed).await["data"]["consumed"], true);
}
