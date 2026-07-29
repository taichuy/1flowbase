use crate::_tests::support::{
    create_member, login_and_capture_cookie, test_api_state_with_database_url, test_config,
};
use axum::{
    body::{to_bytes, Body},
    http::{header, Request, StatusCode},
};
use control_plane::{
    i18n_catalog::RuntimeI18nCatalogService,
    ports::{I18nCatalogRepository, UpsertCatalogTranslationInput},
};
use domain::{CatalogLocale, CatalogMessageIdentity, CatalogModuleId, CatalogTranslation};
use serde_json::Value;
use tower::ServiceExt;
use utoipa::OpenApi;

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

async fn body_bytes(response: axum::response::Response) -> Vec<u8> {
    to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap()
        .to_vec()
}

fn manifest_module<'a>(manifest: &'a Value, module: &str) -> &'a Value {
    manifest["modules"]
        .as_array()
        .unwrap()
        .iter()
        .find(|candidate| candidate["module"] == module)
        .unwrap()
}

#[test]
fn ac_011_runtime_routes_are_authenticated_and_documented() {
    let assembly = crate::routes::console_route_assembly::migrated_core_console_route_assembly();
    for path in [
        "/api/console/i18n/manifest",
        "/api/console/i18n/bundles/:digest",
    ] {
        let binding = assembly
            .bindings()
            .iter()
            .find(|binding| binding.route.method == "GET" && binding.route.path == path)
            .unwrap();
        assert_eq!(
            binding.ownership,
            access_control::ConsoleRouteOwnership::Authenticated
        );
    }
    let openapi = serde_json::to_value(crate::openapi::ApiDoc::openapi()).unwrap();
    for path in [
        "/api/console/i18n/manifest",
        "/api/console/i18n/bundles/{digest}",
    ] {
        assert!(!openapi["paths"][path]["get"]["summary"]
            .as_str()
            .unwrap()
            .is_empty());
        assert!(!openapi["paths"][path]["get"]["description"]
            .as_str()
            .unwrap()
            .is_empty());
    }
}

#[tokio::test]
async fn ac_011_manifest_bundle_etag_cache_and_server_resolution_contract() {
    let (state, _) = test_api_state_with_database_url().await;
    activate_seed(&state).await;
    let app = crate::app_with_state_and_config(state.clone(), &test_config());
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    create_member(
        &app,
        &root_cookie,
        &root_csrf,
        "runtime-reader",
        "reader-pass",
    )
    .await;
    let (reader_cookie, _) = login_and_capture_cookie(&app, "runtime-reader", "reader-pass").await;

    let cache_miss_bundle =
        RuntimeI18nCatalogService::new(state.store.clone(), state.bootstrap_workspace_id)
            .current_bundle(
                state.bootstrap_workspace_id,
                &CatalogModuleId::new("@taichuy/platform/common").unwrap(),
                &CatalogLocale::new("de_DE").unwrap(),
            )
            .await
            .unwrap()
            .unwrap();
    let cache_miss_href = format!(
        "/api/console/i18n/bundles/{}?module=%40taichuy%2Fplatform%2Fcommon&locale=de_DE",
        cache_miss_bundle.digest.as_str(),
    );
    let cache_miss_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(cache_miss_href)
                .header("cookie", &reader_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        body_bytes(cache_miss_response).await,
        cache_miss_bundle.bundle.canonical_body().unwrap()
    );

    let unauthenticated = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/console/i18n/manifest?locale=zh_Hans")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(unauthenticated.status(), StatusCode::UNAUTHORIZED);

    let manifest_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/console/i18n/manifest?locale=zh_Hans")
                .header("cookie", &reader_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(manifest_response.status(), StatusCode::OK);
    assert_eq!(
        manifest_response.headers()[header::CACHE_CONTROL],
        "no-cache"
    );
    let manifest_etag = manifest_response.headers()[header::ETAG]
        .to_str()
        .unwrap()
        .to_owned();
    let manifest: Value = serde_json::from_slice(&body_bytes(manifest_response).await).unwrap();
    let module = manifest_module(&manifest, "@taichuy/platform/common");
    let digest = module["digest"].as_str().unwrap().to_owned();
    let href = module["href"].as_str().unwrap().to_owned();

    let not_modified = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/console/i18n/manifest?locale=zh_Hans")
                .header("cookie", &reader_cookie)
                .header(header::IF_NONE_MATCH, &manifest_etag)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(not_modified.status(), StatusCode::NOT_MODIFIED);
    assert!(body_bytes(not_modified).await.is_empty());

    let bundle_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(&href)
                .header("cookie", &reader_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(bundle_response.status(), StatusCode::OK);
    assert_eq!(
        bundle_response.headers()[header::CACHE_CONTROL],
        "public,max-age=31536000,immutable"
    );
    assert_eq!(
        bundle_response.headers()[header::ETAG].to_str().unwrap(),
        format!("\"{digest}\"")
    );
    let old_body = body_bytes(bundle_response).await;
    let old_bundle: Value = serde_json::from_slice(&old_body).unwrap();
    assert_eq!(old_bundle["messages"]["Cancel"], "取消");
    assert_eq!(old_bundle["messages"]["Save {name}"], "保存{name}");
    let bundle_not_modified = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(&href)
                .header("cookie", &reader_cookie)
                .header(header::IF_NONE_MATCH, format!("\"{digest}\""))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(bundle_not_modified.status(), StatusCode::NOT_MODIFIED);
    assert!(body_bytes(bundle_not_modified).await.is_empty());

    let identity = CatalogMessageIdentity::new(
        CatalogModuleId::new("@taichuy/platform/common").unwrap(),
        "Save {name}",
    )
    .unwrap();
    let state_before = I18nCatalogRepository::get_workspace_catalog_state(
        &state.store,
        state.bootstrap_workspace_id,
    )
    .await
    .unwrap()
    .unwrap();
    I18nCatalogRepository::upsert_catalog_override(
        &state.store,
        &UpsertCatalogTranslationInput {
            workspace_id: state.bootstrap_workspace_id,
            value: CatalogTranslation::new(
                identity.clone(),
                CatalogLocale::new("zh_Hans").unwrap(),
                "覆盖",
            )
            .unwrap(),
            expected_revision: state_before.revision(),
        },
    )
    .await
    .unwrap();
    let state_after_override = I18nCatalogRepository::get_workspace_catalog_state(
        &state.store,
        state.bootstrap_workspace_id,
    )
    .await
    .unwrap()
    .unwrap();
    I18nCatalogRepository::upsert_custom_catalog_translation(
        &state.store,
        &UpsertCatalogTranslationInput {
            workspace_id: state.bootstrap_workspace_id,
            value: CatalogTranslation::new(
                CatalogMessageIdentity::new(
                    CatalogModuleId::new("@taichuy/platform/common").unwrap(),
                    "custom.key",
                )
                .unwrap(),
                CatalogLocale::new("zh_Hans").unwrap(),
                "自定义",
            )
            .unwrap(),
            expected_revision: state_after_override.revision(),
        },
    )
    .await
    .unwrap();

    let changed_manifest_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/console/i18n/manifest?locale=zh_Hans")
                .header("cookie", &reader_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let changed_manifest: Value =
        serde_json::from_slice(&body_bytes(changed_manifest_response).await).unwrap();
    let changed_module = manifest_module(&changed_manifest, "@taichuy/platform/common");
    let changed_digest = changed_module["digest"].as_str().unwrap();
    assert_ne!(changed_digest, digest);
    let changed_href = changed_module["href"].as_str().unwrap();
    let changed_bundle_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(changed_href)
                .header("cookie", &reader_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let changed_bundle: Value =
        serde_json::from_slice(&body_bytes(changed_bundle_response).await).unwrap();
    assert_eq!(changed_bundle["messages"]["Save {name}"], "覆盖");
    assert_eq!(changed_bundle["messages"]["custom.key"], "自定义");

    let old_again = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(&href)
                .header("cookie", &reader_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(body_bytes(old_again).await, old_body);

    let stale_uncached = href.replace(
        &digest,
        "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    );
    let stale_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(stale_uncached)
                .header("cookie", &reader_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(stale_response.status(), StatusCode::NOT_FOUND);

    let fallback_manifest_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/console/i18n/manifest?locale=fr_FR")
                .header("cookie", &reader_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let fallback_manifest: Value =
        serde_json::from_slice(&body_bytes(fallback_manifest_response).await).unwrap();
    let fallback_href = manifest_module(&fallback_manifest, "@taichuy/platform/common")["href"]
        .as_str()
        .unwrap();
    let fallback_response = app
        .oneshot(
            Request::builder()
                .uri(fallback_href)
                .header("cookie", &reader_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let fallback: Value = serde_json::from_slice(&body_bytes(fallback_response).await).unwrap();
    assert_eq!(fallback["messages"]["Cancel"], "Cancel");
    assert_eq!(fallback["messages"]["custom.key"], "custom.key");
}

#[tokio::test]
async fn ac_011_runtime_inputs_reject_invalid_locale_module_and_digest() {
    let (state, _) = test_api_state_with_database_url().await;
    let app = crate::app_with_state_and_config(state, &test_config());
    let (cookie, _) = login_and_capture_cookie(&app, "root", "change-me").await;
    for path in [
        "/api/console/i18n/manifest?locale=../../etc",
        "/api/console/i18n/bundles/not-a-digest?module=%40taichuy%2Fplatform%2Fcommon&locale=zh_Hans",
        "/api/console/i18n/bundles/sha256%3Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa?module=bad&locale=zh_Hans",
    ] {
        let response = app.clone().oneshot(
            Request::builder().uri(path).header("cookie", &cookie).body(Body::empty()).unwrap(),
        ).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST, "{path}");
    }
}
