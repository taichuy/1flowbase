use crate::_tests::support::{
    create_member, create_role, login_and_capture_cookie, replace_member_roles,
    test_api_state_with_database_url, test_config,
};
use axum::{
    body::{to_bytes, Body},
    http::{header, Request, StatusCode},
};
use serde_json::{json, Value};
use tower::ServiceExt;
async fn replace_backup_policy(
    app: &axum::Router,
    root_cookie: &str,
    root_csrf: &str,
    role_code: &str,
    operations: &[(&str, bool)],
) {
    let operations = operations
        .iter()
        .map(|(operation_id, enabled)| {
            json!({
                "kind": "simple",
                "operation_id": operation_id,
                "enabled": enabled
            })
        })
        .collect::<Vec<_>>();
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!(
                    "/api/console/settings/roles/{role_code}/console-policy"
                ))
                .header("cookie", root_cookie)
                .header("x-csrf-token", root_csrf)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "groups": [{
                            "kind": "settings_feature",
                            "group_id": "system.backups",
                            "enabled": true,
                            "strategy": "custom",
                            "operations": operations
                        }]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn portable_template_routes_enforce_independent_grants_and_reject_invalid_package() {
    let (state, _) = test_api_state_with_database_url().await;
    let app = crate::app_with_state_and_config(state, &test_config());
    let (root, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    create_role(&app, &root, &csrf, "template_reader").await;
    let member = create_member(&app, &root, &csrf, "template-reader", "temp-pass").await;
    replace_member_roles(&app, &root, &csrf, &member, &["template_reader"]).await;
    let (cookie, member_csrf) =
        login_and_capture_cookie(&app, "template-reader", "temp-pass").await;
    for granted in [false, true, false] {
        replace_backup_policy(
            &app,
            &root,
            &csrf,
            "template_reader",
            &[("system_templates.catalog", granted)],
        )
        .await;
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/console/settings/system-templates/catalog")
                    .header("cookie", &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            response.status(),
            if granted {
                StatusCode::OK
            } else {
                StatusCode::FORBIDDEN
            }
        );
        if granted {
            let payload: Value =
                serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap())
                    .unwrap();
            assert!(payload["data"]["pages"].is_array());
            assert!(payload["data"]["applications"].is_array());
            assert!(payload["data"]["data_models"].is_array());
            assert!(payload["data"]["mcp_instances"].is_array());
        }
    }
    let package = json!({"schema_version":format!("invalid{}", "x".repeat(2 * 1024 * 1024)),"pages":[],"applications":[],"data_models":[],"plugins":[]});
    let denied = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/settings/system-templates/install")
                .header("cookie", &cookie)
                .header("x-csrf-token", &member_csrf)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(package.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(denied.status(), StatusCode::FORBIDDEN);
    let preview = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/settings/system-templates/preview")
                .header("cookie", &root)
                .header("x-csrf-token", &csrf)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(package.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(preview.status(), StatusCode::OK);
    let payload: Value =
        serde_json::from_slice(&to_bytes(preview.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(payload["data"]["valid"], false);
    assert!(payload["data"]["failures"]
        .as_array()
        .unwrap()
        .iter()
        .any(|v| v == "portable_template_schema_version"));
    let rejected = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/settings/system-templates/install")
                .header("cookie", &root)
                .header("x-csrf-token", &csrf)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(package.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(rejected.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn portable_template_install_registers_models_without_restart() {
    let (state, _) = test_api_state_with_database_url().await;
    let app = crate::app_with_state_and_config(state, &test_config());
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let models = ["published", "draft"].map(|status| {
        json!({
            "id": uuid::Uuid::new_v4(), "code": format!("template_{status}"),
            "title": status, "description": null, "scope_kind": "workspace",
            "template_provider": "core", "template_code": "general", "template_version": "v1",
            "status": status, "builtin": false, "fields": []
        })
    });
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/settings/system-templates/install")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "schema_version": "1flowbase.portable-template/v1", "pages": [],
                        "applications": [], "data_models": models, "plugins": []
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(body["data"]["complete"], true, "{body}");
    for (code, expected) in [
        ("template_published", StatusCode::OK),
        ("template_draft", StatusCode::CONFLICT),
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/api/runtime/models/{code}/list"))
                    .header("cookie", &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = response.status();
        let body: Value =
            serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap())
                .unwrap();
        assert_eq!(status, expected, "{body}");
        if code == "template_draft" {
            assert_eq!(body["code"], "model_not_published");
        }
    }
}

#[tokio::test]
async fn mcp_only_application_template_creates_then_updates_the_selected_instance() {
    let (state, _) = test_api_state_with_database_url().await;
    let app = crate::app_with_state_and_config(state, &test_config());
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let send = |method: &str, path: &str, body: Value| {
        let app = app.clone();
        let cookie = cookie.clone();
        let csrf = csrf.clone();
        let method = method.to_owned();
        let path = path.to_owned();
        async move {
            app.oneshot(
                Request::builder()
                    .method(method.as_str())
                    .uri(path)
                    .header("cookie", cookie)
                    .header("x-csrf-token", csrf)
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap()
        }
    };
    let created = send(
        "POST",
        "/api/console/mcp/instances",
        json!({
            "instance_id": "template_instance", "name": "Initial instance",
            "description_short": null, "status": "draft", "default_entry_path": "/"
        }),
    )
    .await;
    assert_eq!(created.status(), StatusCode::CREATED);

    let exported = send(
        "POST",
        "/api/console/settings/system-templates/export",
        json!({
            "page_ids": [], "application_ids": [], "data_model_ids": [],
            "mcp_instance_ids": ["template_instance"]
        }),
    )
    .await;
    assert_eq!(exported.status(), StatusCode::OK);
    let exported: Value =
        serde_json::from_slice(&to_bytes(exported.into_body(), usize::MAX).await.unwrap()).unwrap();
    let mut package = exported["data"].clone();
    assert_eq!(
        package["mcp_bundle"]["instances"].as_array().unwrap().len(),
        1
    );
    assert_eq!(
        package["mcp_bundle"]["instances"][0]["instance_id"],
        "template_instance"
    );

    let deleted = send(
        "DELETE",
        "/api/console/mcp/instances/template_instance",
        json!({}),
    )
    .await;
    assert_eq!(deleted.status(), StatusCode::NO_CONTENT);
    let preview = send(
        "POST",
        "/api/console/settings/system-templates/preview",
        package.clone(),
    )
    .await;
    assert_eq!(preview.status(), StatusCode::OK);
    let preview: Value =
        serde_json::from_slice(&to_bytes(preview.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(preview["data"]["valid"], true);
    assert!(preview["data"]["effects"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item["kind"] == "mcp_instance" && item["action"] == "create"));
    let installed = send(
        "POST",
        "/api/console/settings/system-templates/install",
        package.clone(),
    )
    .await;
    assert_eq!(installed.status(), StatusCode::OK);
    let installed: Value =
        serde_json::from_slice(&to_bytes(installed.into_body(), usize::MAX).await.unwrap())
            .unwrap();
    assert_eq!(installed["data"]["complete"], true);

    package["mcp_bundle"]["instances"][0]["name"] = json!("Updated instance");
    let repeated = send(
        "POST",
        "/api/console/settings/system-templates/install",
        package,
    )
    .await;
    assert_eq!(repeated.status(), StatusCode::OK);
    let repeated: Value =
        serde_json::from_slice(&to_bytes(repeated.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(repeated["data"]["complete"], true);
    assert!(repeated["data"]["updated"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item["kind"] == "mcp_instance" && item["target_id"] == "template_instance"));
}
