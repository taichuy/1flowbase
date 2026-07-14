use std::path::Path;

use crate::_tests::support::{login_and_capture_cookie, test_app};
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use serde_json::{json, Value};
use tower::ServiceExt;

use super::support::{
    create_fixture_provider_package, create_member, create_role, replace_member_roles,
    replace_role_permissions,
};

#[tokio::test]
async fn plugin_routes_install_enable_assign_and_query_tasks() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let package_root = std::env::temp_dir().join(format!("plugin-route-{}", uuid::Uuid::now_v7()));
    create_fixture_provider_package(&package_root, "0.1.0");

    let install = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/plugins/install")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "package_root": package_root.display().to_string() }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(install.status(), StatusCode::CREATED);
    let install_payload: Value =
        serde_json::from_slice(&to_bytes(install.into_body(), usize::MAX).await.unwrap()).unwrap();
    let installation_id = install_payload["data"]["installation"]["id"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(
        install_payload["data"]["installation"]["plugin_id"],
        "fixture_provider@0.1.0"
    );
    assert!(install_payload["data"]["task"]["id"].as_str().is_some());
    assert_eq!(
        install_payload["data"]["installation"]["source_kind"],
        "uploaded"
    );
    assert_eq!(
        install_payload["data"]["installation"]["signature_status"],
        "unsigned"
    );
    assert_eq!(
        install_payload["data"]["installation"]["desired_state"],
        "disabled"
    );
    assert_eq!(
        install_payload["data"]["installation"]["artifact_status"],
        "ready"
    );
    assert_eq!(
        install_payload["data"]["installation"]["runtime_status"],
        "inactive"
    );
    assert_eq!(
        install_payload["data"]["installation"]["availability_status"],
        "disabled"
    );
    assert!(install_payload["data"]["installation"]["package_path"].is_null());
    assert!(install_payload["data"]["installation"]["manifest_fingerprint"].is_string());
    assert!(install_payload["data"]["installation"]["last_load_error"].is_null());
    let installed_path = install_payload["data"]["installation"]["installed_path"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(!Path::new(&installed_path).join("demo").exists());
    assert!(!Path::new(&installed_path).join("scripts").exists());

    let refresh_projection = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/console/plugins/{installation_id}/catalog-projection/refresh"
                ))
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(refresh_projection.status(), StatusCode::OK);
    let refresh_payload: Value = serde_json::from_slice(
        &to_bytes(refresh_projection.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        refresh_payload["data"]["projection_status"].as_str(),
        Some("ok")
    );
    assert_eq!(
        refresh_payload["data"]["package_code"].as_str(),
        Some("fixture_provider")
    );

    let enable = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/console/plugins/{installation_id}/enable"))
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(enable.status(), StatusCode::OK);

    let assign = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/console/plugins/{installation_id}/assign"))
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(assign.status(), StatusCode::OK);

    let catalog = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/console/plugins/catalog")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(catalog.status(), StatusCode::OK);
    let catalog_payload: Value =
        serde_json::from_slice(&to_bytes(catalog.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(
        catalog_payload["data"]["entries"].as_array().unwrap().len(),
        1
    );
    assert_eq!(
        catalog_payload["data"]["entries"][0]["assigned_to_current_workspace"].as_bool(),
        Some(true)
    );
    assert_eq!(
        catalog_payload["data"]["entries"][0]["plugin_type"],
        "model_provider"
    );
    assert_eq!(
        catalog_payload["data"]["entries"][0]["namespace"],
        "plugin.fixture_provider"
    );
    assert_eq!(
        catalog_payload["data"]["entries"][0]["catalog_refresh_status"].as_str(),
        Some("ok")
    );
    assert_eq!(
        catalog_payload["data"]["entries"][0]["label_key"],
        "plugin.label"
    );
    assert!(catalog_payload["data"]["entries"][0]
        .get("display_name")
        .is_none());
    assert!(
        catalog_payload["data"]["i18n_catalog"]["plugin.fixture_provider"]["en_US"].is_object()
    );

    let tasks = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/console/plugins/tasks")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(tasks.status(), StatusCode::OK);
    let tasks_payload: Value =
        serde_json::from_slice(&to_bytes(tasks.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(tasks_payload["data"].as_array().unwrap().len(), 3);
    let task_id = tasks_payload["data"][0]["id"].as_str().unwrap().to_string();

    let task = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/console/plugins/tasks/{task_id}"))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(task.status(), StatusCode::OK);
}

#[tokio::test]
async fn plugin_routes_expose_slot_kind_without_breaking_provider_code() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let package_root =
        std::env::temp_dir().join(format!("plugin-route-slot-{}", uuid::Uuid::now_v7()));
    create_fixture_provider_package(&package_root, "0.1.0");

    let install = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/plugins/install")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "package_root": package_root.display().to_string() }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(install.status(), StatusCode::CREATED);
    let payload: Value =
        serde_json::from_slice(&to_bytes(install.into_body(), usize::MAX).await.unwrap()).unwrap();

    let installation = &payload["data"]["installation"];
    assert_eq!(installation["provider_code"], "fixture_provider");
    assert_eq!(installation["runtime_slot"], "model_provider");
}

#[tokio::test]
async fn plugin_routes_allow_view_only_users_to_read_but_not_install() {
    let app = test_app().await;
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;

    create_role(&app, &root_cookie, &root_csrf, "plugin_viewer").await;
    replace_role_permissions(
        &app,
        &root_cookie,
        &root_csrf,
        "plugin_viewer",
        &["plugin_config.view.all"],
    )
    .await;
    let member_id =
        create_member(&app, &root_cookie, &root_csrf, "plugin-viewer", "change-me").await;
    replace_member_roles(
        &app,
        &root_cookie,
        &root_csrf,
        &member_id,
        &["plugin_viewer"],
    )
    .await;

    let (viewer_cookie, viewer_csrf) =
        login_and_capture_cookie(&app, "plugin-viewer", "change-me").await;

    let catalog = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/console/plugins/catalog")
                .header("cookie", &viewer_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(catalog.status(), StatusCode::OK);

    let denied = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/plugins/install")
                .header("cookie", &viewer_cookie)
                .header("x-csrf-token", &viewer_csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "package_root": "/tmp/none" }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(denied.status(), StatusCode::FORBIDDEN);
}
