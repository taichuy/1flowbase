use super::*;

#[tokio::test]
async fn model_provider_authenticate_route_requires_csrf_persists_managed_secret_and_redacts_it() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let installation_id =
        install_enable_assign_with_fixture(&app, &cookie, &csrf, create_auth_provider_fixture)
            .await;

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
                        "display_name": "Authenticated Fixture",
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
    let create_payload: Value =
        serde_json::from_slice(&to_bytes(create.into_body(), usize::MAX).await.unwrap()).unwrap();
    let instance_id = create_payload["data"]["id"].as_str().unwrap();

    let missing_csrf = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/console/settings/model-providers/instances/{instance_id}/authenticate"
                ))
                .header("cookie", &cookie)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "operation": { "type": "begin", "action": "device_code" } })
                        .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(missing_csrf.status(), StatusCode::UNAUTHORIZED);

    let authenticate = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/console/settings/model-providers/instances/{instance_id}/authenticate"
                ))
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "operation": { "type": "begin", "action": "device_code" } })
                        .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let authenticate_status = authenticate.status();
    let authenticate_payload: Value = serde_json::from_slice(
        &to_bytes(authenticate.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        authenticate_status,
        StatusCode::OK,
        "authenticate payload: {authenticate_payload}"
    );
    assert_eq!(
        authenticate_payload["data"]["status"].as_str(),
        Some("authorized")
    );
    assert_eq!(
        authenticate_payload["data"]["message"].as_str(),
        Some("Fixture device code authorized")
    );
    assert!(!authenticate_payload
        .to_string()
        .contains("fixture-access-token"));
    assert!(authenticate_payload["data"]
        .get("managed_secret_patch")
        .is_none());

    let validate = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/console/settings/model-providers/instances/{instance_id}/validate"
                ))
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(validate.status(), StatusCode::OK);
    let validate_payload: Value =
        serde_json::from_slice(&to_bytes(validate.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(
        validate_payload["data"]["output"]["sanitized"]["access_token"].as_str(),
        Some("***")
    );
}
