use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use serde_json::json;
use tower::ServiceExt;

use crate::_tests::support::{
    create_member, create_role, login_and_capture_cookie, replace_member_roles, test_app,
};

async fn list_templates(app: &axum::Router, cookie: &str) -> StatusCode {
    app.clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/console/settings/ui-management/templates")
                .header("cookie", cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap()
        .status()
}

#[tokio::test]
async fn wp_d2_component_record_routes_expose_custom_crud_at_the_stable_namespace() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let create = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/settings/ui-management/components")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "component_code": "local.status-panel",
                        "name": "Status panel",
                        "description": "Shows system status",
                        "import_code": "opaque import {{{",
                        "source_code": "opaque source }}}",
                        "source": "local",
                        "group": "operations",
                        "upstream": { "identity": "@local/status-panel", "version": "0.1.0" },
                        "version": "1.0.0",
                        "keywords": ["status"]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create.status(), StatusCode::CREATED);
    let payload: serde_json::Value =
        serde_json::from_slice(&to_bytes(create.into_body(), usize::MAX).await.unwrap()).unwrap();
    let id = payload["data"]["id"].as_str().unwrap();
    assert_eq!(payload["data"]["origin"], "custom");
    assert_eq!(
        payload["data"]["scope_id"],
        "00000000-0000-0000-0000-000000000000"
    );

    let get = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/settings/ui-management/components/{id}"
                ))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(get.status(), StatusCode::OK);

    let update = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!(
                    "/api/console/settings/ui-management/components/{id}"
                ))
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "System status panel",
                        "description": "Shows system status",
                        "import_code": "opaque import {{{",
                        "source_code": "opaque source }}}",
                        "source": "local",
                        "group": "operations",
                        "upstream": { "identity": "@local/status-panel", "version": "0.2.0" },
                        "version": "1.1.0",
                        "keywords": ["status"]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(update.status(), StatusCode::OK);

    let list = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/console/settings/ui-management/components")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(list.status(), StatusCode::OK);
    let list_payload: serde_json::Value =
        serde_json::from_slice(&to_bytes(list.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(
        list_payload["data"][0]["component_code"],
        "local.status-panel"
    );

    let delete = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!(
                    "/api/console/settings/ui-management/components/{id}"
                ))
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(delete.status(), StatusCode::NO_CONTENT);

    let missing = app
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/settings/ui-management/components/{id}"
                ))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(missing.status(), StatusCode::NOT_FOUND);
}

async fn grant_template_list_operation(app: &axum::Router, cookie: &str, csrf: &str) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/console/settings/roles/ui_management_limited/console-policy")
                .header("cookie", cookie)
                .header("x-csrf-token", csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "groups": [{
                            "kind": "settings_feature",
                            "group_id": "system.ui-management",
                            "enabled": true,
                            "strategy": "custom",
                            "operations": [{
                                "kind": "simple",
                                "operation_id": "ui_management.templates.list",
                                "enabled": true
                            }]
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
async fn ac_001_ui_management_api_requires_its_list_operation_grant() {
    let app = test_app().await;
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let member_id = create_member(
        &app,
        &root_cookie,
        &root_csrf,
        "ui-management-member",
        "temp-pass",
    )
    .await;
    create_role(&app, &root_cookie, &root_csrf, "ui_management_limited").await;
    replace_member_roles(
        &app,
        &root_cookie,
        &root_csrf,
        &member_id,
        &["ui_management_limited"],
    )
    .await;
    let (member_cookie, _) =
        login_and_capture_cookie(&app, "ui-management-member", "temp-pass").await;

    assert_eq!(
        list_templates(&app, &member_cookie).await,
        StatusCode::FORBIDDEN
    );

    grant_template_list_operation(&app, &root_cookie, &root_csrf).await;
    assert_eq!(list_templates(&app, &member_cookie).await, StatusCode::OK);
}

#[tokio::test]
async fn template_creation_uses_backend_code_block_default() {
    use crate::_tests::support::{test_app_with_database_url, test_config};
    use uuid::Uuid;
    let (app, database_url) = test_app_with_database_url().await;
    let pool = sqlx::PgPool::connect(&database_url).await.unwrap();
    let actor_id: Uuid = sqlx::query_scalar("select id from users where account = 'root'")
        .fetch_one(&pool)
        .await
        .unwrap();
    let installation_id = Uuid::now_v7();
    sqlx::query("insert into extension_installations (id, category, organization, artifact_id, artifact_version, plugin_id, contract_version, protocol, display_name, source_kind, trust_level, verification_status, desired_state, signature_status, metadata_json, created_by) values ($1, 'capability-plugins', 'test', 'template-default', '1.0.0', 'template-default@1.0.0', '1flowbase.capability/v1', 'stdio_json', 'Template default test', 'uploaded', 'checksum_only', 'valid', 'active_requested', 'missing', '{}', $2)")
        .bind(installation_id).bind(actor_id).execute(&pool).await.unwrap();
    sqlx::query("insert into extension_artifact_instances (node_id, installation_id, local_version, local_path, artifact_status, runtime_status, availability_status) values ($1, $2, '1.0.0', '/tmp/template-default', 'ready', 'inactive', 'available')")
        .bind(test_config().api_node_id).bind(installation_id).execute(&pool).await.unwrap();
    for (provider, contribution, title) in [
        ("other", "other-block", "AAA other block"),
        ("1flowbase", "frontstage.js-ui-block", "Code block"),
    ] {
        sqlx::query("insert into frontend_block_catalog (id, installation_id, provider_code, plugin_id, plugin_version, contribution_code, title, runtime, entry, context_contract, permission_network, permission_storage, permission_secrets, ui_capabilities, code_template, code_template_version, code_template_language, code_modules) values ($1, $2, $3, 'template-default@1.0.0', '1.0.0', $4, $5, 'native_react', 'index.js', '{\"primitives\":[],\"input_schema\":{}}', 'none', 'none', 'none', '[]', 'export default function Demo() { return null; }', '1.0.0', 'tsx', '[]')")
            .bind(Uuid::now_v7()).bind(installation_id).bind(provider).bind(contribution).bind(title).execute(&pool).await.unwrap();
    }
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/console/settings/ui-management/templates")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let list: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(list["data"]["official"].as_array().unwrap().len(), 2);
    assert_eq!(
        list["data"]["default_template"]["provider_code"],
        "1flowbase"
    );
    assert_eq!(
        list["data"]["default_template"]["contribution_code"],
        "frontstage.js-ui-block"
    );
    for explicit in [false, true] {
        let mut body = json!({"name":"Template", "source":"export default function Demo() { return null; }", "language":"tsx"});
        if explicit {
            body["provider_code"] = json!("other");
            body["contribution_code"] = json!("other-block");
        }
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/console/settings/ui-management/templates")
                    .header("cookie", &cookie)
                    .header("x-csrf-token", &csrf)
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = response.status();
        let created: serde_json::Value =
            serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap())
                .unwrap();
        assert_eq!(status, StatusCode::CREATED, "{created}");
        assert_eq!(
            created["data"]["provider_code"],
            if explicit { "other" } else { "1flowbase" }
        );
        assert_eq!(
            created["data"]["contribution_code"],
            if explicit {
                "other-block"
            } else {
                "frontstage.js-ui-block"
            }
        );
        let id = created["data"]["id"].as_str().unwrap();
        let stored: (String, String) = sqlx::query_as(
            "select provider_code, contribution_code from ui_code_templates where id = $1",
        )
        .bind(Uuid::parse_str(id).unwrap())
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(stored.0, created["data"]["provider_code"].as_str().unwrap());
        assert_eq!(
            stored.1,
            created["data"]["contribution_code"].as_str().unwrap()
        );
    }
    pool.close().await;
}

#[tokio::test]
async fn root_2014_ac_014_old_process_cannot_serve_new_target_template() {
    use crate::_tests::host_extensions::host_extension_boot_tests::NativeSettingsFixture;
    use control_plane::ports::PluginRepository;
    let f = NativeSettingsFixture::new().await;
    let first = f.select("1.0.0").await;
    f.state
        .store
        .apply_native_plugin_settings_templates(&first)
        .await
        .unwrap();
    let old = crate::app_with_state(f.process_state("1.0.0", first));
    let (cookie, csrf) = login_and_capture_cookie(&old, "root", "change-me").await;
    let second = f.select("2.0.0").await;
    f.state
        .store
        .apply_native_plugin_settings_templates(&second)
        .await
        .unwrap();
    let new = crate::app_with_state(f.process_state("2.0.0", second));
    let uri = "/api/console/settings/ui-management/plugin-settings-page?route_id=northwind.settings-page.settings";
    let response = old
        .oneshot(
            Request::builder()
                .uri(uri)
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CONFLICT);
    let response = new
        .clone()
        .oneshot(
            Request::builder()
                .uri(uri)
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 1024 * 1024).await.unwrap())
            .unwrap();
    assert_eq!(body["data"]["applied_plugin_version"], "2.0.0");
    assert_eq!(
        body["data"]["feature_id"],
        "northwind.settings-page.settings"
    );
    assert!(body["data"]["source"]
        .as_str()
        .is_some_and(|s| !s.is_empty()));
    let member_id = create_member(&new, &cookie, &csrf, "native-page-denied", "temp-pass").await;
    create_role(&new, &cookie, &csrf, "native_page_denied").await;
    replace_member_roles(&new, &cookie, &csrf, &member_id, &["native_page_denied"]).await;
    let (member_cookie, _) =
        login_and_capture_cookie(&new, "native-page-denied", "temp-pass").await;
    let forbidden = new
        .clone()
        .oneshot(
            Request::builder()
                .uri(uri)
                .header("cookie", &member_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(forbidden.status(), StatusCode::FORBIDDEN);
    let denied = new
        .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(denied.status(), StatusCode::UNAUTHORIZED);
}

fn native_probe_route_assembly() -> crate::routes::console_route_assembly::ConsoleRouteAssembly<
    std::sync::Arc<crate::app_state::ApiState>,
> {
    use crate::routes::console_route_assembly::{console_get, ConsoleRouteAssembly};
    ConsoleRouteAssembly::new().route(
        "/northwind.settings-page/probe",
        console_get(
            native_probe,
            access_control::ConsoleRouteOwnership::ConsoleOperation(
                "northwind.settings-page.probe".into(),
            ),
        ),
    )
}
async fn native_probe() -> StatusCode {
    StatusCode::NO_CONTENT
}

#[tokio::test]
async fn root_2014_ac_014_old_process_plugin_operation_version_gate() {
    use crate::_tests::host_extensions::host_extension_boot_tests::NativeSettingsFixture;
    use control_plane::ports::PluginRepository;
    use std::sync::Arc;
    let f = NativeSettingsFixture::new().await;
    let first = f.select("1.0.0").await;
    f.state
        .store
        .apply_native_plugin_settings_templates(&first)
        .await
        .unwrap();
    let login_app = crate::app_with_state(f.process_state("1.0.0", first.clone()));
    let (cookie, _) = login_and_capture_cookie(&login_app, "root", "change-me").await;
    let second = f.select("2.0.0").await;
    f.state
        .store
        .apply_native_plugin_settings_templates(&second)
        .await
        .unwrap();
    for (version, target, expected) in [
        ("1.0.0", first, StatusCode::CONFLICT),
        ("2.0.0", second, StatusCode::NO_CONTENT),
    ] {
        // A separate API-bearing host fixture binds a real handler; the page-only package
        // remains free of fake API declarations in the default loader tests.
        let mut contribution = f.contribution(version);
        let api = plugin_framework::parse_host_extension_contribution_manifest(&format!(r#"
schema_version: 1flowbase.host-extension/v1
extension_id: northwind.settings-page
version: {version}
bootstrap_phase: boot
native:
  abi_version: 1flowbase.host.native/v1
  library: builtin://northwind.settings-page
  entry_symbol: oneflowbase_host_extension_entry_v1
settings_features:
  - feature_id: northwind.settings-page.settings
    owner: {{kind: host_extension, owner_id: northwind.settings-page, version: {version}}}
    lifecycle: active
    console_surface:
      route_id: northwind.settings-page.settings
      surface_key: northwind.settings-page.settings
      path: /settings/northwind.settings-page
      label_key: northwind.settings-page.console.settings.label
      description_key: northwind.settings-page.console.settings.description
      order: 100
    api_routes: [{{method: GET, path: /api/console/northwind.settings-page/probe}}]
console_operations:
  - operation_id: northwind.settings-page.probe
    owner: {{kind: host_extension, owner_id: northwind.settings-page, version: {version}}}
    lifecycle: active
    policy_group: !settings_feature northwind.settings-page.settings
    order: 100
    routes: [{{method: GET, path: /api/console/northwind.settings-page/probe}}]
    authorization: {{kind: simple}}
console_locale_catalog:
  texts:
    - {{reference: northwind.settings-page.console.settings.label, en_us: Settings, zh_hans: 设置}}
    - {{reference: northwind.settings-page.console.settings.description, en_us: Settings, zh_hans: 设置}}
  policy_groups: []
"#)).unwrap();
        contribution.settings_features = api.settings_features;
        contribution.console_operations = api.console_operations;
        let source = crate::host_extensions::console::LinkedHostConsoleRouteSource {
            extension_id: "northwind.settings-page",
            version: if version == "1.0.0" { "1.0.0" } else { "2.0.0" },
            route_assembly: native_probe_route_assembly,
        };
        let resolved =
            crate::host_extensions::console::resolve_linked_host_extension_console_contribution(
                contribution.clone(),
                &[source],
            )
            .unwrap();
        let plan = crate::app_state::compile_console_boot_plan([resolved]).unwrap();
        let state = Arc::new(crate::app_state::ApiState {
            settings_feature_registry: plan.settings_feature_registry,
            console_operation_registry: plan.console_operation_registry,
            console_surface_registry: Arc::new(
                plan.console_surface_registry
                    .as_ref()
                    .clone()
                    .with_native_targets(vec![target]),
            ),
            ..(*f.state).clone()
        });
        let app = axum::Router::new().route("/api/console/northwind.settings-page/probe",axum::routing::get(native_probe))
            .layer(axum::middleware::from_fn_with_state(state,crate::middleware::require_settings_feature_permission::require_settings_feature_permission));
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/console/northwind.settings-page/probe")
                    .header("cookie", &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), expected);
    }
}
