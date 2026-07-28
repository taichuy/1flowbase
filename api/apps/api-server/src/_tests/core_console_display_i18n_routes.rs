use crate::_tests::support::{
    login_and_capture_cookie, test_api_state_with_database_url, test_config,
};
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use control_plane::ports::{I18nCatalogRepository, UpsertCatalogTranslationInput};
use domain::{CatalogLocale, CatalogMessageIdentity, CatalogModuleId, CatalogTranslation};
use tower::ServiceExt;

async fn seed_translation(
    state: &crate::app_state::ApiState,
    module: &str,
    msgid: &str,
    value: &str,
) {
    let workspace_id = state.bootstrap_workspace_id;
    let catalog_state =
        I18nCatalogRepository::bootstrap_workspace_catalog_state(&state.store, workspace_id)
            .await
            .unwrap();
    I18nCatalogRepository::upsert_catalog_override(
        &state.store,
        &UpsertCatalogTranslationInput {
            workspace_id,
            value: CatalogTranslation::new(
                CatalogMessageIdentity::new(CatalogModuleId::new(module).unwrap(), msgid).unwrap(),
                CatalogLocale::new("zh_Hans").unwrap(),
                value,
            )
            .unwrap(),
            expected_revision: catalog_state.revision(),
        },
    )
    .await
    .unwrap();
}

async fn get_json(app: &axum::Router, cookie: &str, path: &str) -> serde_json::Value {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(path)
                .header("cookie", cookie)
                .header("x-1flowbase-locale", "zh_Hans")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap()
}

// AC-006/012/013: real navigation and policy/resource routes use the request-locale resolver,
// preserve English fallback, and leave #1487 operation interface metadata unchanged.
#[tokio::test]
async fn core_console_display_routes_resolve_dynamic_zh_hans_and_fallback_to_english() {
    let (state, _) = test_api_state_with_database_url().await;
    for (module, msgid, value) in [
        (
            "@taichuy/platform/console/settings",
            "Application management",
            "应用管理（动态）",
        ),
        (
            "@taichuy/platform/console/settings",
            "Application management operations",
            "应用管理操作（动态）",
        ),
        (
            "@taichuy/platform/console/settings",
            "Permission management",
            "权限管理（动态）",
        ),
        (
            "@taichuy/platform/console/settings/policy",
            "Full access",
            "完全开放（动态）",
        ),
        (
            "@taichuy/platform/console/settings/resources",
            "Applications",
            "应用（动态）",
        ),
    ] {
        seed_translation(&state, module, msgid, value).await;
    }
    let app = crate::app_with_state_and_config(state, &test_config());
    let (cookie, _) = login_and_capture_cookie(&app, "root", "change-me").await;

    let navigation = get_json(&app, &cookie, "/api/console/navigation").await;
    let application_item = navigation["data"]["navigation_items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["item_id"] == "settings.applications")
        .unwrap();
    assert_eq!(application_item["label"], "应用管理（动态）");
    let docs_item = navigation["data"]["navigation_items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["item_id"] == "settings.docs")
        .unwrap();
    assert_eq!(docs_item["label"], "API documentation");
    assert!(application_item.get("label_key").is_none());

    let permissions = get_json(
        &app,
        &cookie,
        "/api/console/settings/roles/permission-options",
    )
    .await;
    let roles_feature = permissions["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|permission| permission["code"] == "settings_feature.access.system.roles")
        .unwrap();
    assert_eq!(
        roles_feature["settings_feature"]["label"],
        "权限管理（动态）"
    );
    assert!(roles_feature["settings_feature"].get("label_key").is_none());

    let policy = get_json(
        &app,
        &cookie,
        "/api/console/settings/roles/console-policy-catalog",
    )
    .await;
    assert_eq!(policy["data"]["locale"], "zh_Hans");
    assert_eq!(
        policy["data"]["group_strategy_options"][0]["label"],
        "完全开放（动态）"
    );
    let applications = policy["data"]["groups"]
        .as_array()
        .unwrap()
        .iter()
        .find(|group| group["group_id"] == "system.applications")
        .unwrap();
    assert_eq!(applications["label"], "应用管理（动态）");
    assert_eq!(applications["description"], "应用管理操作（动态）");
    let operation = applications["operations"]
        .as_array()
        .unwrap()
        .first()
        .unwrap();
    assert!(operation["summary"]
        .as_str()
        .is_some_and(|value| !value.is_empty()));
    assert!(operation["description"]
        .as_str()
        .is_some_and(|value| !value.contains("动态")));
    let resource = policy["data"]["resources"]
        .as_array()
        .unwrap()
        .iter()
        .find(|resource| resource["resource_code"] == "applications")
        .unwrap();
    assert_eq!(resource["label"], "应用（动态）");
    assert_eq!(
        resource["description"],
        "Applications in the current workspace"
    );
}
