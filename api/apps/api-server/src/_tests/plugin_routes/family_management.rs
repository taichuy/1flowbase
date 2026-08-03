use std::path::Path;

use crate::_tests::support::{login_and_capture_cookie, test_app};
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use serde_json::{json, Value};
use tower::ServiceExt;

use super::support::{create_fixture_provider_package, create_openai_compatible_package};

#[tokio::test]
async fn plugin_routes_list_families_and_switch_local_version() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let package_root_v1 =
        std::env::temp_dir().join(format!("plugin-route-switch-v1-{}", uuid::Uuid::now_v7()));
    let package_root_v2 =
        std::env::temp_dir().join(format!("plugin-route-switch-v2-{}", uuid::Uuid::now_v7()));
    create_fixture_provider_package(&package_root_v1, "0.1.0");
    create_fixture_provider_package(&package_root_v2, "0.2.0");

    let install_v1 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/plugins/install")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "package_root": package_root_v1.display().to_string() }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(install_v1.status(), StatusCode::CREATED);
    let install_v1_payload: Value =
        serde_json::from_slice(&to_bytes(install_v1.into_body(), usize::MAX).await.unwrap())
            .unwrap();
    let installation_v1_id = install_v1_payload["data"]["installation"]["id"]
        .as_str()
        .unwrap()
        .to_string();

    let install_v2 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/plugins/install")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "package_root": package_root_v2.display().to_string() }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(install_v2.status(), StatusCode::CREATED);
    let install_v2_payload: Value =
        serde_json::from_slice(&to_bytes(install_v2.into_body(), usize::MAX).await.unwrap())
            .unwrap();
    let installation_v2_id = install_v2_payload["data"]["installation"]["id"]
        .as_str()
        .unwrap()
        .to_string();

    let enable_v1 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/console/plugins/{installation_v1_id}/enable"))
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(enable_v1.status(), StatusCode::OK);

    let assign_v1 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/console/plugins/{installation_v1_id}/assign"))
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(assign_v1.status(), StatusCode::OK);

    let list_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/console/plugins/families")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(list_response.status(), StatusCode::OK);

    let switch_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/plugins/families/fixture_provider/switch-version")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "installation_id": installation_v2_id }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(switch_response.status(), StatusCode::OK);
}

#[tokio::test]
async fn plugin_routes_delete_family_rejects_current_and_referenced_installations() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let package_root_v1 =
        std::env::temp_dir().join(format!("plugin-route-delete-v1-{}", uuid::Uuid::now_v7()));
    let package_root_v2 =
        std::env::temp_dir().join(format!("plugin-route-delete-v2-{}", uuid::Uuid::now_v7()));
    create_fixture_provider_package(&package_root_v1, "0.1.0");
    create_fixture_provider_package(&package_root_v2, "0.2.0");

    let install_v1 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/plugins/install")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "package_root": package_root_v1.display().to_string() }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(install_v1.status(), StatusCode::CREATED);
    let install_v1_payload: Value =
        serde_json::from_slice(&to_bytes(install_v1.into_body(), usize::MAX).await.unwrap())
            .unwrap();
    let installation_v1_id = install_v1_payload["data"]["installation"]["id"]
        .as_str()
        .unwrap()
        .to_string();
    let install_v1_path = install_v1_payload["data"]["installation"]["local_artifact"]
        ["local_path"]
        .as_str()
        .unwrap()
        .to_string();

    let install_v2 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/plugins/install")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "package_root": package_root_v2.display().to_string() }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(install_v2.status(), StatusCode::CREATED);
    let install_v2_payload: Value =
        serde_json::from_slice(&to_bytes(install_v2.into_body(), usize::MAX).await.unwrap())
            .unwrap();
    let install_v2_path = install_v2_payload["data"]["installation"]["local_artifact"]
        ["local_path"]
        .as_str()
        .unwrap()
        .to_string();

    let enable_v1 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/console/plugins/{installation_v1_id}/enable"))
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(enable_v1.status(), StatusCode::OK);

    let assign_v1 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/console/plugins/{installation_v1_id}/assign"))
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(assign_v1.status(), StatusCode::OK);

    let create_instance = app
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
                        "installation_id": installation_v1_id,
                        "display_name": "Fixture Provider Prod",
                        "config": {
                            "base_url": "https://api.example.com",
                            "api_key": "secret"
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_instance.status(), StatusCode::CREATED);

    let delete_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/console/plugins/families/fixture_provider")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(delete_response.status(), StatusCode::CONFLICT);

    let families_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/console/plugins/families")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(families_response.status(), StatusCode::OK);
    let families_payload: Value = serde_json::from_slice(
        &to_bytes(families_response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        families_payload["data"]["entries"]
            .as_array()
            .unwrap()
            .len(),
        1
    );

    let instances_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/console/settings/model-providers/instances")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(instances_response.status(), StatusCode::OK);
    let instances_payload: Value = serde_json::from_slice(
        &to_bytes(instances_response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(instances_payload["data"].as_array().unwrap().len(), 1);

    assert!(Path::new(&install_v1_path).exists());
    assert!(Path::new(&install_v2_path).exists());
}

#[tokio::test]
async fn plugin_routes_upgrade_family_to_latest_official_version() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let package_root_v1 =
        std::env::temp_dir().join(format!("plugin-route-openai-v1-{}", uuid::Uuid::now_v7()));
    create_openai_compatible_package(&package_root_v1, "0.1.0");

    let install_v1 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/plugins/install")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "package_root": package_root_v1.display().to_string() }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(install_v1.status(), StatusCode::CREATED);
    let install_v1_payload: Value =
        serde_json::from_slice(&to_bytes(install_v1.into_body(), usize::MAX).await.unwrap())
            .unwrap();
    let installation_v1_id = install_v1_payload["data"]["installation"]["id"]
        .as_str()
        .unwrap()
        .to_string();

    let enable_v1 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/console/plugins/{installation_v1_id}/enable"))
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(enable_v1.status(), StatusCode::OK);

    let assign_v1 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/console/plugins/{installation_v1_id}/assign"))
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(assign_v1.status(), StatusCode::OK);

    let upgrade_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/plugins/families/openai_compatible/upgrade-latest")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "risk_override": {
                            "reason": "route fixture accepts unsigned official package",
                            "acknowledged_warnings": ["signature_missing"]
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(upgrade_response.status(), StatusCode::OK);
}
