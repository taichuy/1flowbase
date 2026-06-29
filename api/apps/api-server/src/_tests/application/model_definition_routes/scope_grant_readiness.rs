use super::*;

#[tokio::test]
async fn model_definition_scope_grant_routes_do_not_drive_api_exposure_status() {
    let (app, database_url) = test_app_with_database_url().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;

    let create_model_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/models")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "scope_kind": "workspace",
                        "code": "scope_grant_route_orders",
                        "title": "Scope Grant Route Orders"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_model_response.status(), StatusCode::CREATED);
    let created: serde_json::Value = serde_json::from_slice(
        &to_bytes(create_model_response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    let model_id = created["data"]["id"].as_str().unwrap().to_string();
    assert_eq!(
        created["data"]["api_exposure_status"],
        json!("published_not_exposed")
    );

    let create_grant_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/console/models/{model_id}/scope-grants"))
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "scope_kind": "system",
                        "scope_id": domain::SYSTEM_SCOPE_ID,
                        "enabled": true,
                        "permission_profile": "scope_all"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_grant_response.status(), StatusCode::CREATED);
    let grant_payload: serde_json::Value = serde_json::from_slice(
        &to_bytes(create_grant_response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    let grant_id = grant_payload["data"]["id"].as_str().unwrap().to_string();

    let list_grants_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/console/models/{model_id}/scope-grants"))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(list_grants_response.status(), StatusCode::OK);
    let list_grants_payload: serde_json::Value = serde_json::from_slice(
        &to_bytes(list_grants_response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert!(list_grants_payload["data"]
        .as_array()
        .unwrap()
        .iter()
        .any(|grant| {
            grant["id"].as_str() == Some(&grant_id)
                && grant["data_model_id"].as_str() == Some(&model_id)
                && grant["scope_kind"].as_str() == Some("system")
                && grant["permission_profile"].as_str() == Some("scope_all")
        }));

    let get_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/console/models/{model_id}"))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(get_response.status(), StatusCode::OK);
    let model_payload: serde_json::Value = serde_json::from_slice(
        &to_bytes(get_response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        model_payload["data"]["api_exposure_status"],
        json!("published_not_exposed")
    );
    assert_eq!(
        audit_event_count(&database_url, "state_model.scope_grant_created").await,
        2
    );
}
