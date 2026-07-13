use crate::_tests::support::{
    create_member, create_role, login_and_capture_cookie, replace_member_roles,
    replace_role_permissions, seed_workspace, test_api_state_with_database_url, test_config,
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
    let store = storage_durable::build_main_durable_postgres(database_url)
        .await
        .expect("test database should be available")
        .store;
    let definitions = [
        (AUTH_CENTER_FEATURE, "system.auth-center"),
        (HOST_INFRASTRUCTURE_FEATURE, "system.host-infrastructure"),
        (MEMORY_OBSERVATION_FEATURE, "system.memory-observation"),
        (APPLICATIONS_FEATURE, "system.applications"),
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
                .uri("/api/console/settings/auth-center/authenticators")
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
                    "/api/console/settings/auth-center/authenticators/{}",
                    domain::PASSWORD_LOCAL_AUTHENTICATOR_ID
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
    let store = storage_durable::build_main_durable_postgres(&database_url)
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
    replace_role_permissions(
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
