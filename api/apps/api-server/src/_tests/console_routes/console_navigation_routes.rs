use crate::_tests::support::{
    create_member, create_role, login_and_capture_cookie, replace_member_roles,
    replace_role_legacy_permissions_only, replace_role_permissions,
    test_api_state_with_database_url, test_app, test_config,
};
use std::sync::Arc;

use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use plugin_framework::parse_host_extension_contribution_manifest;
use serde_json::{json, Value};
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
    assert!(!route_ids.contains(&"home".to_string()));
    assert!(route_ids.contains(&"settings.roles".to_string()));

    let item_ids = string_values(&payload, &["data", "navigation_items"], "item_id");
    assert!(item_ids.contains(&"settings".to_string()));
    assert!(item_ids.contains(&"settings.api-key-authentication".to_string()));
    assert!(item_ids.contains(&"settings.applications".to_string()));

    let mut settings_items = payload["data"]["navigation_items"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|item| item["parent_item_id"] == "settings")
        .map(|item| {
            (
                item["order"].as_i64().unwrap(),
                item["item_id"].as_str().unwrap(),
            )
        })
        .collect::<Vec<_>>();
    settings_items.sort_by_key(|(order, _)| *order);
    assert_eq!(
        settings_items
            .into_iter()
            .map(|(_, item_id)| item_id)
            .collect::<Vec<_>>(),
        vec![
            "settings.extension-center",
            "settings.model-providers",
            "settings.network-center",
            "settings.applications",
            "settings.mcp-management",
            "settings.members",
            "settings.communications",
            "settings.ui-management",
            "settings.backups",
            "settings.roles",
            "settings.docs",
            "settings.api-key-authentication",
            "settings.data-models",
            "settings.auth-center",
            "settings.files",
            "settings.system-runtime",
            "settings.memory-observation",
            "settings.i18n",
            "settings.system",
        ]
    );
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
    assert!(!item_ids.contains(&"home".to_string()));
    assert!(!item_ids.contains(&"templates".to_string()));
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
    assert!(!route_ids.contains(&"home".to_string()));

    let binding_route_ids = string_values(&payload, &["data", "permission_bindings"], "route_id");
    assert!(!binding_route_ids.contains(&"settings.docs".to_string()));
    assert!(!binding_route_ids.contains(&"settings.roles".to_string()));
    assert!(!binding_route_ids.contains(&"templates".to_string()));
    assert!(!binding_route_ids.contains(&"home".to_string()));
}

#[tokio::test]
async fn console_navigation_route_includes_registered_host_extension_surfaces() {
    let (base_state, _) = test_api_state_with_database_url().await;
    let contribution = parse_host_extension_contribution_manifest(
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
    let state = Arc::new(crate::app_state::ApiState {
        console_surface_registry: Arc::new(
            crate::console_surface_registry::ConsoleSurfaceRegistry::from_host_extension_contributions(
                [&contribution],
            )
            .unwrap(),
        ),
        ..(*base_state).clone()
    });
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

async fn replace_navigation_policy(
    app: &axum::Router,
    cookie: &str,
    csrf: &str,
    role: &str,
    group: Value,
) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/console/settings/roles/{role}/console-policy"))
                .header("cookie", cookie)
                .header("x-csrf-token", csrf)
                .header("content-type", "application/json")
                .body(Body::from(json!({"groups": [group]}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
}

async fn navigation_me(app: &axum::Router, cookie: &str) -> Value {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/console/me")
                .header("cookie", cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    serde_json::from_slice(&body).unwrap()
}

#[tokio::test]
async fn mcp_role_permission_navigation_uses_canonical_readonly_policy_not_stale_legacy() {
    let (state, _) = test_api_state_with_database_url().await;
    let operations = state
        .console_operation_registry
        .inventory()
        .operations
        .iter()
        .filter(|operation| {
            operation.policy_group
                == access_control::ConsolePolicyGroup::SettingsFeature("system.roles".into())
                && operation.lifecycle == access_control::SettingsFeatureLifecycle::Active
                && operation.routes.iter().any(|route| route.method == "GET")
        })
        .map(|operation| {
            assert_eq!(
                operation.authorization,
                access_control::ConsoleAuthorization::Simple
            );
            json!({"kind": "simple", "operation_id": operation.operation_id, "enabled": true})
        })
        .collect::<Vec<_>>();
    assert!(!operations.is_empty());
    let app = crate::app_with_state_and_config(state, &test_config());
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let role = "navigation_canonical_reader";
    create_role(&app, &root_cookie, &root_csrf, role).await;
    let mut group = json!({
        "kind": "settings_feature", "group_id": "system.roles", "enabled": true,
        "strategy": "custom", "operations": operations
    });
    replace_navigation_policy(&app, &root_cookie, &root_csrf, role, group.clone()).await;
    let legacy = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/console/settings/roles/{role}/permissions"))
                .header("cookie", &root_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(legacy.status(), StatusCode::OK);
    let legacy_body = to_bytes(legacy.into_body(), usize::MAX).await.unwrap();
    let legacy: Value = serde_json::from_slice(&legacy_body).unwrap();
    assert_eq!(legacy["data"]["permission_codes"], json!([]));
    let member_id = create_member(
        &app,
        &root_cookie,
        &root_csrf,
        "navigation-canonical-reader",
        "temp-pass",
    )
    .await;
    replace_member_roles(&app, &root_cookie, &root_csrf, &member_id, &[role]).await;
    let (cookie, csrf) =
        login_and_capture_cookie(&app, "navigation-canonical-reader", "temp-pass").await;

    // No legacy feature or manage grants: canonical GET operations alone open the read-only page.
    let (status, payload) = get_console_navigation(&app, &cookie).await;
    assert_eq!(status, StatusCode::OK);
    for (path, key) in [
        ("navigation_items", "item_id"),
        ("route_definitions", "route_id"),
        ("permission_bindings", "route_id"),
    ] {
        assert!(string_values(&payload, &["data", path], key).contains(&"settings.roles".into()));
    }
    let me = navigation_me(&app, &cookie).await;
    let permissions = me["data"]["permissions"].as_array().unwrap();
    assert!(permissions.contains(&json!("settings_feature.access.system.roles")));
    assert!(!permissions.contains(&json!("role_permission.manage.all")));
    let read = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/console/settings/roles")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(read.status(), StatusCode::OK);

    for (method, uri, body) in [
        (
            "POST",
            "/api/console/settings/roles".to_string(),
            json!({
                "code": "navigation_escalated", "name": "must not create", "introduction": "denied"
            }),
        ),
        (
            "PUT",
            format!("/api/console/settings/roles/{role}/console-policy"),
            json!({
                "groups": [{"kind": "settings_feature", "group_id": "system.roles",
                    "enabled": true, "strategy": "full", "operations": []}]
            }),
        ),
        (
            "DELETE",
            format!("/api/console/settings/roles/{role}"),
            Value::Null,
        ),
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri(uri)
                    .header("cookie", &cookie)
                    .header("x-csrf-token", &csrf)
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN, "{method}");
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let error: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(error["code"], "console_operation_permission_denied");
    }

    // Seed only the stale legacy table through its existing test owner, not the policy projector.
    replace_role_legacy_permissions_only(
        &app,
        &root_cookie,
        &root_csrf,
        role,
        &[
            "settings_feature.access.system.roles",
            "settings_feature.access.system.docs",
        ],
    )
    .await;
    group["enabled"] = json!(false);
    replace_navigation_policy(&app, &root_cookie, &root_csrf, role, group.clone()).await;
    let (status, payload) = get_console_navigation(&app, &cookie).await;
    assert_eq!(status, StatusCode::OK);
    for (path, key) in [
        ("navigation_items", "item_id"),
        ("route_definitions", "route_id"),
        ("permission_bindings", "route_id"),
    ] {
        let ids = string_values(&payload, &["data", path], key);
        assert!(!ids.contains(&"settings.roles".into()));
        assert!(!ids.contains(&"settings.docs".into()));
    }
    let me = navigation_me(&app, &cookie).await;
    assert!(me["data"]["permissions"]
        .as_array()
        .unwrap()
        .iter()
        .all(|permission| {
            !permission
                .as_str()
                .unwrap()
                .starts_with("settings_feature.access.")
        }));
    let denied = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/console/settings/roles")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(denied.status(), StatusCode::FORBIDDEN);

    // The same session observes re-enabled canonical policy without a stale navigation cache.
    group["enabled"] = json!(true);
    replace_navigation_policy(&app, &root_cookie, &root_csrf, role, group).await;
    let (status, payload) = get_console_navigation(&app, &cookie).await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        string_values(&payload, &["data", "navigation_items"], "item_id")
            .contains(&"settings.roles".into())
    );
}

fn navigation_operation_inventory(row: bool) -> access_control::ConsoleOperationCompiledInventory {
    let mut inventory = crate::app_state::compile_core_console_operation_inventory_snapshot()
        .unwrap()
        .compiled_inventory;
    let operation = inventory
        .operations
        .iter()
        .find(|operation| {
            matches!(
                operation.policy_group,
                access_control::ConsolePolicyGroup::SettingsFeature(_)
            ) && operation.lifecycle == access_control::SettingsFeatureLifecycle::Active
                && if row {
                    matches!(
                        operation.authorization,
                        access_control::ConsoleAuthorization::ResourceAction { .. }
                    )
                } else {
                    operation.operation_id == "roles.console_policy.view"
                }
        })
        .expect("compiled settings operation fixture")
        .clone();
    inventory.operations = vec![operation];
    inventory
}

#[test]
fn mcp_role_permission_navigation_simple_full_custom_none_and_inactive_invariants() {
    use crate::middleware::require_session::settings_feature_navigation_permissions;
    use domain::{
        ConsoleOperationId, ConsoleOperationPolicy, ConsolePolicyGroup, RoleConsoleGroupPolicy,
        RoleConsolePolicy,
    };
    let mut inventory = navigation_operation_inventory(false);
    let group = ConsolePolicyGroup::settings_feature("system.roles").unwrap();
    let id = ConsoleOperationId::try_from("roles.console_policy.view").unwrap();
    let enabled = ConsoleOperationPolicy::simple(id.clone(), true);
    let custom = RoleConsoleGroupPolicy::custom(group.clone(), vec![enabled]);
    for (policy, expected) in [
        (RoleConsoleGroupPolicy::full(group.clone()), true),
        (custom.clone(), true),
        (custom.with_enabled(false), false),
        (RoleConsoleGroupPolicy::disabled(group.clone()), false),
        (RoleConsoleGroupPolicy::custom(group.clone(), vec![]), false),
        (
            RoleConsoleGroupPolicy::custom(
                group.clone(),
                vec![ConsoleOperationPolicy::simple(id.clone(), false)],
            ),
            false,
        ),
        (
            RoleConsoleGroupPolicy::custom(
                group.clone(),
                vec![ConsoleOperationPolicy::row(
                    id.clone(),
                    domain::ConsoleOperationRowScope::Own,
                )],
            ),
            false,
        ),
        (
            RoleConsoleGroupPolicy::custom(
                group.clone(),
                vec![ConsoleOperationPolicy::simple(
                    ConsoleOperationId::try_from("roles.create").unwrap(),
                    true,
                )],
            ),
            false,
        ),
    ] {
        let permissions = settings_feature_navigation_permissions(
            &inventory,
            &[RoleConsolePolicy::new(
                uuid::Uuid::nil(),
                vec![policy.clone()],
            )],
        );
        assert_eq!(
            permissions.contains("settings_feature.access.system.roles"),
            expected,
            "{policy:?}"
        );
        assert_eq!(permissions.len(), usize::from(expected));
    }
    assert!(settings_feature_navigation_permissions(&inventory, &[]).is_empty());
    let policies = [RoleConsolePolicy::new(
        uuid::Uuid::nil(),
        vec![RoleConsoleGroupPolicy::full(group)],
    )];
    inventory.operations[0].lifecycle = access_control::SettingsFeatureLifecycle::Inactive;
    assert!(settings_feature_navigation_permissions(&inventory, &policies).is_empty());
    inventory.operations[0].lifecycle = access_control::SettingsFeatureLifecycle::Active;
    inventory.operations[0].authorization = access_control::ConsoleAuthorization::Authenticated;
    assert!(settings_feature_navigation_permissions(&inventory, &policies).is_empty());
    inventory.operations[0].authorization = access_control::ConsoleAuthorization::Simple;
    inventory.operations[0].policy_group =
        access_control::ConsolePolicyGroup::Other("other.test".into());
    let policies = [RoleConsolePolicy::new(
        uuid::Uuid::nil(),
        vec![RoleConsoleGroupPolicy::full(
            ConsolePolicyGroup::other("other.test").unwrap(),
        )],
    )];
    assert!(settings_feature_navigation_permissions(&inventory, &policies).is_empty());
}

#[test]
fn mcp_role_permission_navigation_row_full_custom_none_and_inactive_invariants() {
    use crate::middleware::require_session::settings_feature_navigation_permissions;
    use domain::{
        ConsoleOperationId, ConsoleOperationPolicy, ConsoleOperationRowScope, ConsolePolicyGroup,
        RoleConsoleGroupPolicy, RoleConsolePolicy,
    };
    let mut inventory = navigation_operation_inventory(true);
    let access_control::ConsolePolicyGroup::SettingsFeature(feature) =
        &inventory.operations[0].policy_group
    else {
        unreachable!()
    };
    let permission = format!("settings_feature.access.{feature}");
    let group = ConsolePolicyGroup::settings_feature(feature).unwrap();
    let id = ConsoleOperationId::try_from(inventory.operations[0].operation_id.as_str()).unwrap();
    for (scope, expected) in [
        (ConsoleOperationRowScope::Own, true),
        (ConsoleOperationRowScope::ScopeAll, true),
        (ConsoleOperationRowScope::Disabled, false),
    ] {
        let policy = RoleConsoleGroupPolicy::custom(
            group.clone(),
            vec![ConsoleOperationPolicy::row(id.clone(), scope)],
        );
        let permissions = settings_feature_navigation_permissions(
            &inventory,
            &[RoleConsolePolicy::new(uuid::Uuid::nil(), vec![policy])],
        );
        assert_eq!(permissions.contains(&permission), expected, "{scope:?}");
        assert_eq!(permissions.len(), usize::from(expected));
    }
    for (policy, expected) in [
        (RoleConsoleGroupPolicy::full(group.clone()), true),
        (
            RoleConsoleGroupPolicy::full(group.clone()).with_enabled(false),
            false,
        ),
        (RoleConsoleGroupPolicy::custom(group.clone(), vec![]), false),
        (
            RoleConsoleGroupPolicy::custom(
                group.clone(),
                vec![ConsoleOperationPolicy::simple(id, true)],
            ),
            false,
        ),
    ] {
        assert_eq!(
            settings_feature_navigation_permissions(
                &inventory,
                &[RoleConsolePolicy::new(
                    uuid::Uuid::nil(),
                    vec![policy.clone()]
                ),]
            )
            .contains(&permission),
            expected,
            "{policy:?}"
        );
    }
    let policies = [RoleConsolePolicy::new(
        uuid::Uuid::nil(),
        vec![RoleConsoleGroupPolicy::full(group)],
    )];
    inventory.operations[0].lifecycle = access_control::SettingsFeatureLifecycle::Inactive;
    assert!(settings_feature_navigation_permissions(&inventory, &policies).is_empty());
    inventory.operations.clear();
    assert!(settings_feature_navigation_permissions(&inventory, &policies).is_empty());
}
