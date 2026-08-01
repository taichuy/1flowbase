use super::*;

#[tokio::test]
async fn model_definition_scope_grant_routes_do_not_return_model_level_api_exposure() {
    let (app, database_url) = test_app_with_database_url().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let session_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/console/session")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let session_payload: serde_json::Value = serde_json::from_slice(
        &to_bytes(session_response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    let workspace_id = session_payload["data"]["session"]["current_workspace_id"]
        .as_str()
        .unwrap()
        .to_string();

    let create_model_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/settings/data-models/model-definitions")
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
    assert_eq!(created["data"]["status"], json!("published"));
    assert!(!created["data"]
        .as_object()
        .unwrap()
        .contains_key("api_exposure_status"));

    let initial_grants_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/settings/data-models/model-definitions/{model_id}/scope-grants"
                ))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(initial_grants_response.status(), StatusCode::OK);
    let initial_grants: serde_json::Value = serde_json::from_slice(
        &to_bytes(initial_grants_response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    let initial_grant = initial_grants["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|grant| grant["data_model_id"].as_str() == Some(&model_id))
        .expect("workspace model creation must install its owner grant");
    let initial_grant_id = initial_grant["id"].as_str().unwrap().to_string();
    assert_eq!(initial_grant["scope_kind"], json!("workspace"));
    assert_eq!(initial_grant["scope_id"], json!(workspace_id));
    assert_eq!(initial_grant["permission_profile"], json!("scope_all"));

    let create_grant_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/console/settings/data-models/model-definitions/{model_id}/scope-grants"
                ))
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "scope_kind": "workspace",
                        "scope_id": workspace_id,
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
    assert_eq!(grant_id, initial_grant_id);

    let list_grants_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!(
                    "/api/console/settings/data-models/model-definitions/{model_id}/scope-grants"
                ))
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
                && grant["scope_kind"].as_str() == Some("workspace")
                && grant["permission_profile"].as_str() == Some("scope_all")
        }));
    assert_eq!(
        list_grants_payload["data"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|grant| grant["data_model_id"].as_str() == Some(&model_id))
            .count(),
        1
    );

    let list_models_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/console/settings/data-models/model-definitions")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(list_models_response.status(), StatusCode::OK);
    let list_models_payload: serde_json::Value = serde_json::from_slice(
        &to_bytes(list_models_response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    let model_payload = list_models_payload["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|model| model["id"].as_str() == Some(&model_id))
        .unwrap();
    assert_eq!(model_payload["status"], json!("published"));
    assert!(!model_payload
        .as_object()
        .unwrap()
        .contains_key("api_exposure_status"));
    assert_eq!(
        audit_event_count(&database_url, "state_model.scope_grant_created").await,
        1
    );
}
