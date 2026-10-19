use std::{fs, path::Path};

use crate::_tests::support::{
    login_and_capture_cookie, test_app, test_app_with_database_url, write_provider_manifest_v2,
    write_provider_runtime_script,
};
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use serde_json::{json, Value};
use sqlx::PgPool;
use tower::ServiceExt;

fn create_provider_fixture(root: &Path) {
    fs::create_dir_all(root.join("provider")).unwrap();
    fs::create_dir_all(root.join("bin")).unwrap();
    fs::create_dir_all(root.join("models/llm")).unwrap();
    fs::create_dir_all(root.join("i18n")).unwrap();
    write_provider_manifest_v2(root, "fixture_provider", "Fixture Provider", "0.1.0");
    fs::write(
        root.join("provider/fixture_provider.yaml"),
        r#"provider_code: fixture_provider
display_name: Fixture Provider
protocol: openai_compatible
help_url: https://example.com/help
default_base_url: https://api.example.com
model_discovery: hybrid
config_schema:
  - key: base_url
    type: string
    required: true
  - key: api_key
    type: secret
    required: true
"#,
    )
    .unwrap();
    write_provider_runtime_script(
        &root.join("bin/fixture_provider-provider"),
        "fixture_chat",
        "Fixture Chat",
    );
    fs::write(
        root.join("models/llm/_position.yaml"),
        "items:\n  - fixture_chat\n",
    )
    .unwrap();
    fs::write(
        root.join("models/llm/fixture_chat.yaml"),
        r#"model: fixture_chat
label: Fixture Chat
family: llm
capabilities:
  - stream
"#,
    )
    .unwrap();
    fs::write(
        root.join("i18n/en_US.json"),
        r#"{ "plugin": { "label": "Fixture Provider" } }"#,
    )
    .unwrap();
    fs::write(
        root.join("icon.svg"),
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16"><rect width="16" height="16" rx="4" fill="#111827"/><path d="M4 8h8" stroke="#f9fafb" stroke-width="2" stroke-linecap="round"/></svg>"##,
    )
    .unwrap();
}

fn create_assets_icon_provider_fixture(root: &Path) {
    fs::create_dir_all(root.join("provider")).unwrap();
    fs::create_dir_all(root.join("bin")).unwrap();
    fs::create_dir_all(root.join("models/llm")).unwrap();
    fs::create_dir_all(root.join("i18n")).unwrap();
    fs::create_dir_all(root.join("_assets")).unwrap();
    write_provider_manifest_v2(root, "openai_compatible", "OpenAI Compatible", "0.3.99");
    fs::write(
        root.join("provider/openai_compatible.yaml"),
        r#"provider_code: openai_compatible
display_name: OpenAI Compatible
protocol: openai_compatible
help_url: https://platform.openai.com/docs/api-reference
default_base_url: https://api.openai.com/v1
model_discovery: hybrid
config_schema:
  - key: base_url
    type: string
    required: true
  - key: api_key
    type: secret
    required: true
"#,
    )
    .unwrap();
    write_provider_runtime_script(
        &root.join("bin/openai_compatible-provider"),
        "openai_compatible_chat",
        "OpenAI Compatible Chat",
    );
    fs::write(
        root.join("models/llm/_position.yaml"),
        "items:\n  - openai_compatible_chat\n",
    )
    .unwrap();
    fs::write(
        root.join("models/llm/openai_compatible_chat.yaml"),
        r#"model: openai_compatible_chat
label: OpenAI Compatible Chat
family: llm
capabilities:
  - stream
"#,
    )
    .unwrap();
    fs::write(
        root.join("i18n/en_US.json"),
        r#"{ "plugin": { "label": "OpenAI Compatible" } }"#,
    )
    .unwrap();
    fs::write(
        root.join("_assets/icon.svg"),
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16"><rect width="16" height="16" rx="4" fill="#10b981"/><path d="M4 4l8 8" stroke="#ecfeff" stroke-width="2" stroke-linecap="round"/></svg>"##,
    )
    .unwrap();
}

async fn install_enable_assign_with_package_root(
    app: &axum::Router,
    cookie: &str,
    csrf: &str,
    package_root: &Path,
) -> String {
    let install = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/plugins/install")
                .header("cookie", cookie)
                .header("x-csrf-token", csrf)
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

    let enable = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/console/plugins/{installation_id}/enable"))
                .header("cookie", cookie)
                .header("x-csrf-token", csrf)
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
                .header("cookie", cookie)
                .header("x-csrf-token", csrf)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(assign.status(), StatusCode::OK);

    installation_id
}

async fn install_enable_assign(app: &axum::Router, cookie: &str, csrf: &str) -> String {
    let package_root = std::env::temp_dir().join(format!(
        "model-provider-icon-route-{}",
        uuid::Uuid::now_v7()
    ));
    create_provider_fixture(&package_root);
    install_enable_assign_with_package_root(app, cookie, csrf, &package_root).await
}

async fn install_enable_assign_assets_icon(app: &axum::Router, cookie: &str, csrf: &str) -> String {
    let package_root = std::env::temp_dir().join(format!(
        "model-provider-assets-icon-route-{}",
        uuid::Uuid::now_v7()
    ));
    create_assets_icon_provider_fixture(&package_root);
    install_enable_assign_with_package_root(app, cookie, csrf, &package_root).await
}

#[tokio::test]
async fn model_provider_icon_routes_serve_installed_provider_icon_and_normalize_option_url() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let installation_id = install_enable_assign(&app, &cookie, &csrf).await;

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
                        "display_name": "Fixture Ready",
                        "configured_models": [
                            {
                                "model_id": "fixture_chat",
                                "enabled": true,
                                "context_window_override_tokens": null
                            }
                        ],
                        "enabled_model_ids": ["fixture_chat"],
                        "included_in_main": true,
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

    let options = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/console/model-providers/options")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(options.status(), StatusCode::OK);
    let options_payload: Value =
        serde_json::from_slice(&to_bytes(options.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(
        options_payload["data"]["providers"][0]["icon"].as_str(),
        Some("/api/console/model-providers/providers/fixture_provider/icon")
    );

    let icon = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/console/model-providers/providers/fixture_provider/icon")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(icon.status(), StatusCode::OK);
    assert_eq!(
        icon.headers()
            .get("content-type")
            .and_then(|value| value.to_str().ok()),
        Some("image/svg+xml")
    );
    let icon_body = to_bytes(icon.into_body(), usize::MAX).await.unwrap();
    assert!(String::from_utf8(icon_body.to_vec())
        .unwrap()
        .contains("<svg"));
}

#[tokio::test]
async fn model_provider_icon_routes_read_assets_directory_icon_when_manifest_uses_basename() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let installation_id = install_enable_assign_assets_icon(&app, &cookie, &csrf).await;

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
                        "display_name": "OpenAI Compatible Ready",
                        "configured_models": [
                            {
                                "model_id": "openai_compatible_chat",
                                "enabled": true,
                                "context_window_override_tokens": null
                            }
                        ],
                        "enabled_model_ids": ["openai_compatible_chat"],
                        "included_in_main": true,
                        "config": {
                            "base_url": "https://api.openai.com/v1",
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

    let icon = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/console/model-providers/providers/openai_compatible/icon")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(icon.status(), StatusCode::OK);
    assert_eq!(
        icon.headers()
            .get("content-type")
            .and_then(|value| value.to_str().ok()),
        Some("image/svg+xml")
    );
    let icon_body = to_bytes(icon.into_body(), usize::MAX).await.unwrap();
    assert!(String::from_utf8(icon_body.to_vec())
        .unwrap()
        .contains("<svg"));
}

#[tokio::test]
async fn model_provider_icon_routes_fall_back_to_manifest_icon_when_metadata_is_missing() {
    let (app, database_url) = test_app_with_database_url().await;
    let pool = PgPool::connect(&database_url).await.unwrap();
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let installation_id = install_enable_assign_assets_icon(&app, &cookie, &csrf).await;

    sqlx::query(
        "update extension_installations
         set metadata_json = coalesce(metadata_json, '{}'::jsonb) - 'icon'
         where id = $1",
    )
    .bind(uuid::Uuid::parse_str(&installation_id).unwrap())
    .execute(&pool)
    .await
    .unwrap();

    let icon = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/console/model-providers/providers/openai_compatible/icon")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(icon.status(), StatusCode::OK);
    assert_eq!(
        icon.headers()
            .get("content-type")
            .and_then(|value| value.to_str().ok()),
        Some("image/svg+xml")
    );
    let icon_body = to_bytes(icon.into_body(), usize::MAX).await.unwrap();
    assert!(String::from_utf8(icon_body.to_vec())
        .unwrap()
        .contains("<svg"));
}
