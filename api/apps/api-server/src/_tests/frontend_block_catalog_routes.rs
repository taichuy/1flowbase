use crate::{
    _tests::support::{
        login_and_capture_cookie, test_api_state_with_database_url, test_app_with_database_url,
    },
    provider_runtime::ApiProviderRuntime,
};
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use serde_json::{json, Value};
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

async fn seed_frontend_block(database_url: &str, workspace_assigned: bool) -> Uuid {
    let pool = PgPool::connect(database_url).await.unwrap();
    let workspace_id: Uuid =
        sqlx::query_scalar("select id from workspaces order by created_at asc limit 1")
            .fetch_one(&pool)
            .await
            .unwrap();
    let actor_id: Uuid = sqlx::query_scalar("select id from users where account = 'root' limit 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    let installation_id = Uuid::now_v7();
    let suffix = if workspace_assigned {
        "assigned"
    } else {
        "hidden"
    };
    let provider_code = format!("fixture_frontend_blocks_{suffix}");
    let plugin_id = format!("fixture_frontend_blocks_{suffix}@0.1.0");
    let installed_path =
        std::env::temp_dir().join(format!("1flowbase-frontend-module-{installation_id}"));
    std::fs::create_dir_all(installed_path.join("browser-assets")).unwrap();
    std::fs::write(
        installed_path.join("browser-assets/native-components.js"),
        "export function Button() {}\n",
    )
    .unwrap();

    sqlx::query(
        r#"
        insert into plugin_installations (
            id, provider_code, plugin_id, plugin_version, contract_version, protocol,
            display_name, source_kind, trust_level, verification_status, desired_state,
            artifact_status, runtime_status, availability_status, package_path, installed_path,
            checksum, manifest_fingerprint, signature_status, signature_algorithm, signing_key_id,
            last_load_error, metadata_json, created_by
        ) values (
            $1, $2, $3, '0.1.0',
            '1flowbase.capability/v1', 'stdio_json', 'Fixture Frontend Blocks',
            'uploaded', 'checksum_only', 'valid', 'active_requested', 'ready', 'inactive',
            'available', null, $4, null, null,
            'unsigned', null, null, null, $5, $6
        )
        "#,
    )
    .bind(installation_id)
    .bind(&provider_code)
    .bind(&plugin_id)
    .bind(installed_path.display().to_string())
    .bind(json!({}))
    .bind(actor_id)
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        r#"
        insert into frontend_block_catalog (
            id, installation_id, provider_code, plugin_id, plugin_version, contribution_code,
            title, runtime, entry, context_contract, permission_network, permission_storage,
            permission_secrets, ui_capabilities, code_template, code_template_version, code_template_language, code_modules
        ) values (
            $1, $2, $3, $4, '0.1.0',
            'hero_banner', 'Hero Banner', 'native_react', 'blocks/hero/index.html',
            $5, 'none', 'none', 'none', $6, $7, '1.0.0', 'tsx', $8
        )
        "#,
    )
    .bind(Uuid::now_v7())
    .bind(installation_id)
    .bind(&provider_code)
    .bind(&plugin_id)
    .bind(json!({
        "primitives": ["text", "image"],
        "input_schema": { "type": "object" }
    }))
    .bind(json!(["responsive"]))
    .bind("export default function HeroBanner() { return <section>Hero</section>; }")
    .bind(json!([{
        "source": "@acme/native-components",
        "version": "1.2.3",
        "browser_asset": {
            "path": "browser-assets/native-components.js",
            "sha256": "b5e317e6a0049e9af18eae918c3347af3626f7f3a1bbf0d32567d005260480e0"
        },
        "type_declarations": "declare module '@acme/native-components' {}",
        "components": [{
            "component_code": "button",
            "export_name": "Button",
            "upstream": {
                "package": "antd",
                "component": "Button",
                "version": "5.x"
            },
            "description": "Native React Button component.",
            "props": [{
                "name": "actionId",
                "type": "string",
                "required": false,
                "description": "点击后发送的区块 action 标识。"
            }],
            "limitations": ["不支持 React onClick。"],
            "examples": [{
                "title": "触发保存操作",
                "code": "<Button actionId=\"save\">保存</Button>"
            }],
            "insert_snippet": "<Button actionId=\"save\">保存</Button>"
        }]
    }]))
    .execute(&pool)
    .await
    .unwrap();

    if workspace_assigned {
        sqlx::query(
            r#"
            insert into plugin_assignments (
                id, installation_id, workspace_id, provider_code, assigned_by
            ) values ($1, $2, $3, $4, $5)
            "#,
        )
        .bind(Uuid::now_v7())
        .bind(installation_id)
        .bind(workspace_id)
        .bind(&provider_code)
        .bind(actor_id)
        .execute(&pool)
        .await
        .unwrap();
    }

    installation_id
}

#[tokio::test]
async fn d2_ac_001_and_004_component_contract_and_registered_asset_route_are_fail_closed() {
    let (app, database_url) = test_app_with_database_url().await;
    let installation_id = seed_frontend_block(&database_url, true).await;
    let pool = PgPool::connect(&database_url).await.unwrap();
    let workspace_id: Uuid =
        sqlx::query_scalar("select id from workspaces order by created_at asc limit 1")
            .fetch_one(&pool)
            .await
            .unwrap();
    let (cookie, _) = login_and_capture_cookie(&app, "root", "change-me").await;

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/frontstage/{workspace_id}/component-capabilities?installation_id={installation_id}&query=button&limit=1"
                ))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let payload: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(payload["data"]["total"], 1);
    assert_eq!(payload["data"]["items"][0]["export_name"], "Button");
    assert_eq!(
        payload["data"]["items"][0]["module_source"],
        "@acme/native-components"
    );
    assert_eq!(payload["data"]["items"][0]["module_version"], "1.2.3");
    assert_eq!(
        payload["data"]["items"][0]["browser_asset"]["sha256"],
        "b5e317e6a0049e9af18eae918c3347af3626f7f3a1bbf0d32567d005260480e0"
    );
    assert!(!payload["data"]["items"][0]["browser_asset"]["url"]
        .as_str()
        .unwrap()
        .contains("/tmp/"));
    let component_id = payload["data"]["items"][0]["component_id"]
        .as_str()
        .unwrap();

    let detail_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/frontstage/{workspace_id}/component-capabilities/{component_id}"
                ))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(detail_response.status(), StatusCode::OK);
    let detail: Value = serde_json::from_slice(
        &to_bytes(detail_response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(detail["data"]["props"][0]["name"], "actionId");
    assert!(detail["data"]["typescript_declaration"]
        .as_str()
        .unwrap()
        .contains("readonly actionId?: string"));
    assert!(!detail["data"]["typescript_declaration"]
        .as_str()
        .unwrap()
        .contains("@1flowbase-component"));

    let asset_url = payload["data"]["items"][0]["browser_asset"]["url"]
        .as_str()
        .unwrap();
    let asset_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(asset_url)
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(asset_response.status(), StatusCode::OK);
    assert_eq!(
        asset_response.headers()["content-type"],
        "text/javascript; charset=utf-8"
    );
    let asset_bytes = to_bytes(asset_response.into_body(), usize::MAX)
        .await
        .unwrap();
    assert_eq!(asset_bytes.as_ref(), b"export function Button() {}\n");

    let missing_asset_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(asset_url.replace(
                    "b5e317e6a0049e9af18eae918c3347af3626f7f3a1bbf0d32567d005260480e0",
                    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                ))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(missing_asset_response.status(), StatusCode::NOT_FOUND);

    let installed_path: String =
        sqlx::query_scalar("select installed_path from plugin_installations where id = $1")
            .bind(installation_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    std::fs::write(
        std::path::Path::new(&installed_path).join("browser-assets/native-components.js"),
        "export function Tampered() {}\n",
    )
    .unwrap();
    let corrupt_asset_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(asset_url)
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(corrupt_asset_response.status(), StatusCode::BAD_GATEWAY);

    let unauthenticated_asset_response = app
        .oneshot(
            Request::builder()
                .uri(asset_url)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        unauthenticated_asset_response.status(),
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn frontend_block_catalog_route_includes_system_builtin_jsx_block() {
    let (app, _) = test_app_with_database_url().await;
    let (cookie, _) = login_and_capture_cookie(&app, "root", "change-me").await;

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/console/frontend-blocks")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let payload: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
    let entries = payload["data"].as_array().unwrap();
    let jsx_block = entries
        .iter()
        .find(|entry| {
            entry["provider_code"] == "1flowbase"
                && entry["contribution_code"] == "frontstage.js-ui-block"
        })
        .expect("system bootstrap must register the built-in JSX block");

    assert_eq!(jsx_block["code_template_language"], "tsx");
    assert_eq!(jsx_block["code_template_version"], "4.0.0");
    let code_template = jsx_block["code_template"].as_str().unwrap();
    assert!(code_template.contains("export default function ExampleBlock"));
    assert!(code_template.contains("useState"));
    assert!(code_template.contains("onClick"));
    assert!(!code_template.contains("BlockModule"));
    let sdk_declarations = jsx_block["code_modules"]
        .as_array()
        .unwrap()
        .iter()
        .find(|module| module["source"] == "@1flowbase/block-sdk")
        .and_then(|module| module["type_declarations"].as_str())
        .unwrap();
    assert!(sdk_declarations.contains("interface BlockComponentProps"));
    assert!(sdk_declarations.contains("readonly inputs"));
    assert!(!sdk_declarations.contains("interfaceId"));
    assert!(!sdk_declarations.contains("schemaDigest"));
    assert!(!sdk_declarations.contains("defineBlock"));
    let native_declarations = jsx_block["code_modules"]
        .as_array()
        .unwrap()
        .iter()
        .find(|module| module["source"] == "@1flowbase/native-components")
        .and_then(|module| module["type_declarations"].as_str())
        .unwrap();
    assert!(native_declarations.contains("interface SurfaceProps"));
    let sdk_module = jsx_block["code_modules"]
        .as_array()
        .unwrap()
        .iter()
        .find(|module| module["source"] == "@1flowbase/block-sdk")
        .unwrap();
    assert_eq!(sdk_module["version"], "1.0.0");
    assert_eq!(
        sdk_module["browser_asset"]["sha256"],
        "89d33c09ed7013cf4f60f07b5b4b511686e57e011867ec7656f8bc3538c0298f"
    );
    assert!(sdk_module["browser_asset"].get("path").is_none());
    assert!(sdk_module["browser_asset"].get("url").is_none());
    let native_module = jsx_block["code_modules"]
        .as_array()
        .unwrap()
        .iter()
        .find(|module| module["source"] == "@1flowbase/native-components")
        .unwrap();
    assert_eq!(native_module["version"], "1.0.0");
    assert_eq!(
        native_module["browser_asset"]["sha256"],
        "00c568e229c81c4c18af20961ec14663efa6f7460c0134708391746d7e8ec2e0"
    );
    assert!(native_module["browser_asset"].get("path").is_none());
    assert!(native_module["browser_asset"].get("url").is_none());
    assert_eq!(
        jsx_block["code_modules"]
            .as_array()
            .unwrap()
            .iter()
            .map(|module| module["source"].as_str().unwrap())
            .collect::<Vec<_>>(),
        vec!["@1flowbase/block-sdk", "@1flowbase/native-components"]
    );
}

#[tokio::test]
async fn builtin_jsx_block_bootstrap_is_idempotent() {
    let (state, database_url) = test_api_state_with_database_url().await;
    let pool = PgPool::connect(&database_url).await.unwrap();
    let actor_user_id: Uuid =
        sqlx::query_scalar("select id from users where account = 'root' limit 1")
            .fetch_one(&pool)
            .await
            .unwrap();
    let workspace_id: Uuid =
        sqlx::query_scalar("select id from workspaces order by created_at asc limit 1")
            .fetch_one(&pool)
            .await
            .unwrap();

    control_plane::plugin_management::PluginManagementService::new(
        state.store.clone(),
        ApiProviderRuntime::new(state.provider_runtime.clone()),
        state.official_plugin_source.clone(),
        state.provider_install_root.clone(),
    )
    .with_node_id(state.api_node_id.clone())
    .ensure_builtin_plugin(
        control_plane::plugin_management::EnsureBuiltinPluginCommand {
            actor_user_id,
            package_root: crate::builtin_jsx_block_package_root()
                .unwrap()
                .display()
                .to_string(),
        },
    )
    .await
    .unwrap();

    let installation_count: i64 = sqlx::query_scalar(
        "select count(*) from plugin_installations where provider_code = '1flowbase'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let catalog_count: i64 = sqlx::query_scalar(
        "select count(*) from frontend_block_catalog where provider_code = '1flowbase' and contribution_code = 'frontstage.js-ui-block'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let assignment_count: i64 = sqlx::query_scalar(
        "select count(*) from plugin_assignments where workspace_id = $1 and provider_code = '1flowbase'",
    )
    .bind(workspace_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(installation_count, 1);
    assert_eq!(catalog_count, 1);
    assert_eq!(assignment_count, 0);
}

#[tokio::test]
async fn frontend_block_catalog_route_lists_builtin_and_assigned_workspace_blocks() {
    let (app, database_url) = test_app_with_database_url().await;
    let (cookie, _) = login_and_capture_cookie(&app, "root", "change-me").await;
    seed_frontend_block(&database_url, false).await;
    seed_frontend_block(&database_url, true).await;

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/console/frontend-blocks")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let payload: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
    let entries = payload["data"].as_array().unwrap();

    assert_eq!(entries.len(), 2);
    let entry = entries
        .iter()
        .find(|entry| entry["contribution_code"] == "hero_banner")
        .expect("assigned fixture block must be visible");
    assert!(!entries
        .iter()
        .any(|entry| { entry["provider_code"] == "fixture_frontend_blocks_hidden" }));
    assert_eq!(entry["contribution_code"].as_str(), Some("hero_banner"));
    assert_eq!(entry["runtime"].as_str(), Some("native_react"));
    assert_eq!(
        entry["code_template"].as_str(),
        Some("export default function HeroBanner() { return <section>Hero</section>; }")
    );
    assert_eq!(entry["code_template_version"].as_str(), Some("1.0.0"));
    assert_eq!(entry["code_template_language"].as_str(), Some("tsx"));
    assert_eq!(
        entry["code_modules"][0]["source"].as_str(),
        Some("@acme/native-components")
    );
    assert_eq!(entry["code_modules"][0]["version"], "1.2.3");
    assert_eq!(
        entry["code_modules"][0]["browser_asset"]["sha256"],
        "b5e317e6a0049e9af18eae918c3347af3626f7f3a1bbf0d32567d005260480e0"
    );
    assert!(entry["code_modules"][0]["browser_asset"]
        .get("path")
        .is_none());
    assert!(entry["code_modules"][0]["browser_asset"]
        .get("url")
        .is_none());
    assert_eq!(
        entry["context_contract"]["primitives"][0].as_str(),
        Some("text")
    );
}
