use std::fs;

use crate::_tests::support::{login_and_capture_cookie, test_app};
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use serde_json::{json, Value};
use tower::ServiceExt;

use super::support::{
    build_signed_openai_upload_package, build_signed_openai_upload_package_with_payload,
    build_upload_body, create_host_extension_package, create_member, create_role, pack_tar_gz,
    replace_member_roles, replace_role_permissions,
};

#[tokio::test]
async fn plugin_routes_list_official_catalog_and_install_official_package() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;

    let catalog = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/console/plugins/official-catalog")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(catalog.status(), StatusCode::OK);

    let install = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/plugins/install-official")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "plugin_id": "1flowbase.openai_compatible",
                        "risk_override": {
                            "reason": "route fixture accepts unsigned package",
                            "acknowledged_warnings": ["signature_missing"]
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(install.status(), StatusCode::CREATED);
}

#[tokio::test]
async fn plugin_routes_list_official_catalog_with_source_metadata() {
    let app = test_app().await;
    let (cookie, _csrf) = login_and_capture_cookie(&app, "root", "change-me").await;

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/console/plugins/official-catalog")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let payload: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(payload["data"]["source_kind"], "mirror_registry");
    assert_eq!(payload["data"]["source_label"], "Mirror source");
    assert_eq!(
        payload["data"]["entries"][0]["plugin_id"],
        "1flowbase.openai_compatible"
    );
    assert_eq!(
        payload["data"]["entries"][0]["icon"],
        "https://raw.githubusercontent.com/taichuy/1flowbase-official-plugins/main/runtime-extensions/model-providers/openai_compatible/_assets/icon.svg"
    );
    assert_eq!(
        payload["data"]["entries"][0]["minimum_host_version"],
        "0.1.0"
    );
    assert_eq!(
        payload["data"]["entries"][0]["current_host_version"],
        env!("CARGO_PKG_VERSION")
    );
    assert_eq!(
        payload["data"]["entries"][0]["compatibility_status"],
        "compatible"
    );
    assert!(payload["data"]["entries"][0]["compatibility_warning_reason"].is_null());
}

#[tokio::test]
async fn plugin_routes_list_official_catalog_returns_localized_page_items() {
    let app = test_app().await;
    let (cookie, _csrf) = login_and_capture_cookie(&app, "root", "change-me").await;

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/console/plugins/official-catalog?plugin_type=model_provider&locale=zh_Hans&q=provider&limit=1")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let payload: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(payload["data"]["locale_meta"]["resolved_locale"], "zh_Hans");
    assert!(payload["data"].get("i18n_catalog").is_none());
    assert_eq!(payload["data"]["page"]["limit"], 1);
    assert!(payload["data"]["page"]["next_cursor"].is_null());
    assert_eq!(
        payload["data"]["entries"][0]["display_name"],
        "OpenAI Compatible"
    );
    assert_eq!(
        payload["data"]["entries"][0]["description"],
        "官方 Provider 插件"
    );
    assert_eq!(
        payload["data"]["entries"][0]["selected_artifact"]["download_url"],
        "https://example.com/openai-compatible.1flowbasepkg"
    );
}

#[tokio::test]
async fn plugin_routes_install_upload_accepts_multipart_package() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let boundary = "----1flowbase-test-boundary";
    let package_bytes = build_signed_openai_upload_package("0.2.0");
    let body = build_upload_body(
        boundary,
        "openai_compatible-0.2.0.1flowbasepkg",
        &package_bytes,
    );

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/plugins/install-upload")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header(
                    "content-type",
                    format!("multipart/form-data; boundary={boundary}"),
                )
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);

    let payload: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(payload["data"]["installation"]["source_kind"], "uploaded");
    assert_eq!(
        payload["data"]["installation"]["signature_status"],
        "verified"
    );

    let installed = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/console/settings/extension-center/installed?category=runtime-extensions")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(installed.status(), StatusCode::OK);
    let installed_payload: Value =
        serde_json::from_slice(&to_bytes(installed.into_body(), usize::MAX).await.unwrap())
            .unwrap();
    assert!(installed_payload["data"]["entries"]
        .as_array()
        .unwrap()
        .iter()
        .any(|entry| {
            entry["artifact_id"] == "openai_compatible" && entry["version"] == "0.2.0"
        }));
}

#[tokio::test]
async fn plugin_routes_install_upload_accepts_official_package_larger_than_default_body_limit() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let boundary = "----1flowbase-large-plugin-boundary";
    let package_bytes = build_signed_openai_upload_package_with_payload("0.2.1", 3 * 1024 * 1024);
    assert!(package_bytes.len() > 2 * 1024 * 1024);
    let body = build_upload_body(
        boundary,
        "openai_compatible-0.2.1.1flowbasepkg",
        &package_bytes,
    );

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/plugins/install-upload")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header(
                    "content-type",
                    format!("multipart/form-data; boundary={boundary}"),
                )
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
}

#[tokio::test]
async fn plugin_routes_install_upload_rejects_payload_above_route_limit() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let boundary = "----1flowbase-oversize-plugin-boundary";
    let body = build_upload_body(
        boundary,
        "oversize.1flowbasepkg",
        &vec![0_u8; 8 * 1024 * 1024],
    );

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/plugins/install-upload")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header(
                    "content-type",
                    format!("multipart/form-data; boundary={boundary}"),
                )
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
}

#[tokio::test]
async fn model_provider_settings_install_upload_accepts_package_larger_than_default_body_limit() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let boundary = "----1flowbase-large-settings-plugin-boundary";
    let package_bytes = build_signed_openai_upload_package_with_payload("0.2.2", 3 * 1024 * 1024);
    assert!(package_bytes.len() > 2 * 1024 * 1024);
    let body = build_upload_body(
        boundary,
        "openai_compatible-0.2.2.1flowbasepkg",
        &package_bytes,
    );

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/settings/model-providers/plugins/install-upload")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header(
                    "content-type",
                    format!("multipart/form-data; boundary={boundary}"),
                )
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);

    let installed = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/console/settings/extension-center/installed?category=runtime-extensions")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(installed.status(), StatusCode::OK);
    let installed_payload: Value =
        serde_json::from_slice(&to_bytes(installed.into_body(), usize::MAX).await.unwrap())
            .unwrap();
    assert!(installed_payload["data"]["entries"]
        .as_array()
        .unwrap()
        .iter()
        .any(|entry| {
            entry["artifact_id"] == "openai_compatible" && entry["version"] == "0.2.2"
        }));
}

#[tokio::test]
async fn model_provider_settings_install_upload_rejects_payload_above_route_limit() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let boundary = "----1flowbase-oversize-settings-plugin-boundary";
    let body = build_upload_body(
        boundary,
        "oversize.1flowbasepkg",
        &vec![0_u8; 8 * 1024 * 1024],
    );

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/settings/model-providers/plugins/install-upload")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header(
                    "content-type",
                    format!("multipart/form-data; boundary={boundary}"),
                )
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
}

#[tokio::test]
async fn plugin_routes_install_upload_persists_host_extension_as_pending_restart() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let boundary = "----1flowbase-host-extension-boundary";
    let package_root =
        std::env::temp_dir().join(format!("host-extension-route-{}", uuid::Uuid::now_v7()));
    create_host_extension_package(&package_root, "0.1.0");
    let package_bytes = pack_tar_gz(&package_root);
    let body = build_upload_body(
        boundary,
        "fixture_host_extension-0.1.0.1flowbasepkg",
        &package_bytes,
    );

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/plugins/install-upload")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header(
                    "content-type",
                    format!("multipart/form-data; boundary={boundary}"),
                )
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);

    let payload: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(
        payload["data"]["installation"]["desired_state"],
        "pending_restart"
    );
    assert_eq!(
        payload["data"]["installation"]["runtime_status"],
        "inactive"
    );
    assert_eq!(
        payload["data"]["installation"]["availability_status"],
        "pending_restart"
    );
    assert_eq!(
        payload["data"]["task"]["status_message"],
        "installed; restart required"
    );

    let _ = fs::remove_dir_all(package_root);
}

#[tokio::test]
async fn plugin_routes_forbid_non_root_host_extension_upload() {
    let app = test_app().await;
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;

    create_role(&app, &root_cookie, &root_csrf, "plugin_manager").await;
    replace_role_permissions(
        &app,
        &root_cookie,
        &root_csrf,
        "plugin_manager",
        &["plugin_config.configure.all"],
    )
    .await;
    let member_id = create_member(
        &app,
        &root_cookie,
        &root_csrf,
        "plugin-manager",
        "change-me",
    )
    .await;
    replace_member_roles(
        &app,
        &root_cookie,
        &root_csrf,
        &member_id,
        &["plugin_manager"],
    )
    .await;
    let (manager_cookie, manager_csrf) =
        login_and_capture_cookie(&app, "plugin-manager", "change-me").await;

    let boundary = "----1flowbase-host-extension-forbidden-boundary";
    let package_root = std::env::temp_dir().join(format!(
        "host-extension-route-forbidden-{}",
        uuid::Uuid::now_v7()
    ));
    create_host_extension_package(&package_root, "0.1.0");
    let package_bytes = pack_tar_gz(&package_root);
    let body = build_upload_body(
        boundary,
        "fixture_host_extension-0.1.0.1flowbasepkg",
        &package_bytes,
    );

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/plugins/install-upload")
                .header("cookie", &manager_cookie)
                .header("x-csrf-token", &manager_csrf)
                .header(
                    "content-type",
                    format!("multipart/form-data; boundary={boundary}"),
                )
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);

    let _ = fs::remove_dir_all(package_root);
}
