use crate::_tests::support::evaluate_console_access;
use crate::_tests::support::{
    create_member, create_role, login_and_capture_cookie, replace_member_roles,
    replace_role_legacy_permissions_only, replace_role_permissions, seed_workspace,
    test_api_state_with_database_url, test_config,
};
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use domain::PermissionDefinition;
use serde_json::{json, Value};
use tower::ServiceExt;

const AUTH_CENTER_FEATURE: &str = "settings_feature.access.system.auth-center";
const HOST_INFRASTRUCTURE_FEATURE: &str = "settings_feature.access.system.host-infrastructure";
const MEMORY_OBSERVATION_FEATURE: &str = "settings_feature.access.system.memory-observation";
const APPLICATIONS_FEATURE: &str = "settings_feature.access.system.applications";
const DOCS_FEATURE: &str = "settings_feature.access.system.docs";
const API_KEY_AUTHENTICATION_FEATURE: &str =
    "settings_feature.access.system.api-key-authentication";
const SYSTEM_RUNTIME_FEATURE: &str = "settings_feature.access.system.system-runtime";
const MCP_MANAGEMENT_FEATURE: &str = "settings_feature.access.system.mcp-management";

async fn response_json(response: axum::response::Response) -> Value {
    serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap()
}

async fn create_test_application(
    app: &axum::Router,
    cookie: &str,
    csrf: &str,
    name: &str,
) -> uuid::Uuid {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/applications")
                .header("cookie", cookie)
                .header("x-csrf-token", csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "application_type": "agent_flow",
                        "workflow_trigger_type": null,
                        "name": name,
                        "description": format!("{name} description"),
                        "icon": null,
                        "icon_type": null,
                        "icon_background": null
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    uuid::Uuid::parse_str(
        response_json(response).await["data"]["id"]
            .as_str()
            .unwrap(),
    )
    .unwrap()
}

async fn register_feature_permissions(database_url: &str) {
    let store = storage_durable_postgres::build_main_durable_postgres(database_url)
        .await
        .expect("test database should be available")
        .store;
    let definitions = [
        (AUTH_CENTER_FEATURE, "system.auth-center"),
        (HOST_INFRASTRUCTURE_FEATURE, "system.host-infrastructure"),
        (MEMORY_OBSERVATION_FEATURE, "system.memory-observation"),
        (APPLICATIONS_FEATURE, "system.applications"),
        (DOCS_FEATURE, "system.docs"),
        (
            API_KEY_AUTHENTICATION_FEATURE,
            "system.api-key-authentication",
        ),
        (SYSTEM_RUNTIME_FEATURE, "system.system-runtime"),
        (MCP_MANAGEMENT_FEATURE, "system.mcp-management"),
    ]
    .into_iter()
    .map(|(code, scope)| PermissionDefinition {
        code: code.to_string(),
        resource: "settings_feature".to_string(),
        action: "access".to_string(),
        scope: scope.to_string(),
        name: format!("settings_feature:access:{scope}"),
    })
    .collect::<Vec<_>>();
    store
        .upsert_permission_catalog(&definitions)
        .await
        .expect("settings feature permissions should be seeded");
}

// AC-003/AC-006/AC-011: the final four legacy settings surfaces accept only
// their SettingsFeature grant and no longer depend on settings_route grants.
#[tokio::test]
async fn final_settings_features_authorize_representative_routes() {
    let (state, database_url) = test_api_state_with_database_url().await;
    let app = crate::app_with_state_and_config(state, &test_config());
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    register_feature_permissions(&database_url).await;

    for (username, permission, path) in [
        (
            "docs-feature-actor",
            DOCS_FEATURE,
            "/api/console/docs/catalog",
        ),
        (
            "api-key-feature-actor",
            API_KEY_AUTHENTICATION_FEATURE,
            "/api/console/user-api-keys",
        ),
        (
            "system-runtime-feature-actor",
            SYSTEM_RUNTIME_FEATURE,
            "/api/console/system/runtime-profile",
        ),
        (
            "mcp-management-feature-actor",
            MCP_MANAGEMENT_FEATURE,
            "/api/console/mcp/catalog",
        ),
    ] {
        let (cookie, _) =
            create_feature_actor(&app, &root_cookie, &root_csrf, username, permission).await;
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(path)
                    .header("cookie", cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK, "{path}");
    }
}

async fn create_feature_actor(
    app: &axum::Router,
    root_cookie: &str,
    root_csrf: &str,
    username: &str,
    permission: &str,
) -> (String, String) {
    let role_code = format!("{username}_role");
    create_role(app, root_cookie, root_csrf, &role_code).await;
    replace_role_permissions(app, root_cookie, root_csrf, &role_code, &[permission]).await;
    let actor_id = create_member(app, root_cookie, root_csrf, username, "temp-pass").await;
    replace_member_roles(app, root_cookie, root_csrf, &actor_id, &[&role_code]).await;
    login_and_capture_cookie(app, username, "temp-pass").await
}

// AC-003/AC-006: each explicit Settings namespace accepts its feature grant alone,
// including representative state-changing operations, without legacy business actions.
#[tokio::test]
async fn explicit_settings_features_authorize_representative_routes_and_writes() {
    let (state, database_url) = test_api_state_with_database_url().await;
    let cache = state.infrastructure.cache_store();
    cache
        .set_json("application-logs:red-light", json!({ "value": 1 }), None)
        .await
        .unwrap();
    let app = crate::app_with_state_and_config(state, &test_config());
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    register_feature_permissions(&database_url).await;

    let (auth_cookie, auth_csrf) = create_feature_actor(
        &app,
        &root_cookie,
        &root_csrf,
        "auth-center-feature-actor",
        AUTH_CENTER_FEATURE,
    )
    .await;
    let auth_create = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/settings/auth-center/login-entries")
                .header("cookie", &auth_cookie)
                .header("x-csrf-token", &auth_csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "auth_type": "password-local",
                        "title": "Feature-owned password",
                        "description": null,
                        "enabled": false,
                        "sort_order": 50
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(auth_create.status(), StatusCode::CREATED);

    let builtin_delete = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!(
                    "/api/console/settings/auth-center/login-entries/{}",
                    domain::BUILTIN_PASSWORD_LOGIN_ENTRY_ID
                ))
                .header("cookie", &auth_cookie)
                .header("x-csrf-token", &auth_csrf)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(builtin_delete.status(), StatusCode::BAD_REQUEST);

    let (host_cookie, host_csrf) = create_feature_actor(
        &app,
        &root_cookie,
        &root_csrf,
        "host-infrastructure-feature-actor",
        HOST_INFRASTRUCTURE_FEATURE,
    )
    .await;
    let cache_clear = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/settings/host-infrastructure/cache/domains/application-logs/clear")
                .header("cookie", &host_cookie)
                .header("x-csrf-token", &host_csrf)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(cache_clear.status(), StatusCode::OK);

    cache
        .set_json(
            "application-logs:secret-boundary",
            json!({ "secret": "redact-until-explicit-reveal" }),
            None,
        )
        .await
        .unwrap();

    let (memory_cookie, memory_csrf) = create_feature_actor(
        &app,
        &root_cookie,
        &root_csrf,
        "memory-observation-feature-actor",
        MEMORY_OBSERVATION_FEATURE,
    )
    .await;
    let memory_overview = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/console/settings/host-infrastructure/memory")
                .header("cookie", &memory_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(memory_overview.status(), StatusCode::OK);

    let memory_entries = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/console/settings/host-infrastructure/memory/contracts/cache-store/entries?path=application-logs")
                .header("cookie", &memory_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(memory_entries.status(), StatusCode::OK);
    let memory_entries = response_json(memory_entries).await;
    let secret_entry = memory_entries["data"]["entries"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["entry_ref"] == "application-logs:secret-boundary")
        .expect("seeded secret metadata should be listed");
    assert!(secret_entry.as_object().unwrap().get("value").is_none());
    let secret_entry_ref = secret_entry["entry_ref"].as_str().unwrap();

    let invalid_reveal = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/settings/host-infrastructure/memory/contracts/cache-store/entries/reveal")
                .header("cookie", &memory_cookie)
                .header("x-csrf-token", &memory_csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "entry_ref": secret_entry_ref, "reveal_mode": "plaintext" })
                        .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(invalid_reveal.status(), StatusCode::BAD_REQUEST);

    let explicit_reveal = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/settings/host-infrastructure/memory/contracts/cache-store/entries/reveal")
                .header("cookie", &memory_cookie)
                .header("x-csrf-token", &memory_csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "entry_ref": secret_entry_ref, "reveal_mode": "full" }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(explicit_reveal.status(), StatusCode::OK);
    assert_eq!(
        response_json(explicit_reveal).await["data"]["value"],
        json!({ "secret": "redact-until-explicit-reveal" })
    );

    let current_application =
        create_test_application(&app, &root_cookie, &root_csrf, "Current workspace").await;
    let outside_application =
        create_test_application(&app, &root_cookie, &root_csrf, "Outside workspace").await;
    let outside_workspace = seed_workspace(&database_url, "Outside applications").await;
    let store = storage_durable_postgres::build_main_durable_postgres(&database_url)
        .await
        .unwrap()
        .store;
    sqlx::query("update applications set workspace_id = $1 where id = $2")
        .bind(outside_workspace)
        .bind(outside_application)
        .execute(store.pool())
        .await
        .unwrap();

    let (applications_cookie, _) = create_feature_actor(
        &app,
        &root_cookie,
        &root_csrf,
        "applications-feature-actor",
        APPLICATIONS_FEATURE,
    )
    .await;
    let applications = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/console/settings/applications")
                .header("cookie", &applications_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(applications.status(), StatusCode::OK);
    let applications = response_json(applications).await;
    assert_eq!(applications["data"]["total"], 1);
    assert_eq!(
        applications["data"]["items"][0]["id"],
        current_application.to_string()
    );

    let invalid_sort = app
        .oneshot(
            Request::builder()
                .uri("/api/console/settings/applications?sort=unknown:asc")
                .header("cookie", &applications_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(invalid_sort.status(), StatusCode::BAD_REQUEST);
}

// AC-002/AC-003: legacy actions do not substitute for feature grants, and an
// unregistered method + path remains fail closed inside the Settings namespace.
#[tokio::test]
async fn legacy_actions_and_unregistered_explicit_settings_routes_are_forbidden() {
    let (state, _) = test_api_state_with_database_url().await;
    let app = crate::app_with_state_and_config(state, &test_config());
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    create_role(
        &app,
        &root_cookie,
        &root_csrf,
        "legacy_explicit_settings_actions",
    )
    .await;
    replace_role_legacy_permissions_only(
        &app,
        &root_cookie,
        &root_csrf,
        "legacy_explicit_settings_actions",
        &[
            "user.view.all",
            "user.manage.all",
            "plugin_config.view.all",
            "plugin_config.configure.all",
            "application.view.all",
        ],
    )
    .await;
    let actor_id = create_member(
        &app,
        &root_cookie,
        &root_csrf,
        "legacy-explicit-settings-actor",
        "temp-pass",
    )
    .await;
    replace_member_roles(
        &app,
        &root_cookie,
        &root_csrf,
        &actor_id,
        &["legacy_explicit_settings_actions"],
    )
    .await;
    let (actor_cookie, _) =
        login_and_capture_cookie(&app, "legacy-explicit-settings-actor", "temp-pass").await;

    for path in [
        "/api/console/settings/auth-center/overview",
        "/api/console/settings/host-infrastructure/cache",
        "/api/console/settings/host-infrastructure/memory",
        "/api/console/settings/applications",
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(path)
                    .header("cookie", &actor_cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN, "{path}");
    }

    let unregistered = app
        .oneshot(
            Request::builder()
                .uri("/api/console/settings/auth-center/unregistered")
                .header("cookie", &root_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(unregistered.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn platform_feature_requires_both_bootstrap_workspace_and_console_grant() {
    use control_plane_contracts::ports::workspace_governance::{
        GrantWorkspaceMembershipInput, ProvisionWorkspaceInput, ProvisionWorkspaceTenant,
        WorkspaceGovernanceRepository,
    };

    async fn request(
        app: &axum::Router,
        cookie: &str,
        csrf: &str,
        method: &str,
        path: &str,
        body: Value,
    ) -> (StatusCode, Value) {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri(path)
                    .header("cookie", cookie)
                    .header("x-csrf-token", csrf)
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = response.status();
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        (status, serde_json::from_slice(&body).unwrap_or(Value::Null))
    }

    const ROUTE: &str = "/api/console/settings/auth-center/overview";
    const CATALOG: &str = "/api/console/settings/roles/console-policy-catalog";
    const MCP_CATALOG: &str = "/api/console/mcp/interface-capabilities";
    const NAV: &str = "/api/console/navigation";
    const GROUP: &str = "system.auth-center";
    const OP: &str = "auth_center.overview.view";
    let (state, _) = test_api_state_with_database_url().await;
    let store = state.store.clone();
    let app = crate::app_with_state_and_config(state, &test_config());
    let (root, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let user = create_member(&app, &root, &root_csrf, "platform-reader", "temp-pass").await;
    let (member, member_csrf) =
        login_and_capture_cookie(&app, "platform-reader", "temp-pass").await;
    let (status, _) = request(&app, &member, &member_csrf, "GET", ROUTE, Value::Null).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, nav) = request(&app, &member, &member_csrf, "GET", NAV, Value::Null).await;
    assert_eq!(status, StatusCode::OK);
    assert!(!nav.to_string().contains("settings.auth-center"));

    let (status, catalog) = request(&app, &root, &root_csrf, "GET", CATALOG, Value::Null).await;
    assert_eq!(status, StatusCode::OK, "{catalog}");
    assert!(catalog["data"]["groups"]
        .as_array()
        .unwrap()
        .iter()
        .any(|group| group["group_id"] == GROUP));
    let (status, interfaces) =
        request(&app, &root, &root_csrf, "GET", MCP_CATALOG, Value::Null).await;
    assert_eq!(status, StatusCode::OK, "{interfaces}");
    let mcp_id = interfaces["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|interface| interface["path"] == ROUTE && interface["method"] == "GET")
        .expect("bootstrap must expose the documented route")["interface_id"]
        .as_str()
        .unwrap();

    let grant = json!({"groups":[{
        "kind":"settings_feature", "group_id":GROUP, "enabled":true, "strategy":"custom",
        "operations":[{"kind":"simple","operation_id":OP,"enabled":true}]
    }]});
    create_role(&app, &root, &root_csrf, "platform_reader").await;
    let (status, body) = request(
        &app,
        &root,
        &root_csrf,
        "PUT",
        "/api/console/settings/roles/platform_reader/console-policy",
        grant.clone(),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
    replace_member_roles(&app, &root, &root_csrf, &user, &["platform_reader"]).await;
    let (member, member_csrf) =
        login_and_capture_cookie(&app, "platform-reader", "temp-pass").await;
    let (status, body) = request(&app, &member, &member_csrf, "GET", ROUTE, Value::Null).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (_, nav) = request(&app, &member, &member_csrf, "GET", NAV, Value::Null).await;
    assert!(nav.to_string().contains("settings.auth-center"));

    let granted_me =
        evaluate_console_access(&app, &member, &member_csrf, &[OP, AUTH_CENTER_FEATURE]).await;
    assert_eq!(granted_me["allowed"][OP], true);
    let (status, body) = request(
        &app, &root, &root_csrf, "PUT", "/api/console/settings/roles/platform_reader/console-policy",
        json!({"groups":[{"kind":"settings_feature", "group_id":GROUP, "enabled":false, "strategy":"full", "operations":[]}]}),
    ).await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
    let (status, _) = request(&app, &member, &member_csrf, "GET", ROUTE, Value::Null).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let revoked_me =
        evaluate_console_access(&app, &member, &member_csrf, &[OP, AUTH_CENTER_FEATURE]).await;
    assert_eq!(revoked_me["allowed"][OP], false);
    assert_ne!(
        revoked_me["policy_fingerprint"],
        granted_me["policy_fingerprint"]
    );

    let customer = store
        .provision_workspace(&ProvisionWorkspaceInput {
            tenant: ProvisionWorkspaceTenant::New {
                code: "customer-platform-test".into(),
                name: "Customer".into(),
            },
            workspace_name: "Customer".into(),
            logo_url: None,
            introduction: String::new(),
            role_templates: access_control::bootstrap_role_templates(),
            initial_member: None,
            actor_user_id: None,
        })
        .await
        .unwrap()
        .workspace
        .id;
    store
        .grant_or_activate_workspace_membership(&GrantWorkspaceMembershipInput {
            workspace_id: customer,
            user_id: user.parse().unwrap(),
            role_codes: vec!["admin".into()],
            role_templates: access_control::bootstrap_role_templates(),
            actor_user_id: None,
        })
        .await
        .unwrap();
    for (cookie, csrf) in [(root, root_csrf), (member, member_csrf)] {
        let (status, switched) = request(
            &app,
            &cookie,
            &csrf,
            "POST",
            "/api/console/session/actions/switch-workspace",
            json!({"workspace_id":customer}),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{switched}");
        let csrf = switched["data"]["csrf_token"].as_str().unwrap();
        let (status, error) = request(&app, &cookie, csrf, "GET", ROUTE, Value::Null).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{error}");
        assert_eq!(error["code"], "permission_denied");
        let access = evaluate_console_access(&app, &cookie, csrf, &[OP, AUTH_CENTER_FEATURE]).await;
        assert_eq!(
            access["allowed"],
            json!({(OP): false, (AUTH_CENTER_FEATURE): false})
        );
        let (status, nav) = request(&app, &cookie, csrf, "GET", NAV, Value::Null).await;
        assert_eq!(status, StatusCode::OK);
        assert!(!nav.to_string().contains("settings.auth-center"));
        let (status, catalog) = request(&app, &cookie, csrf, "GET", CATALOG, Value::Null).await;
        assert_eq!(status, StatusCode::OK, "{catalog}");
        assert!(!catalog["data"]["groups"]
            .as_array()
            .unwrap()
            .iter()
            .any(|group| group["group_id"] == GROUP));
        let (status, error) = request(
            &app,
            &cookie,
            csrf,
            "PUT",
            "/api/console/settings/roles/admin/console-policy",
            grant.clone(),
        )
        .await;
        assert!(
            matches!(status, StatusCode::BAD_REQUEST | StatusCode::FORBIDDEN),
            "{status}: {error}"
        );
        let (status, interfaces) =
            request(&app, &cookie, csrf, "GET", MCP_CATALOG, Value::Null).await;
        assert_eq!(status, StatusCode::OK, "{interfaces}");
        assert!(!interfaces.to_string().contains(mcp_id));
        assert!(!interfaces.to_string().contains(ROUTE));
        let (status, error) = request(
            &app,
            &cookie,
            csrf,
            "POST",
            "/api/console/mcp/debug/execute",
            json!({
                "interface_id":mcp_id,"mcp_arguments":{},"input_mapping":{},"output_mapping":{}
            }),
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{error}");
    }
}
