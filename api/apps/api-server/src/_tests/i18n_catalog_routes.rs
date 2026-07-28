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
            ("DELETE", "/api/console/settings/i18n/custom-keys"),
            ("DELETE", "/api/console/settings/i18n/overrides"),
            ("GET", "/api/console/settings/i18n/catalog"),
            ("GET", "/api/console/settings/i18n/entries"),
            ("GET", "/api/console/settings/i18n/entries/detail"),
            (
                "GET",
                "/api/console/settings/i18n/modules/{module}/messages",
            ),
            ("GET", "/api/console/settings/i18n/update-check"),
            ("POST", "/api/console/settings/i18n/activate"),
            ("POST", "/api/console/settings/i18n/restore-overrides"),
            ("PUT", "/api/console/settings/i18n/custom-translations"),
            ("PUT", "/api/console/settings/i18n/overrides"),
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
        ("get", "/api/console/settings/i18n/entries"),
        ("get", "/api/console/settings/i18n/entries/detail"),
        ("put", "/api/console/settings/i18n/overrides"),
        ("delete", "/api/console/settings/i18n/overrides"),
        ("put", "/api/console/settings/i18n/custom-translations"),
        ("delete", "/api/console/settings/i18n/custom-keys"),
        ("post", "/api/console/settings/i18n/restore-overrides"),
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
                .uri("/api/console/settings/i18n/entries")
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
                .uri("/api/console/settings/i18n/entries")
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

#[tokio::test]
async fn ac_007_management_list_and_detail_preserve_domain_field_names_and_filters() {
    let (state, _) = test_api_state_with_database_url().await;
    activate_seed(&state).await;
    let app = crate::app_with_state_and_config(state, &test_config());
    let (cookie, _) = login_and_capture_cookie(&app, "root", "change-me").await;

    let list = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/console/settings/i18n/entries?module=%40taichuy%2Fplatform%2Fcommon&locale=zh_Hans&search=Settings&origin=official&offset=0&limit=20")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(list.status(), StatusCode::OK);
    let payload = response_json(list).await;
    assert_eq!(payload["data"]["total"], json!(1));
    assert!(payload["data"]["revision"].as_i64().is_some());
    let entry = &payload["data"]["entries"][0];
    assert_eq!(entry["module"], json!("@taichuy/platform/common"));
    assert_eq!(entry["msgid"], json!("Settings"));
    assert_eq!(entry["locale"], json!("zh_Hans"));
    assert_eq!(entry["official_translation"], json!("设置"));
    assert_eq!(entry["override_translation"], Value::Null);
    assert_eq!(entry["custom_translation"], Value::Null);
    assert_eq!(entry["effective_value"], json!("设置"));
    assert_eq!(entry["origin"], json!("official"));
    assert_eq!(entry["missing"], json!(false));
    assert_eq!(entry["obsolete"], json!(false));
    assert_eq!(entry["revision"], payload["data"]["revision"]);

    let detail = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/console/settings/i18n/entries/detail?module=%40taichuy%2Fplatform%2Fcommon&msgid=Settings&locale=zh_Hans")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(detail.status(), StatusCode::OK);
    let detail = response_json(detail).await;
    assert_eq!(detail["data"], entry.clone());

    let invalid_page = app
        .oneshot(
            Request::builder()
                .uri("/api/console/settings/i18n/entries?limit=0")
                .header("cookie", cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(invalid_page.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        response_json(invalid_page).await["code"],
        json!("i18n_catalog_page_limit")
    );
}

async fn catalog_mutation(
    app: &axum::Router,
    method: &str,
    path: &str,
    cookie: &str,
    csrf: Option<&str>,
    body: Value,
) -> axum::response::Response {
    let mut request = Request::builder()
        .method(method)
        .uri(path)
        .header("cookie", cookie)
        .header("content-type", "application/json");
    if let Some(csrf) = csrf {
        request = request.header("x-csrf-token", csrf);
    }
    app.clone()
        .oneshot(request.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap()
}

#[tokio::test]
async fn ac_008_ac_009_management_mutations_are_csrf_revision_and_action_scoped() {
    let (state, _) = test_api_state_with_database_url().await;
    activate_seed(&state).await;
    let app = crate::app_with_state_and_config(state, &test_config());
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let catalog = app
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
    let initial_revision = response_json(catalog).await["data"]["revision"]
        .as_i64()
        .unwrap();
    let official_override = json!({
        "module": "@taichuy/platform/common",
        "msgid": "Settings",
        "locale": "zh_Hans",
        "translation": "覆盖",
        "expected_revision": initial_revision,
    });

    let missing_csrf = catalog_mutation(
        &app,
        "PUT",
        "/api/console/settings/i18n/overrides",
        &cookie,
        None,
        official_override.clone(),
    )
    .await;
    assert_eq!(missing_csrf.status(), StatusCode::FORBIDDEN);

    let mut stale_body = official_override.clone();
    stale_body["expected_revision"] = json!(initial_revision - 1);
    let stale = catalog_mutation(
        &app,
        "PUT",
        "/api/console/settings/i18n/overrides",
        &cookie,
        Some(&csrf),
        stale_body,
    )
    .await;
    assert_eq!(stale.status(), StatusCode::CONFLICT);
    let stale_error = response_json(stale).await;
    assert_eq!(stale_error["status"], json!(409));
    assert_eq!(stale_error["code"], json!("i18n_catalog_revision"));
    assert!(stale_error["message"].as_str().is_some());

    let upserted = catalog_mutation(
        &app,
        "PUT",
        "/api/console/settings/i18n/overrides",
        &cookie,
        Some(&csrf),
        official_override,
    )
    .await;
    assert_eq!(upserted.status(), StatusCode::OK);
    let upserted = response_json(upserted).await;
    assert_eq!(
        upserted["data"]["entry"]["override_translation"],
        json!("覆盖")
    );
    let override_revision = upserted["data"]["revision"].as_i64().unwrap();

    let restored = catalog_mutation(
        &app,
        "DELETE",
        "/api/console/settings/i18n/overrides",
        &cookie,
        Some(&csrf),
        json!({
            "module": "@taichuy/platform/common",
            "msgid": "Settings",
            "locale": "zh_Hans",
            "expected_revision": override_revision,
        }),
    )
    .await;
    assert_eq!(restored.status(), StatusCode::OK);
    let restored = response_json(restored).await;
    assert_eq!(
        restored["data"]["entry"]["override_translation"],
        Value::Null
    );
    let restored_revision = restored["data"]["revision"].as_i64().unwrap();

    let overridden_again = catalog_mutation(
        &app,
        "PUT",
        "/api/console/settings/i18n/overrides",
        &cookie,
        Some(&csrf),
        json!({
            "module": "@taichuy/platform/common",
            "msgid": "Settings",
            "locale": "zh_Hans",
            "translation": "再次覆盖",
            "expected_revision": restored_revision,
        }),
    )
    .await;
    let overridden_again = response_json(overridden_again).await;
    let overridden_again_revision = overridden_again["data"]["revision"].as_i64().unwrap();

    let custom = catalog_mutation(
        &app,
        "PUT",
        "/api/console/settings/i18n/custom-translations",
        &cookie,
        Some(&csrf),
        json!({
            "module": "@taichuy/platform/common",
            "msgid": "custom.packet.key",
            "locale": "zh_Hans",
            "translation": "自定义",
            "expected_revision": overridden_again_revision,
        }),
    )
    .await;
    assert_eq!(custom.status(), StatusCode::OK);
    let custom = response_json(custom).await;
    assert_eq!(custom["data"]["entry"]["origin"], json!("custom"));
    let custom_revision = custom["data"]["revision"].as_i64().unwrap();

    let globally_restored = catalog_mutation(
        &app,
        "POST",
        "/api/console/settings/i18n/restore-overrides",
        &cookie,
        Some(&csrf),
        json!({ "expected_revision": custom_revision }),
    )
    .await;
    assert_eq!(globally_restored.status(), StatusCode::OK);
    let global_revision = response_json(globally_restored).await["data"]["revision"]
        .as_i64()
        .unwrap();

    let custom_after_restore = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/console/settings/i18n/entries/detail?module=%40taichuy%2Fplatform%2Fcommon&msgid=custom.packet.key&locale=zh_Hans")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(custom_after_restore.status(), StatusCode::OK);
    assert_eq!(
        response_json(custom_after_restore).await["data"]["custom_translation"],
        json!("自定义")
    );

    let deleted = catalog_mutation(
        &app,
        "DELETE",
        "/api/console/settings/i18n/custom-keys",
        &cookie,
        Some(&csrf),
        json!({
            "module": "@taichuy/platform/common",
            "msgid": "custom.packet.key",
            "expected_revision": global_revision,
        }),
    )
    .await;
    assert_eq!(deleted.status(), StatusCode::OK);
    assert_eq!(
        response_json(deleted).await["data"]["revision"],
        json!(global_revision + 1)
    );

    let deleted_detail = app
        .oneshot(
            Request::builder()
                .uri("/api/console/settings/i18n/entries/detail?module=%40taichuy%2Fplatform%2Fcommon&msgid=custom.packet.key&locale=zh_Hans")
                .header("cookie", cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(deleted_detail.status(), StatusCode::NOT_FOUND);
}
