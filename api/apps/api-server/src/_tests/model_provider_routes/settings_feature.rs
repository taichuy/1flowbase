use super::*;
use crate::_tests::support::{
    build_official_provider_package, create_member, create_role, login_and_capture_cookie,
    replace_member_roles, replace_role_permissions, test_app_with_database_url,
};

const MODEL_PROVIDERS_FEATURE_PERMISSION: &str = "settings_feature.access.system.model-providers";

async fn register_model_providers_feature_permission(database_url: &str) {
    let store = storage_durable::build_main_durable_postgres(database_url)
        .await
        .expect("test database should be available")
        .store;
    store
        .upsert_permission_catalog(&[domain::PermissionDefinition {
            code: MODEL_PROVIDERS_FEATURE_PERMISSION.to_string(),
            resource: "settings_feature".to_string(),
            action: "access".to_string(),
            scope: "system.model-providers".to_string(),
            name: "settings_feature:access:system.model-providers".to_string(),
        }])
        .await
        .expect("model-providers feature permission should be seeded");
}

async fn feature_actor(app: &axum::Router, root_cookie: &str, root_csrf: &str) -> (String, String) {
    create_role(app, root_cookie, root_csrf, "model_providers_feature_only").await;
    replace_role_permissions(
        app,
        root_cookie,
        root_csrf,
        "model_providers_feature_only",
        &[MODEL_PROVIDERS_FEATURE_PERMISSION],
    )
    .await;
    let actor_id = create_member(
        app,
        root_cookie,
        root_csrf,
        "model-providers-feature-actor",
        "temp-pass",
    )
    .await;
    replace_member_roles(
        app,
        root_cookie,
        root_csrf,
        &actor_id,
        &["model_providers_feature_only"],
    )
    .await;
    login_and_capture_cookie(app, "model-providers-feature-actor", "temp-pass").await
}

// AC-003/AC-004/AC-006: one SettingsFeature owns the page's catalog and instance
// use cases, while list responses remain redacted and invalid distribution state
// remains rejected by the existing model-provider service boundary.
#[tokio::test]
async fn model_providers_feature_only_lists_catalog_and_redacted_instances() {
    let (app, database_url) = test_app_with_database_url().await;
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    register_model_providers_feature_permission(&database_url).await;
    let installation_id = install_enable_assign(&app, &root_cookie, &root_csrf).await;
    let (actor_cookie, actor_csrf) = feature_actor(&app, &root_cookie, &root_csrf).await;

    let create = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/settings/model-providers/instances")
                .header("cookie", &actor_cookie)
                .header("x-csrf-token", &actor_csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "installation_id": installation_id,
                        "display_name": "Feature fixture",
                        "enabled_model_ids": [],
                        "config": {
                            "base_url": "https://api.example.com",
                            "api_key": "feature-super-secret"
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create.status(), StatusCode::CREATED);
    let create_payload = response_json(create).await;
    let instance_id = create_payload["data"]["id"].as_str().unwrap().to_string();
    let catalog = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/console/settings/model-providers/catalog?locale=zh_Hans")
                .header("cookie", &actor_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(catalog.status(), StatusCode::OK);
    let catalog_payload: Value =
        serde_json::from_slice(&to_bytes(catalog.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(
        catalog_payload["data"]["entries"][0]["provider_code"].as_str(),
        Some("fixture_provider")
    );
    assert!(!catalog_payload.to_string().contains("feature-super-secret"));

    let instances = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/console/settings/model-providers/instances")
                .header("cookie", &actor_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(instances.status(), StatusCode::OK);
    let instances_payload: Value =
        serde_json::from_slice(&to_bytes(instances.into_body(), usize::MAX).await.unwrap())
            .unwrap();
    assert_eq!(
        instances_payload["data"][0]["config_json"]["api_key"].as_str(),
        Some("feat****cret")
    );
    assert_eq!(
        instances_payload["data"][0]["status"].as_str(),
        Some("draft")
    );
    assert!(!instances_payload
        .to_string()
        .contains("feature-super-secret"));

    let reveal = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/console/settings/model-providers/instances/{instance_id}/secrets/reveal"
                ))
                .header("cookie", &actor_cookie)
                .header("x-csrf-token", &actor_csrf)
                .header("content-type", "application/json")
                .body(Body::from(json!({ "key": "api_key" }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(reveal.status(), StatusCode::OK);
    assert_eq!(
        response_json(reveal).await["data"]["value"],
        "feature-super-secret"
    );

    let validate = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/console/settings/model-providers/instances/{instance_id}/validate"
                ))
                .header("cookie", &actor_cookie)
                .header("x-csrf-token", &actor_csrf)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(validate.status(), StatusCode::OK);
    let validate_payload = response_json(validate).await;
    assert_eq!(validate_payload["data"]["instance"]["status"], "draft");
    assert_eq!(
        validate_payload["data"]["output"]["sanitized"]["api_key"],
        "***"
    );

    for (method, path) in [
        (
            "GET",
            format!("/api/console/settings/model-providers/instances/{instance_id}/models"),
        ),
        (
            "POST",
            format!("/api/console/settings/model-providers/instances/{instance_id}/models/refresh"),
        ),
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri(path)
                    .header("cookie", &actor_cookie)
                    .header("x-csrf-token", &actor_csrf)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK, "{method}");
    }

    let preview = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/settings/model-providers/preview-models")
                .header("cookie", &actor_cookie)
                .header("x-csrf-token", &actor_csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "installation_id": installation_id,
                        "instance_id": null,
                        "config": {
                            "base_url": "https://api.example.com",
                            "api_key": "preview-secret"
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(preview.status(), StatusCode::OK);

    let update = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!(
                    "/api/console/settings/model-providers/instances/{instance_id}"
                ))
                .header("cookie", &actor_cookie)
                .header("x-csrf-token", &actor_csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "display_name": "Feature fixture updated",
                        "configured_models": [],
                        "enabled_model_ids": [],
                        "included_in_main": true,
                        "preview_token": null,
                        "config": {
                            "base_url": "https://api.example.com",
                            "api_key": "feature-super-secret"
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(update.status(), StatusCode::OK);

    let valid_distribution = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(
                    "/api/console/settings/model-providers/providers/fixture_provider/main-instance",
                )
                .header("cookie", &actor_cookie)
                .header("x-csrf-token", &actor_csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "auto_include_new_instances": true,
                        "model_distribution_rules": [{
                            "model_id": "fixture_chat",
                            "distribution_rule": "none"
                        }]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(valid_distribution.status(), StatusCode::OK);

    let main_instance = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(
                    "/api/console/settings/model-providers/providers/fixture_provider/main-instance",
                )
                .header("cookie", &actor_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(main_instance.status(), StatusCode::OK);

    let invalid_distribution = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(
                    "/api/console/settings/model-providers/providers/fixture_provider/main-instance",
                )
                .header("cookie", &actor_cookie)
                .header("x-csrf-token", &actor_csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "auto_include_new_instances": true,
                        "model_distribution_rules": [{
                            "model_id": "fixture_chat",
                            "distribution_rule": "illegal"
                        }]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(invalid_distribution.status(), StatusCode::BAD_REQUEST);

    let delete = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!(
                    "/api/console/settings/model-providers/instances/{instance_id}"
                ))
                .header("cookie", actor_cookie)
                .header("x-csrf-token", actor_csrf)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(delete.status(), StatusCode::OK);
}

// AC-002/AC-003/AC-006/AC-011: Settings ownership is exclusive, while the
// Agent Flow options route and generic plugin-management HTTP keep Action ACL.
#[tokio::test]
async fn model_providers_settings_routes_do_not_take_over_business_consumers() {
    let (app, database_url) = test_app_with_database_url().await;
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    register_model_providers_feature_permission(&database_url).await;
    let (feature_cookie, feature_csrf) = feature_actor(&app, &root_cookie, &root_csrf).await;

    for path in [
        "/api/console/settings/model-providers/catalog",
        "/api/console/settings/model-providers/options",
        "/api/console/settings/model-providers/request-logs",
        "/api/console/settings/model-providers/plugins/families?plugin_type=data_source",
        "/api/console/settings/model-providers/plugins/official-catalog?plugin_type=data_source",
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(path)
                    .header("cookie", &feature_cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK, "{path}");
        let payload: Value = response_json(response).await;
        if path.contains("official-catalog") {
            assert!(payload["data"]["entries"]
                .as_array()
                .unwrap()
                .iter()
                .all(|entry| entry["plugin_type"] == "model_provider"));
        }
    }

    let delete_empty = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/console/settings/model-providers/request-logs")
                .header("cookie", &feature_cookie)
                .header("x-csrf-token", &feature_csrf)
                .header("content-type", "application/json")
                .body(Body::from(json!({ "attempt_ids": [] }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(delete_empty.status(), StatusCode::BAD_REQUEST);

    let clear = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/settings/model-providers/request-logs/clear")
                .header("cookie", &feature_cookie)
                .header("x-csrf-token", &feature_csrf)
                .header("content-type", "application/json")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(clear.status(), StatusCode::OK);

    for path in [
        "/api/console/model-providers/options",
        "/api/console/model-providers/providers/not-assigned/icon",
        "/api/console/model-providers/00000000-0000-0000-0000-000000000001/balance",
        "/api/console/plugins/families?plugin_type=model_provider",
        "/api/console/settings/model-providers/unregistered",
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(path)
                    .header("cookie", &feature_cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN, "{path}");
    }

    for old_settings_path in [
        "/api/console/model-providers/catalog",
        "/api/console/model-providers",
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(old_settings_path)
                    .header("cookie", &root_cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            response.status(),
            StatusCode::NOT_FOUND,
            "{old_settings_path}"
        );
    }

    create_role(
        &app,
        &root_cookie,
        &root_csrf,
        "legacy_model_provider_actions",
    )
    .await;
    replace_role_permissions(
        &app,
        &root_cookie,
        &root_csrf,
        "legacy_model_provider_actions",
        &["state_model.view.all", "plugin_config.view.all"],
    )
    .await;
    let legacy_actor_id = create_member(
        &app,
        &root_cookie,
        &root_csrf,
        "legacy-model-provider-action-actor",
        "temp-pass",
    )
    .await;
    replace_member_roles(
        &app,
        &root_cookie,
        &root_csrf,
        &legacy_actor_id,
        &["legacy_model_provider_actions"],
    )
    .await;
    let (legacy_cookie, _) =
        login_and_capture_cookie(&app, "legacy-model-provider-action-actor", "temp-pass").await;

    let settings_denied = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/console/settings/model-providers/catalog")
                .header("cookie", &legacy_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(settings_denied.status(), StatusCode::FORBIDDEN);

    for business_path in [
        "/api/console/model-providers/options",
        "/api/console/plugins/families?plugin_type=model_provider",
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(business_path)
                    .header("cookie", &legacy_cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK, "{business_path}");
    }
}

// AC-003/AC-004/AC-006: feature-only actors can complete the plugin-backed page
// lifecycle without plugin_config grants; existing desired/availability/version
// constraints still execute inside PluginManagementService.
#[tokio::test]
async fn model_providers_feature_only_completes_plugin_install_and_family_lifecycle() {
    let (app, database_url) = test_app_with_database_url().await;
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    register_model_providers_feature_permission(&database_url).await;
    let (actor_cookie, actor_csrf) = feature_actor(&app, &root_cookie, &root_csrf).await;

    let boundary = "----1flowbase-model-provider-settings";
    let package_bytes = build_official_provider_package("0.1.0");
    let upload = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/settings/model-providers/plugins/install-upload")
                .header("cookie", &actor_cookie)
                .header("x-csrf-token", &actor_csrf)
                .header(
                    "content-type",
                    format!("multipart/form-data; boundary={boundary}"),
                )
                .body(Body::from(upload_body(
                    boundary,
                    "openai_compatible-0.1.0.1flowbasepkg",
                    &package_bytes,
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(upload.status(), StatusCode::CREATED);
    let upload_payload = response_json(upload).await;
    let old_installation_id = upload_payload["data"]["installation"]["id"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(
        upload_payload["data"]["installation"]["runtime_slot"],
        "model_provider"
    );

    let install_official = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/settings/model-providers/plugins/install-official")
                .header("cookie", &actor_cookie)
                .header("x-csrf-token", &actor_csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "plugin_id": "1flowbase.openai_compatible" }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(install_official.status(), StatusCode::CREATED);
    let official_payload = response_json(install_official).await;
    let current_installation_id = official_payload["data"]["installation"]["id"]
        .as_str()
        .unwrap()
        .to_string();
    let task_id = official_payload["data"]["task"]["id"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(
        official_payload["data"]["installation"]["availability_status"],
        "available"
    );

    for suffix in ["artifact/refresh", "artifact/install-current-node"] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!(
                        "/api/console/settings/model-providers/plugins/{current_installation_id}/{suffix}"
                    ))
                    .header("cookie", &actor_cookie)
                    .header("x-csrf-token", &actor_csrf)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK, "{suffix}");
    }

    let task = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/settings/model-providers/plugins/tasks/{task_id}"
                ))
                .header("cookie", &actor_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(task.status(), StatusCode::OK);

    let switch = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/settings/model-providers/plugins/families/openai_compatible/switch-version")
                .header("cookie", &actor_cookie)
                .header("x-csrf-token", &actor_csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "installation_id": old_installation_id }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(switch.status(), StatusCode::OK);

    let upgrade = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/settings/model-providers/plugins/families/openai_compatible/upgrade-latest")
                .header("cookie", &actor_cookie)
                .header("x-csrf-token", &actor_csrf)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(upgrade.status(), StatusCode::OK);

    let delete = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/console/settings/model-providers/plugins/families/openai_compatible")
                .header("cookie", actor_cookie)
                .header("x-csrf-token", actor_csrf)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(delete.status(), StatusCode::OK);
}

fn upload_body(boundary: &str, file_name: &str, package_bytes: &[u8]) -> Vec<u8> {
    let mut body = Vec::new();
    body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
    body.extend_from_slice(
        format!("Content-Disposition: form-data; name=\"file\"; filename=\"{file_name}\"\r\n")
            .as_bytes(),
    );
    body.extend_from_slice(b"Content-Type: application/octet-stream\r\n\r\n");
    body.extend_from_slice(package_bytes);
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    body
}

#[tokio::test]
async fn model_providers_compiled_inventory_matches_openapi() {
    let registration = access_control::core_settings_feature_registrations()
        .into_iter()
        .find(|registration| registration.feature_id == "system.model-providers")
        .expect("model-providers SettingsFeature should be registered");
    assert_eq!(registration.api_routes.len(), 26);
    let openapi = openapi_payload().await;
    for route in registration.api_routes {
        let method = route.method.to_ascii_lowercase();
        assert!(
            openapi["paths"][&route.path][&method].is_object(),
            "missing OpenAPI operation {} {}",
            route.method,
            route.path
        );
    }
    assert!(openapi["paths"]["/api/console/model-providers/catalog"].is_null());
    assert!(openapi["paths"]["/api/console/model-providers"].is_null());
    assert!(openapi["paths"]["/api/console/model-providers/options"]["get"].is_object());
    assert!(openapi["paths"]["/api/console/model-providers/{id}/balance"]["get"].is_object());
}

async fn response_json(response: axum::response::Response) -> Value {
    serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap()
}
