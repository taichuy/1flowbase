use crate::_tests::support::{
    create_member, create_role, login_and_capture_cookie, replace_member_roles,
    replace_role_permissions, seed_workspace, test_api_state_with_database_url, test_config,
};
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use control_plane::ports::I18nCatalogRepository;
use serde_json::{json, Value};
use tower::ServiceExt;
use utoipa::OpenApi;

async fn response_json(response: axum::response::Response) -> Value {
    serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap()
}

async fn activate_seed(state: &crate::app_state::ApiState) {
    let seed = crate::official_i18n_catalog_seed::load_official_i18n_catalog_seed().unwrap();
    let release = seed
        .bind_to_workspace(state.bootstrap_workspace_id)
        .unwrap();
    I18nCatalogRepository::import_verified_release(&state.store, &release)
        .await
        .unwrap();
    let catalog_state = I18nCatalogRepository::bootstrap_workspace_catalog_state(
        &state.store,
        state.bootstrap_workspace_id,
    )
    .await
    .unwrap();
    I18nCatalogRepository::activate_verified_release(
        &state.store,
        state.bootstrap_workspace_id,
        release.id(),
        catalog_state.revision(),
    )
    .await
    .unwrap();
}

#[test]
fn ac_004_settings_feature_route_assembly_and_openapi_are_exact() {
    let registry = crate::app_state::compile_core_settings_feature_registry().unwrap();
    let feature = registry
        .inventory()
        .features
        .iter()
        .find(|feature| {
            feature.feature_id == access_control::SYSTEM_I18N_CATALOG_SETTINGS_FEATURE_ID
        })
        .unwrap();
    assert_eq!(feature.console_surface.path, "/settings/i18n");
    assert_eq!(
        feature
            .api_routes
            .iter()
            .map(|route| (route.method.as_str(), route.path.as_str()))
            .collect::<Vec<_>>(),
        vec![
            ("GET", "/api/console/settings/i18n/catalog"),
            (
                "GET",
                "/api/console/settings/i18n/modules/{module}/messages"
            ),
            ("GET", "/api/console/settings/i18n/update-check"),
            ("POST", "/api/console/settings/i18n/activate"),
        ]
    );

    let openapi = serde_json::to_value(crate::openapi::ApiDoc::openapi()).unwrap();
    for (method, path) in [
        ("get", "/api/console/settings/i18n/catalog"),
        (
            "get",
            "/api/console/settings/i18n/modules/{module}/messages",
        ),
        ("get", "/api/console/settings/i18n/update-check"),
        ("post", "/api/console/settings/i18n/activate"),
    ] {
        let operation = &openapi["paths"][path][method];
        assert!(operation["operationId"].as_str().is_some());
        assert!(!operation["summary"]
            .as_str()
            .unwrap_or_default()
            .trim()
            .is_empty());
        assert!(!operation["description"]
            .as_str()
            .unwrap_or_default()
            .trim()
            .is_empty());
    }
}

#[tokio::test]
async fn ac_004_root_reads_catalog_state_and_backend_resolved_bundle() {
    let (state, _) = test_api_state_with_database_url().await;
    activate_seed(&state).await;
    let app = crate::app_with_state_and_config(state, &test_config());
    let (cookie, _) = login_and_capture_cookie(&app, "root", "change-me").await;

    let state_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/console/settings/i18n/catalog")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(state_response.status(), StatusCode::OK);
    let state_payload = response_json(state_response).await;
    assert_eq!(
        state_payload["data"]["active_catalog_version"],
        json!("1.0.0")
    );
    assert_eq!(state_payload["data"]["source"], json!("official"));
    assert_eq!(state_payload["data"]["source_locale"], json!("en_US"));
    assert!(state_payload["data"]["locales"]
        .as_array()
        .unwrap()
        .contains(&json!("zh_Hans")));
    assert!(state_payload["data"]["modules"]
        .as_array()
        .unwrap()
        .contains(&json!("@taichuy/platform/common")));

    let bundle_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/console/settings/i18n/modules/%40taichuy%2Fplatform%2Fcommon/messages?locale=fr_FR")
                .header("cookie", cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(bundle_response.status(), StatusCode::OK);
    let bundle = response_json(bundle_response).await;
    let cancel = bundle["data"]["messages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|message| message["msgid"] == "Cancel")
        .unwrap();
    assert_eq!(cancel["value"], json!("Cancel"));
    assert_eq!(cancel["origin"], json!("english_identity"));
}

#[tokio::test]
async fn ac_004_invalid_module_and_locale_are_safe_client_errors() {
    let (state, _) = test_api_state_with_database_url().await;
    let app = crate::app_with_state_and_config(state, &test_config());
    let (cookie, _) = login_and_capture_cookie(&app, "root", "change-me").await;

    for path in [
        "/api/console/settings/i18n/modules/not-a-module/messages?locale=zh_Hans",
        "/api/console/settings/i18n/modules/%40taichuy%2Fplatform%2Fcommon/messages?locale=../../etc",
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(path)
                    .header("cookie", &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST, "{path}");
    }
}

#[tokio::test]
async fn ac_005_activation_requires_csrf_and_maps_stale_revision_to_conflict() {
    let (state, _) = test_api_state_with_database_url().await;
    activate_seed(&state).await;
    let app = crate::app_with_state_and_config(state, &test_config());
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let body = json!({ "expected_revision": 0 }).to_string();

    let missing_csrf = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/settings/i18n/activate")
                .header("cookie", &cookie)
                .header("content-type", "application/json")
                .body(Body::from(body.clone()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(missing_csrf.status(), StatusCode::FORBIDDEN);

    let stale = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/settings/i18n/activate")
                .header("cookie", cookie)
                .header("x-csrf-token", csrf)
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(stale.status(), StatusCode::CONFLICT);
    assert_eq!(
        response_json(stale).await["code"],
        json!("i18n_catalog_revision")
    );
}

#[tokio::test]
async fn ac_006_feature_grant_and_effective_root_in_foreign_workspace_both_fail_closed() {
    let (state, database_url) = test_api_state_with_database_url().await;
    let app = crate::app_with_state_and_config(state, &test_config());
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;

    create_role(&app, &root_cookie, &root_csrf, "i18n_catalog_reader").await;
    replace_role_permissions(
        &app,
        &root_cookie,
        &root_csrf,
        "i18n_catalog_reader",
        &[access_control::SYSTEM_I18N_CATALOG_SETTINGS_FEATURE_PERMISSION],
    )
    .await;
    let member_id = create_member(&app, &root_cookie, &root_csrf, "i18n-reader", "temp-pass").await;
    replace_member_roles(
        &app,
        &root_cookie,
        &root_csrf,
        &member_id,
        &["i18n_catalog_reader"],
    )
    .await;
    let (member_cookie, _) = login_and_capture_cookie(&app, "i18n-reader", "temp-pass").await;
    let member_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/console/settings/i18n/catalog")
                .header("cookie", member_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(member_response.status(), StatusCode::FORBIDDEN);
    assert_eq!(
        response_json(member_response).await["code"],
        json!("root_i18n_catalog_actor")
    );

    let foreign_workspace = seed_workspace(&database_url, "Foreign i18n workspace").await;
    let switched = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/session/actions/switch-workspace")
                .header("cookie", &root_cookie)
                .header("x-csrf-token", root_csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "workspace_id": foreign_workspace }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(switched.status(), StatusCode::OK);
    let foreign_response = app
        .oneshot(
            Request::builder()
                .uri("/api/console/settings/i18n/catalog")
                .header("cookie", root_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(foreign_response.status(), StatusCode::FORBIDDEN);
    assert_eq!(
        response_json(foreign_response).await["code"],
        json!("root_i18n_catalog_actor")
    );
}
