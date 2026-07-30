use crate::_tests::support::{
    login_and_capture_cookie, test_api_state_with_database_url, test_config,
};
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use control_plane::ports::{I18nCatalogRepository, UpsertCatalogTranslationInput};
use domain::{CatalogLocale, CatalogMessageIdentity, CatalogTranslation};
use tower::ServiceExt;

async fn seed_translation(
    state: &crate::app_state::ApiState,
    _module: &str,
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
                CatalogMessageIdentity::new(msgid).unwrap(),
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

const CONSOLE_INTERFACE_MODULE: &str = "@taichuy/platform/console/interfaces";
const USER_API_KEY_INTERFACE_TEXTS: &[(&str, &str, &str, &str, &str)] = &[
    (
        "list_user_api_keys",
        "List user API keys",
        "List user API keys in the system backend.",
        "列出用户 API 密钥（动态）",
        "列出系统后端中的用户 API 密钥（动态）。",
    ),
    (
        "list_user_api_key_role_options",
        "List user API key role options",
        "List user API key role options in the system backend.",
        "列出用户 API 密钥角色选项（动态）",
        "列出系统后端中的用户 API 密钥角色选项（动态）。",
    ),
    (
        "create_user_api_key",
        "Create user API key",
        "Create user API key in the system backend.",
        "创建用户 API 密钥（动态）",
        "在系统后端中创建用户 API 密钥（动态）。",
    ),
    (
        "revoke_user_api_key",
        "Revoke user API key",
        "Revoke user API key in the system backend.",
        "撤销用户 API 密钥（动态）",
        "撤销系统后端中的用户 API 密钥（动态）。",
    ),
];

async fn get_json(app: &axum::Router, cookie: &str, path: &str, locale: &str) -> serde_json::Value {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(path)
                .header("cookie", cookie)
                .header("x-1flowbase-locale", locale)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap()
}

fn policy_operation<'a>(
    policy: &'a serde_json::Value,
    operation_id: &str,
) -> &'a serde_json::Value {
    policy["data"]["groups"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|group| group["operations"].as_array().unwrap())
        .find(|operation| operation["operation_id"] == operation_id)
        .unwrap()
}

// AC-006/012/013: real policy DTOs use the request-locale resolver while #1487 remains the
// unchanged English interface owner, including four independent user API key interfaces.
#[tokio::test]
async fn core_console_display_routes_resolve_dynamic_zh_hans_and_fallback_to_english() {
    let (fallback_state, _) = test_api_state_with_database_url().await;
    let fallback_app = crate::app_with_state_and_config(fallback_state, &test_config());
    let (fallback_cookie, _) = login_and_capture_cookie(&fallback_app, "root", "change-me").await;
    let fallback_policy = get_json(
        &fallback_app,
        &fallback_cookie,
        "/api/console/settings/roles/console-policy-catalog",
        "zh_Hans",
    )
    .await;
    for (operation_id, summary, description, _, _) in USER_API_KEY_INTERFACE_TEXTS {
        let operation = policy_operation(&fallback_policy, operation_id);
        assert_eq!(operation["summary"], *summary);
        assert_eq!(operation["description"], *description);
    }

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
    for (_, summary, description, translated_summary, translated_description) in
        USER_API_KEY_INTERFACE_TEXTS
    {
        seed_translation(
            &state,
            CONSOLE_INTERFACE_MODULE,
            summary,
            translated_summary,
        )
        .await;
        seed_translation(
            &state,
            CONSOLE_INTERFACE_MODULE,
            description,
            translated_description,
        )
        .await;
    }
    let app = crate::app_with_state_and_config(state, &test_config());
    let (cookie, _) = login_and_capture_cookie(&app, "root", "change-me").await;

    let navigation = get_json(&app, &cookie, "/api/console/navigation", "zh_Hans").await;
    let application_item = navigation["data"]["navigation_items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["item_id"] == "settings.applications")
        .unwrap();
    assert_eq!(application_item["label_key"], "auto.application_management");
    let docs_item = navigation["data"]["navigation_items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["item_id"] == "settings.docs")
        .unwrap();
    assert_eq!(docs_item["label_key"], "auto.api_documentation");
    assert!(application_item.get("label").is_none());

    let permissions = get_json(
        &app,
        &cookie,
        "/api/console/settings/roles/permission-options",
        "zh_Hans",
    )
    .await;
    let roles_feature = permissions["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|permission| permission["code"] == "settings_feature.access.system.roles")
        .unwrap();
    assert_eq!(
        roles_feature["settings_feature"]["label_key"],
        "auto.permission_management"
    );
    assert!(roles_feature["settings_feature"].get("label").is_none());

    let policy = get_json(
        &app,
        &cookie,
        "/api/console/settings/roles/console-policy-catalog",
        "zh_Hans",
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
    for (operation_id, _, _, translated_summary, translated_description) in
        USER_API_KEY_INTERFACE_TEXTS
    {
        let operation = policy_operation(&policy, operation_id);
        assert_eq!(operation["summary"], *translated_summary);
        assert_eq!(operation["description"], *translated_description);
    }
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

    let english_policy = get_json(
        &app,
        &cookie,
        "/api/console/settings/roles/console-policy-catalog",
        "en_US",
    )
    .await;
    for (operation_id, summary, description, _, _) in USER_API_KEY_INTERFACE_TEXTS {
        let operation = policy_operation(&english_policy, operation_id);
        assert_eq!(operation["summary"], *summary);
        assert_eq!(operation["description"], *description);
    }
}
