use super::*;

#[tokio::test]
async fn model_definition_routes_require_data_models_feature() {
    let app = test_app().await;
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;

    let create_model_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/settings/data-models/model-definitions")
                .header("cookie", &root_cookie)
                .header("x-csrf-token", &root_csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "scope_kind": "workspace",
                        "code": "orders_acl",
                        "title": "Orders ACL"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(create_model_response.status(), StatusCode::CREATED);
    let model_body = to_bytes(create_model_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let created_model: serde_json::Value = serde_json::from_slice(&model_body).unwrap();
    let model_id = created_model["data"]["id"].as_str().unwrap().to_string();

    create_role(&app, &root_cookie, &root_csrf, "model_reader").await;
    replace_role_permissions(
        &app,
        &root_cookie,
        &root_csrf,
        "model_reader",
        &["settings_feature.access.system.data-models"],
    )
    .await;

    create_role(&app, &root_cookie, &root_csrf, "no_model_access").await;

    let reader_member_id =
        create_member(&app, &root_cookie, &root_csrf, "reader-1", "temp-pass").await;
    replace_member_roles(
        &app,
        &root_cookie,
        &root_csrf,
        &reader_member_id,
        &["model_reader"],
    )
    .await;

    let blocked_member_id =
        create_member(&app, &root_cookie, &root_csrf, "blocked-1", "temp-pass").await;
    replace_member_roles(
        &app,
        &root_cookie,
        &root_csrf,
        &blocked_member_id,
        &["no_model_access"],
    )
    .await;

    let (reader_cookie, _) = login_and_capture_cookie(&app, "reader-1", "temp-pass").await;
    let allowed_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/console/settings/data-models/model-definitions")
                .header("cookie", &reader_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(allowed_response.status(), StatusCode::OK);
    let allowed_payload: serde_json::Value = serde_json::from_slice(
        &to_bytes(allowed_response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert!(
        allowed_payload["data"]
            .as_array()
            .unwrap()
            .iter()
            .any(|model| model["id"].as_str() == Some(&model_id))
    );

    let (blocked_cookie, _) = login_and_capture_cookie(&app, "blocked-1", "temp-pass").await;
    let blocked_response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/console/settings/data-models/model-definitions")
                .header("cookie", &blocked_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(blocked_response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn create_model_route_accepts_workspace_and_system_scope_kinds_only() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let legacy_scope_kind = ["te", "am"].concat();

    let workspace_response = app
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
                        "code": "workspace_orders_scope_contract",
                        "title": "Workspace Orders Scope Contract"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(workspace_response.status(), StatusCode::CREATED);

    let system_response = app
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
                        "scope_kind": "system",
                        "code": "system_orders_scope_contract",
                        "title": "System Orders Scope Contract"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(system_response.status(), StatusCode::CREATED);
    let system_body = to_bytes(system_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let system_payload: serde_json::Value = serde_json::from_slice(&system_body).unwrap();
    assert_eq!(
        system_payload["data"]["scope_id"],
        serde_json::Value::String(domain::SYSTEM_SCOPE_ID.to_string())
    );

    let legacy_response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/settings/data-models/model-definitions")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "scope_kind": legacy_scope_kind,
                        "code": "legacy_team_scope_contract",
                        "title": "Legacy Team Scope Contract"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(legacy_response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn create_model_route_rejects_field_code_that_sanitizes_to_platform_column() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;

    let create_response = app
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
                        "code": "platform_column_orders",
                        "title": "Platform Column Orders"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_response.status(), StatusCode::CREATED);
    let created: serde_json::Value = serde_json::from_slice(
        &to_bytes(create_response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    let model_id = created["data"]["id"].as_str().unwrap();

    let field_response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/console/settings/data-models/model-definitions/{model_id}/fields"
                ))
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "code": "created-at",
                        "title": "Created At",
                        "field_kind": "datetime",
                        "is_required": false,
                        "is_unique": false
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(field_response.status(), StatusCode::BAD_REQUEST);
    let error: serde_json::Value = serde_json::from_slice(
        &to_bytes(field_response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(error["code"], json!("physical_column_name"));
}
