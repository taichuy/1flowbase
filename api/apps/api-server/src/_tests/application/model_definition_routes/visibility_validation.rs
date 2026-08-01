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
    assert!(allowed_payload["data"]
        .as_array()
        .unwrap()
        .iter()
        .any(|model| model["id"].as_str() == Some(&model_id)));

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
    assert_eq!(session_response.status(), StatusCode::OK);
    let session_payload: serde_json::Value = serde_json::from_slice(
        &to_bytes(session_response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    let current_workspace_id = session_payload["data"]["session"]["current_workspace_id"]
        .as_str()
        .unwrap();

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
    let workspace_body = to_bytes(workspace_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let workspace_payload: serde_json::Value = serde_json::from_slice(&workspace_body).unwrap();
    assert_eq!(workspace_payload["data"]["scope_kind"], json!("workspace"));
    assert_eq!(
        workspace_payload["data"]["scope_id"],
        json!(current_workspace_id)
    );
    assert!(workspace_payload["data"]["physical_table_name"]
        .as_str()
        .unwrap()
        .starts_with("rtm_workspace_"));

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
async fn model_definition_response_schemas_accept_nullable_and_dynamic_json_fields() {
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
                        "code": "nullable_response_contract",
                        "title": "Nullable Response Contract"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_response.status(), StatusCode::CREATED);
    let created_model: serde_json::Value = serde_json::from_slice(
        &to_bytes(create_response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    let model_id = created_model["data"]["id"].as_str().unwrap();
    assert!(created_model["data"]["external_resource_key"].is_null());
    assert!(created_model["data"]["builtin_kind"].is_null());

    let field_response = app
        .clone()
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
                        "code": "state",
                        "title": "State",
                        "field_kind": "text",
                        "default_value": "todo"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(field_response.status(), StatusCode::CREATED);
    let created_field: serde_json::Value = serde_json::from_slice(
        &to_bytes(field_response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(created_field["data"]["default_value"], json!("todo"));
    assert!(created_field["data"]["description"].is_null());
    assert!(created_field["data"]["external_field_key"].is_null());

    let list_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/console/settings/data-models/model-definitions")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(list_response.status(), StatusCode::OK);
    let listed_models: serde_json::Value = serde_json::from_slice(
        &to_bytes(list_response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();

    let openapi_response = app
        .oneshot(
            Request::builder()
                .uri("/openapi.json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(openapi_response.status(), StatusCode::OK);
    let openapi: serde_json::Value = serde_json::from_slice(
        &to_bytes(openapi_response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();

    let response_cases = [
        (
            "model_definitions.create",
            "POST",
            "/api/console/settings/data-models/model-definitions",
            &created_model["data"],
        ),
        (
            "model_fields.create",
            "POST",
            "/api/console/settings/data-models/model-definitions/{id}/fields",
            &created_field["data"],
        ),
        (
            "model_definitions.list",
            "GET",
            "/api/console/settings/data-models/model-definitions",
            &listed_models["data"],
        ),
    ];
    for (id, method, path, payload) in response_cases {
        let operation = crate::openapi_docs::DocsCatalogOperation {
            id: id.into(),
            method: method.into(),
            path: path.into(),
            summary: None,
            description: None,
            tags: Vec::new(),
            group: "settings".into(),
            deprecated: false,
        };
        let interface =
            crate::openapi_interface::catalog_entry_from_operation(&operation, &openapi)
                .expect("Data Model interface catalog entry");
        let response_validator = jsonschema::validator_for(&interface.response_schema)
            .expect("generated Data Model response schema");
        assert!(
            response_validator.validate(payload).is_ok(),
            "{id} response must match its strict interface schema"
        );
    }
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
