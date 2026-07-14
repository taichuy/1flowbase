use crate::_tests::support::{
    create_member, create_role, login_and_capture_cookie, replace_member_roles,
    replace_role_permissions, test_api_state_with_database_url, test_app, test_config,
};
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use serde_json::Value;
use tower::ServiceExt;

async fn get_console_navigation(
    app: &axum::Router,
    cookie: &str,
) -> (StatusCode, serde_json::Value) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/console/navigation")
                .header("cookie", cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let status = response.status();
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload = serde_json::from_slice(&body).unwrap_or(Value::Null);

    (status, payload)
}

fn string_values(payload: &Value, path: &[&str], key: &str) -> Vec<String> {
    path.iter()
        .fold(payload, |current, segment| &current[*segment])
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry[key].as_str().unwrap().to_string())
        .collect()
}

#[tokio::test]
async fn console_navigation_route_returns_root_registry_with_separated_arrays() {
    let app = test_app().await;
    let (cookie, _) = login_and_capture_cookie(&app, "root", "change-me").await;

    let (status, payload) = get_console_navigation(&app, &cookie).await;

    assert_eq!(status, StatusCode::OK);
    assert!(payload["data"]["route_definitions"].is_array());
    assert!(payload["data"]["navigation_items"].is_array());
    assert!(payload["data"]["permission_bindings"].is_array());
    assert_eq!(
        payload["data"]["route_definitions"][0]["surface_kind"],
        "system"
    );

    let route_ids = string_values(&payload, &["data", "route_definitions"], "route_id");
    assert!(route_ids.contains(&"home".to_string()));
    assert!(route_ids.contains(&"settings.roles".to_string()));

    let item_ids = string_values(&payload, &["data", "navigation_items"], "item_id");
    assert!(item_ids.contains(&"settings".to_string()));
    assert!(item_ids.contains(&"settings.api-key-authentication".to_string()));
    assert!(item_ids.contains(&"settings.applications".to_string()));
}

#[tokio::test]
async fn console_navigation_route_returns_admin_registry_with_builtin_permissions() {
    let app = test_app().await;
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let member_id = create_member(
        &app,
        &root_cookie,
        &root_csrf,
        "navigation-admin",
        "temp-pass",
    )
    .await;
    replace_member_roles(&app, &root_cookie, &root_csrf, &member_id, &["admin"]).await;
    let (admin_cookie, _) = login_and_capture_cookie(&app, "navigation-admin", "temp-pass").await;

    let (status, payload) = get_console_navigation(&app, &admin_cookie).await;

    assert_eq!(status, StatusCode::OK);
    let item_ids = string_values(&payload, &["data", "navigation_items"], "item_id");
    assert!(item_ids.contains(&"home".to_string()));
    assert!(item_ids.contains(&"templates".to_string()));
    assert!(item_ids.contains(&"settings.docs".to_string()));
    assert!(item_ids.contains(&"settings.roles".to_string()));
}

#[tokio::test]
async fn console_navigation_route_trims_limited_member_registry() {
    let app = test_app().await;
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let member_id = create_member(
        &app,
        &root_cookie,
        &root_csrf,
        "navigation-member",
        "temp-pass",
    )
    .await;
    create_role(&app, &root_cookie, &root_csrf, "navigation_limited").await;
    replace_role_permissions(
        &app,
        &root_cookie,
        &root_csrf,
        "navigation_limited",
        &["user.view.all"],
    )
    .await;
    replace_member_roles(
        &app,
        &root_cookie,
        &root_csrf,
        &member_id,
        &["navigation_limited"],
    )
    .await;
    let (member_cookie, _) = login_and_capture_cookie(&app, "navigation-member", "temp-pass").await;

    let (status, payload) = get_console_navigation(&app, &member_cookie).await;

    assert_eq!(status, StatusCode::OK);
    let item_ids = string_values(&payload, &["data", "navigation_items"], "item_id");
    assert!(!item_ids.contains(&"settings".to_string()));
    assert!(!item_ids.contains(&"settings.api-key-authentication".to_string()));
    assert!(!item_ids.contains(&"settings.auth-center".to_string()));
    assert!(!item_ids.contains(&"settings.members".to_string()));
    assert!(!item_ids.contains(&"settings.docs".to_string()));
    assert!(!item_ids.contains(&"settings.roles".to_string()));
    assert!(!item_ids.contains(&"templates".to_string()));

    let route_ids = string_values(&payload, &["data", "route_definitions"], "route_id");
    assert!(!route_ids.contains(&"settings.docs".to_string()));
    assert!(!route_ids.contains(&"settings.roles".to_string()));
    assert!(!route_ids.contains(&"templates".to_string()));

    let binding_route_ids = string_values(&payload, &["data", "permission_bindings"], "route_id");
    assert!(!binding_route_ids.contains(&"settings.docs".to_string()));
    assert!(!binding_route_ids.contains(&"settings.roles".to_string()));
    assert!(!binding_route_ids.contains(&"templates".to_string()));
}

#[tokio::test]
async fn console_navigation_route_includes_registered_host_extension_surfaces() {
    let (state, _) = test_api_state_with_database_url().await;
    state
        .console_surface_registry
        .register_host_extension_manifest(
            r#"
schema_version: 1flowbase.host-extension/v1
extension_id: file-security
version: 0.1.0
bootstrap_phase: boot
native:
  abi_version: 1flowbase.host.native/v1
  library: builtin://file-security
  entry_symbol: oneflowbase_host_extension_entry_v1
owned_resources: []
extends_resources: []
infrastructure_providers: []
routes: []
console_surfaces:
  route_definitions:
    - route_id: file-security.settings
      surface_key: file-security.settings
      path: /settings/file-security
      surface_kind: host_extension
  navigation_items:
    - item_id: file-security.settings
      route_id: file-security.settings
      parent_item_id: settings
      label_key: auto.api_documentation
      navigation_slot: settings
      order: 1300
  permission_bindings:
    - binding_id: file-security.settings.view
      route_id: file-security.settings
      permission_codes:
        - plugin_config.view.all
      requirement: any_permission
workers: []
migrations: []
"#,
        )
        .unwrap();
    let app = crate::app_with_state_and_config(state, &test_config());
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let member_id = create_member(
        &app,
        &root_cookie,
        &root_csrf,
        "navigation-plugin-member",
        "temp-pass",
    )
    .await;
    create_role(&app, &root_cookie, &root_csrf, "navigation_plugin").await;
    replace_role_permissions(
        &app,
        &root_cookie,
        &root_csrf,
        "navigation_plugin",
        &["plugin_config.view.all"],
    )
    .await;
    replace_member_roles(
        &app,
        &root_cookie,
        &root_csrf,
        &member_id,
        &["navigation_plugin"],
    )
    .await;
    let (member_cookie, _) =
        login_and_capture_cookie(&app, "navigation-plugin-member", "temp-pass").await;

    let (status, payload) = get_console_navigation(&app, &member_cookie).await;

    assert_eq!(status, StatusCode::OK);
    let route_ids = string_values(&payload, &["data", "route_definitions"], "route_id");
    assert!(route_ids.contains(&"file-security.settings".to_string()));
    let surface_kinds = string_values(&payload, &["data", "route_definitions"], "surface_kind");
    assert!(surface_kinds.contains(&"host_extension".to_string()));
    let item_ids = string_values(&payload, &["data", "navigation_items"], "item_id");
    assert!(item_ids.contains(&"file-security.settings".to_string()));
    let binding_ids = string_values(&payload, &["data", "permission_bindings"], "binding_id");
    assert!(binding_ids.contains(&"file-security.settings.view".to_string()));
}

#[tokio::test]
async fn console_navigation_route_uses_settings_feature_permissions() {
    let app = test_app().await;
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let member_id = create_member(
        &app,
        &root_cookie,
        &root_csrf,
        "settings-feature-member",
        "temp-pass",
    )
    .await;
    create_role(
        &app,
        &root_cookie,
        &root_csrf,
        "settings_feature_roles_only",
    )
    .await;
    replace_role_permissions(
        &app,
        &root_cookie,
        &root_csrf,
        "settings_feature_roles_only",
        &["settings_feature.access.system.roles"],
    )
    .await;
    replace_member_roles(
        &app,
        &root_cookie,
        &root_csrf,
        &member_id,
        &["settings_feature_roles_only"],
    )
    .await;
    let (member_cookie, _) =
        login_and_capture_cookie(&app, "settings-feature-member", "temp-pass").await;

    let (status, payload) = get_console_navigation(&app, &member_cookie).await;

    assert_eq!(status, StatusCode::OK);
    let item_ids = string_values(&payload, &["data", "navigation_items"], "item_id");
    assert!(item_ids.contains(&"settings".to_string()));
    assert!(item_ids.contains(&"settings.roles".to_string()));
    assert!(!item_ids.contains(&"settings.members".to_string()));
    assert!(!item_ids.contains(&"settings.docs".to_string()));

    let route_ids = string_values(&payload, &["data", "route_definitions"], "route_id");
    assert!(route_ids.contains(&"settings.roles".to_string()));
    assert!(!route_ids.contains(&"settings.members".to_string()));
}

#[tokio::test]
async fn console_navigation_route_requires_session() {
    let app = test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/console/navigation")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}
