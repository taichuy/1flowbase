use crate::{
    _tests::support::{login_and_capture_cookie, test_api_state_with_database_url, test_config},
    provider_runtime::ApiProviderRuntime,
};
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use std::sync::Arc;
use tower::ServiceExt;
use uuid::Uuid;

async fn test_frontend_block_app_with_database_url() -> (axum::Router, String) {
    let (mut state, database_url) = test_api_state_with_database_url().await;
    let assembly = crate::extension_bus::assemble_extension_graph_input(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."),
        crate::extension_bus::DEFAULT_PLUGIN_SET_PATH,
        Vec::new(),
    )
    .unwrap();
    let snapshot = Arc::new(
        crate::extension_bus::ExtensionBootSnapshot::compile(
            Arc::new(assembly.compile_graph().unwrap()),
            assembly.interface_operations(),
        )
        .unwrap(),
    );
    let route_assembly = crate::routes::console_route_assembly::migrated_core_console_route_assembly_with_interface_operations(
        snapshot.interface_operations(),
    );
    let registry =
        crate::routes::console_route_assembly::compile_migrated_core_console_operation_registry(
            &state.settings_feature_registry,
            route_assembly.bindings(),
        )
        .unwrap();
    let mutable_state = Arc::get_mut(&mut state).unwrap();
    mutable_state.extension_boot_snapshot = Some(snapshot);
    mutable_state.console_operation_registry = Arc::new(registry);
    (
        crate::app_with_state_and_config(state, &test_config()),
        database_url,
    )
}

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
    std::fs::write(
        installed_path.join("browser-assets/native-components.css"),
        ".fixture { color: red; }\n",
    )
    .unwrap();

    sqlx::query(
        r#"
        insert into extension_installations (
            id, category, organization, artifact_id, artifact_version, plugin_id,
            contract_version, protocol, display_name, source_kind, trust_level,
            verification_status, desired_state, signature_status, metadata_json, created_by
        ) values (
            $1, 'capability-plugins', 'test', $2, '0.1.0', $3,
            '1flowbase.capability/v1', 'stdio_json', 'Fixture Frontend Blocks',
            'uploaded', 'checksum_only', 'valid', 'active_requested', 'missing', $4, $5
        )
        "#,
    )
    .bind(installation_id)
    .bind(&provider_code)
    .bind(&plugin_id)
    .bind(json!({}))
    .bind(actor_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"
        insert into extension_artifact_instances (
            node_id, installation_id, local_version, local_path,
            artifact_status, runtime_status, availability_status
        ) values ($1, $2, '0.1.0', $3, 'ready', 'inactive', 'available')
        "#,
    )
    .bind(test_config().api_node_id)
    .bind(installation_id)
    .bind(installed_path.display().to_string())
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
    .bind(json!([
        {
            "source": "@acme/native-components",
            "version": "1.2.3",
            "exports": ["Button"],
            "binding": "fetched",
            "assets": [
                {
                    "path": "browser-assets/native-components.js",
                    "role": "browser_module",
                    "media_type": "text/javascript; charset=utf-8",
                    "sha256": "b5e317e6a0049e9af18eae918c3347af3626f7f3a1bbf0d32567d005260480e0"
                },
                {
                    "path": "browser-assets/native-components.css",
                    "role": "shadow_style",
                    "media_type": "text/css; charset=utf-8",
                    "sha256": "adcff41acf67a64cfedd858a969fee27e6ae7ae328cd3b7afe5ff1263fe2a34f"
                }
            ],
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
        },
        {
            "source": "@acme/runtime-utils",
            "version": "1.2.3",
            "exports": ["useRuntimeValue"],
            "binding": "host",
            "assets": [],
            "type_declarations": "declare module '@acme/runtime-utils' { export function useRuntimeValue(): string; }",
            "components": []
        }
    ]))
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
    let (app, database_url) = test_frontend_block_app_with_database_url().await;
    let installation_id = seed_frontend_block(&database_url, false).await;
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
    assert_eq!(payload["data"]["items"][0]["exports"], json!(["Button"]));
    assert_eq!(
        payload["data"]["items"][0]["browser_asset"]["sha256"],
        "b5e317e6a0049e9af18eae918c3347af3626f7f3a1bbf0d32567d005260480e0"
    );
    assert_eq!(payload["data"]["items"][0]["binding"], "fetched");
    assert_eq!(
        payload["data"]["items"][0]["assets"][1]["role"],
        "shadow_style"
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

    let style_url = payload["data"]["items"][0]["assets"][1]["url"]
        .as_str()
        .unwrap();
    let style_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(style_url)
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(style_response.status(), StatusCode::OK);
    assert_eq!(
        style_response.headers()["content-type"],
        "text/css; charset=utf-8"
    );

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

    let installed_path: String = sqlx::query_scalar(
        "select local_path from extension_artifact_instances where installation_id = $1",
    )
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
async fn component_capabilities_route_includes_registered_exports_without_contracts() {
    let (app, database_url) = test_frontend_block_app_with_database_url().await;
    seed_frontend_block(&database_url, false).await;
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
                    "/api/console/frontstage/{workspace_id}/component-capabilities?query=runtime&limit=20"
                ))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let page: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(page["data"]["total"], 1);
    let item = &page["data"]["items"][0];
    assert_eq!(item["export_name"], "useRuntimeValue");
    assert_eq!(item["module_source"], "@acme/runtime-utils");
    assert_eq!(item["browser_asset"], Value::Null);
    assert_eq!(item["insert_snippet"], "useRuntimeValue");

    let component_id = item["component_id"]
        .as_str()
        .expect("registered export has a stable component id");
    let detail_response = app
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
    assert_eq!(detail["data"]["props"], json!([]));
    assert!(detail["data"]["typescript_declaration"]
        .as_str()
        .unwrap()
        .contains("useRuntimeValue"));
    assert!(detail["data"]["api_documentation"]
        .as_str()
        .unwrap()
        .contains("import { useRuntimeValue } from '@acme/runtime-utils';"));
}

#[tokio::test]
async fn component_dependency_lock_is_derived_from_source_imports() {
    let (app, database_url) = test_frontend_block_app_with_database_url().await;
    seed_frontend_block(&database_url, false).await;
    let pool = PgPool::connect(&database_url).await.unwrap();
    let workspace_id: Uuid =
        sqlx::query_scalar("select id from workspaces order by created_at asc limit 1")
            .fetch_one(&pool)
            .await
            .unwrap();
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/console/frontstage/{workspace_id}/component-dependency-lock"
                ))
                .header("cookie", &cookie)
                .header("x-csrf-token", csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "source_code": "import { useState } from 'react';\nimport { Button } from '@acme/native-components';\nexport default Button;"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let payload: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(
        payload["data"]["dependency_lock"],
        json!([{
            "module_source": "@acme/native-components",
            "module_version": "1.2.3",
            "binding": "fetched",
            "assets": [{
                "role": "browser_module",
                "media_type": "text/javascript; charset=utf-8",
                "sha256": "b5e317e6a0049e9af18eae918c3347af3626f7f3a1bbf0d32567d005260480e0",
                "url": format!(
                    "/api/console/frontstage/{workspace_id}/component-module-assets/b5e317e6a0049e9af18eae918c3347af3626f7f3a1bbf0d32567d005260480e0"
                ),
                "integrity": "verified_sha256"
            }, {
                "role": "shadow_style",
                "media_type": "text/css; charset=utf-8",
                "sha256": "adcff41acf67a64cfedd858a969fee27e6ae7ae328cd3b7afe5ff1263fe2a34f",
                "url": format!(
                    "/api/console/frontstage/{workspace_id}/component-module-assets/adcff41acf67a64cfedd858a969fee27e6ae7ae328cd3b7afe5ff1263fe2a34f"
                ),
                "integrity": "verified_sha256"
            }],
            "exports": ["Button"]
        }])
    );
}

#[tokio::test]
async fn ac_001_locked_asset_survives_current_catalog_digest_replacement() {
    let (app, database_url) = test_frontend_block_app_with_database_url().await;
    let installation_id = seed_frontend_block(&database_url, true).await;
    let pool = PgPool::connect(&database_url).await.unwrap();
    let workspace_id: Uuid =
        sqlx::query_scalar("select id from workspaces order by created_at asc limit 1")
            .fetch_one(&pool)
            .await
            .unwrap();
    let page_id = Uuid::now_v7();
    let tab_id = Uuid::now_v7();
    let mut transaction = pool.begin().await.unwrap();
    sqlx::query(
        r#"
        insert into frontstage_pages (
            id, workspace_id, kind, title, rank
        ) values ($1, $2, 'page', 'Retained asset fixture', 'a')
        "#,
    )
    .bind(page_id)
    .bind(workspace_id)
    .execute(&mut *transaction)
    .await
    .unwrap();
    sqlx::query(
        r#"
        insert into frontstage_page_tabs (
            id, workspace_id, page_id, title, rank, is_default, document_root_uid
        ) values ($1, $2, $3, 'Default', 'a', true, $4)
        "#,
    )
    .bind(tab_id)
    .bind(workspace_id)
    .bind(page_id)
    .bind(format!("frontstage.tab.{tab_id}.root"))
    .execute(&mut *transaction)
    .await
    .unwrap();
    transaction.commit().await.unwrap();
    let actor_id: Uuid = sqlx::query_scalar("select id from users where account = 'root' limit 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    let old_bytes = b"export function Button() {}\n";
    let old_sha256 = format!("{:x}", Sha256::digest(old_bytes));
    let new_sha256 = format!("{:x}", Sha256::digest(b"export function ButtonV2() {}\n"));

    sqlx::query(
        r#"
        insert into retained_frontend_module_assets (
            installation_id, module_source, sha256, media_type, bytes
        ) values ($1, '@acme/native-components', $2, 'text/javascript; charset=utf-8', $3)
        "#,
    )
    .bind(installation_id)
    .bind(&old_sha256)
    .bind(old_bytes.as_slice())
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"
        update frontend_block_catalog
        set code_modules = jsonb_set(code_modules, '{0,assets,0,sha256}', to_jsonb($2::text))
        where installation_id = $1
        "#,
    )
    .bind(installation_id)
    .bind(&new_sha256)
    .execute(&pool)
    .await
    .unwrap();

    let source_code = "import { Button } from '@acme/native-components';\nexport default Button;";
    sqlx::query(
        r#"
        insert into frontstage_block_codes (
            id, workspace_id, page_id, code_ref, code, source_sha256,
            dependency_lock, created_by, updated_by
        ) values ($1, $2, $3, 'retained-asset-fixture', $4, $5, $6, $7, $7)
        "#,
    )
    .bind(Uuid::now_v7())
    .bind(workspace_id)
    .bind(page_id)
    .bind(source_code)
    .bind(format!("{:x}", Sha256::digest(source_code.as_bytes())))
    .bind(json!([{
        "module_source": "@acme/native-components",
        "module_version": "1.2.3",
        "binding": "fetched",
        "assets": [{
            "role": "browser_module",
            "media_type": "text/javascript; charset=utf-8",
            "sha256": old_sha256,
            "url": format!("/api/console/frontstage/{workspace_id}/component-module-assets/{old_sha256}")
        }],
        "exports": ["Button"]
    }]))
    .bind(actor_id)
    .execute(&pool)
    .await
    .unwrap();

    let (cookie, _) = login_and_capture_cookie(&app, "root", "change-me").await;
    let asset_url =
        format!("/api/console/frontstage/{workspace_id}/component-module-assets/{old_sha256}");
    let retained_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(&asset_url)
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(retained_response.status(), StatusCode::OK);
    assert_eq!(
        to_bytes(retained_response.into_body(), usize::MAX)
            .await
            .unwrap()
            .as_ref(),
        old_bytes
    );

    sqlx::query("delete from frontstage_block_codes where code_ref = 'retained-asset-fixture'")
        .execute(&pool)
        .await
        .unwrap();
    let unreferenced_response = app
        .oneshot(
            Request::builder()
                .uri(&asset_url)
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(unreferenced_response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn frontend_block_catalog_route_includes_system_builtin_jsx_block() {
    let (app, _) = test_frontend_block_app_with_database_url().await;
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
    assert_eq!(jsx_block["code_template_version"], "6.0.0");
    assert_eq!(jsx_block["runtime_kind"], "trusted_native");
    assert_eq!(jsx_block["execution_kind"], "ui_mount");
    assert_eq!(jsx_block["isolation_requirement"], "trusted_host_realm");
    assert_eq!(jsx_block["lifecycle_kind"], "workspace_assignment");
    assert_eq!(jsx_block["provenance"]["module_kind"], "boot_core");
    assert!(jsx_block["graph_fingerprint"].as_str().is_some());
    let code_template = jsx_block["code_template"].as_str().unwrap();
    assert!(code_template.contains("export default function ExampleBlock"));
    assert!(code_template.contains("useState"));
    assert!(code_template.contains("onClick"));
    assert!(code_template.contains("import 'tailwindcss'"));
    assert!(code_template.contains("className=\"grid gap-4 p-4\""));
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
    assert!(sdk_declarations.contains("readonly application: BlockContextEntity | null"));
    assert!(sdk_declarations.contains("readonly api"));
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
    assert_eq!(sdk_module["exports"], json!(["blockSdkVersion"]));
    assert_eq!(sdk_module["binding"], "fetched");
    assert_eq!(
        sdk_module["assets"][0]["sha256"],
        "89d33c09ed7013cf4f60f07b5b4b511686e57e011867ec7656f8bc3538c0298f"
    );
    assert_eq!(sdk_module["assets"][0]["role"], "browser_module");
    let native_module = jsx_block["code_modules"]
        .as_array()
        .unwrap()
        .iter()
        .find(|module| module["source"] == "@1flowbase/native-components")
        .unwrap();
    assert_eq!(native_module["version"], "1.0.0");
    assert_eq!(
        native_module["exports"],
        json!(["ScrollableSurface", "Surface"])
    );
    assert_eq!(
        native_module["assets"][0]["sha256"],
        "4b0132d6bf899d0016ec4c94f9dd665b41d6b75b413fc66037706c80965af388"
    );
    assert_eq!(native_module["assets"][1]["role"], "shadow_style");
    assert_eq!(
        jsx_block["code_modules"]
            .as_array()
            .unwrap()
            .iter()
            .map(|module| module["source"].as_str().unwrap())
            .collect::<Vec<_>>(),
        vec![
            "react",
            "antd",
            "@1flowbase/block-sdk",
            "@1flowbase/native-components",
            "@ant-design/icons",
            "@1flowbase/charts",
            "@1flowbase/rich-text"
        ]
    );
    let rich_text_module = jsx_block["code_modules"]
        .as_array()
        .unwrap()
        .iter()
        .find(|module| module["source"] == "@1flowbase/rich-text")
        .unwrap();
    assert_eq!(rich_text_module["exports"], json!(["VditorEditor"]));
    assert!(rich_text_module["type_declarations"]
        .as_str()
        .unwrap()
        .contains("interface VditorEditorProps"));
    for module in jsx_block["code_modules"].as_array().unwrap() {
        let source = module["source"].as_str().unwrap();
        if matches!(source, "react" | "antd") {
            assert_eq!(module["binding"], "host");
            assert_eq!(module["assets"], json!([]));
        } else {
            assert_eq!(module["binding"], "fetched");
            assert_eq!(
                module["assets"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter(|asset| asset["role"] == "browser_module")
                    .count(),
                1
            );
        }
    }
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
        "select count(*) from extension_installations where artifact_id = '1flowbase'",
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
    let retained_charts_bytes: Vec<u8> = sqlx::query_scalar(
        r#"
        select bytes
        from retained_frontend_module_assets
        where installation_id in (
            select id from extension_installations where artifact_id = '1flowbase'
        )
          and module_source = '@1flowbase/charts'
          and sha256 = 'b4df3cc6116a254e1dd7451e99c5d01a48cd23bd4a6f35df10f97dba2e888338'
        "#,
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        format!("{:x}", Sha256::digest(&retained_charts_bytes)),
        "b4df3cc6116a254e1dd7451e99c5d01a48cd23bd4a6f35df10f97dba2e888338"
    );
}

#[tokio::test]
async fn frontend_block_catalog_route_lists_builtin_and_assigned_workspace_blocks() {
    let (app, database_url) = test_frontend_block_app_with_database_url().await;
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
    assert_eq!(entry["code_modules"][0]["exports"], json!(["Button"]));
    assert_eq!(
        entry["code_modules"][0]["assets"][0]["sha256"],
        "b5e317e6a0049e9af18eae918c3347af3626f7f3a1bbf0d32567d005260480e0"
    );
    assert_eq!(entry["code_modules"][0]["binding"], "fetched");
    assert_eq!(
        entry["code_modules"][0]["assets"][0]["integrity"],
        "verified_sha256"
    );
    assert!(entry["code_modules"][0]["assets"][0]["url"]
        .as_str()
        .unwrap()
        .contains("/component-module-assets/"));
    assert_eq!(entry["runtime_kind"], "trusted_native");
    assert_eq!(entry["execution_kind"], "ui_mount");
    assert_eq!(entry["isolation_requirement"], "trusted_host_realm");
    assert_eq!(entry["lifecycle_kind"], "workspace_assignment");
    assert_eq!(
        entry["requested_permissions"],
        json!(["frontend-block.ui-mount.trusted-host"])
    );
    assert_eq!(entry["requested_permissions"], entry["granted_permissions"]);
    assert_eq!(entry["disable_reason"], Value::Null);
    assert_eq!(
        entry["context_contract"]["primitives"][0].as_str(),
        Some("text")
    );
}

#[tokio::test]
async fn d5_p3_catalog_projects_isolated_runtime_contract_without_new_dto_fields() {
    let (app, database_url) = test_frontend_block_app_with_database_url().await;
    let installation_id = seed_frontend_block(&database_url, true).await;
    let pool = PgPool::connect(&database_url).await.unwrap();
    sqlx::query(
        "update frontend_block_catalog set runtime = 'isolated_iframe', entry = '@acme/native-components' where installation_id = $1",
    )
    .bind(installation_id)
    .execute(&pool)
    .await
    .unwrap();
    let (cookie, _) = login_and_capture_cookie(&app, "root", "change-me").await;

    let response = app
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
    let isolated = payload["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["contribution_code"] == "hero_banner")
        .unwrap();
    assert_eq!(isolated["runtime"], "isolated_iframe");
    assert_eq!(isolated["runtime_kind"], "isolated");
    assert_eq!(isolated["execution_kind"], "ui_mount");
    assert_eq!(isolated["isolation_requirement"], "independent_realm");
    assert_eq!(isolated["lifecycle_kind"], "workspace_assignment");
    assert_eq!(
        isolated["requested_permissions"],
        json!(["frontend-block.ui-mount.isolated-realm"])
    );
    assert_eq!(
        isolated["requested_permissions"],
        isolated["granted_permissions"]
    );
}
