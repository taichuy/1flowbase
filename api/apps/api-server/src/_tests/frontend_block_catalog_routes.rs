use crate::_tests::support::{
    login_and_capture_cookie, test_api_state_with_database_url, test_config,
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

async fn seed_frontend_block(
    database_url: &str,
    workspace_assigned: bool,
    include_react_host_module: bool,
) -> Uuid {
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
            "type_declarations": "declare module '@acme/native-components' {}"
        },
        {
            "source": "@acme/runtime-utils",
            "version": "1.2.3",
            "exports": ["useRuntimeValue"],
            "binding": "fetched",
            "assets": [{
                "path": "browser-assets/native-components.js",
                "role": "browser_module",
                "media_type": "text/javascript; charset=utf-8",
                "sha256": "b5e317e6a0049e9af18eae918c3347af3626f7f3a1bbf0d32567d005260480e0"
            }],
            "type_declarations": "declare module '@acme/runtime-utils' { export function useRuntimeValue(): string; }"
        }
    ]))
    .execute(&pool)
    .await
    .unwrap();

    if include_react_host_module {
        sqlx::query(
            r#"
            update frontend_block_catalog
            set code_modules = jsonb_build_array($2::jsonb) || code_modules
            where installation_id = $1
            "#,
        )
        .bind(installation_id)
        .bind(json!({
            "source": "react",
            "version": "19.2.5",
            "exports": ["default", "useState"],
            "binding": "host",
            "assets": [],
            "type_declarations": ""
        }))
        .execute(&pool)
        .await
        .unwrap();
    }

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

async fn seed_persisted_component(database_url: &str) -> Uuid {
    let pool = PgPool::connect(database_url).await.unwrap();
    let actor_id: Uuid = sqlx::query_scalar("select id from users where account = 'root' limit 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    let component_id = Uuid::now_v7();
    sqlx::query(
        r#"
        insert into ui_component_records (
            id, component_code, name, description, import_code, source_code, origin,
            source, "group", upstream_identity, upstream_version, version, keywords,
            created_by, updated_by
        ) values ($1, 'taichuy.opaque.widget', 'Opaque Widget',
            'Persisted without dependency availability',
            'import Widget from ''@definitely/not-installed'';',
            '<Widget impossible={{ syntax: true }} />', 'official', 'taichuy', 'opaque',
            '@definitely/not-installed', '99.0.0', '1.0.0', array['opaque'], $2, $2)
        "#,
    )
    .bind(component_id)
    .bind(actor_id)
    .execute(&pool)
    .await
    .unwrap();
    component_id
}

#[tokio::test]
async fn wp_d4_persisted_component_and_registered_asset_routes_are_independent() {
    let (app, database_url) = test_frontend_block_app_with_database_url().await;
    let installation_id = seed_frontend_block(&database_url, false, false).await;
    let component_id = seed_persisted_component(&database_url).await;
    let pool = PgPool::connect(&database_url).await.unwrap();
    let _workspace_id: Uuid =
        sqlx::query_scalar("select id from workspaces order by created_at asc limit 1")
            .fetch_one(&pool)
            .await
            .unwrap();
    let (cookie, _) = login_and_capture_cookie(&app, "root", "change-me").await;

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/console/frontstage/components?query=not-installed&limit=1")
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
    assert_eq!(payload["data"]["items"][0]["id"], component_id.to_string());
    assert_eq!(
        payload["data"]["items"][0]["import_code"],
        "import Widget from '@definitely/not-installed';"
    );
    assert_eq!(
        payload["data"]["items"][0]["source_code"],
        "<Widget impossible={{ syntax: true }} />"
    );

    let detail_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/console/frontstage/components/{component_id}"))
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
    assert_eq!(detail["data"]["component_code"], "taichuy.opaque.widget");
    assert_eq!(
        detail["data"]["import_code"],
        payload["data"]["items"][0]["import_code"]
    );

    let asset_url = "/api/console/frontstage/component-module-assets/b5e317e6a0049e9af18eae918c3347af3626f7f3a1bbf0d32567d005260480e0";
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

    let style_url = "/api/console/frontstage/component-module-assets/adcff41acf67a64cfedd858a969fee27e6ae7ae328cd3b7afe5ff1263fe2a34f";
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
async fn wp_d4_empty_persistence_yields_empty_components_despite_registered_exports() {
    let (app, database_url) = test_frontend_block_app_with_database_url().await;
    seed_frontend_block(&database_url, false, false).await;
    let pool = PgPool::connect(&database_url).await.unwrap();
    let _workspace_id: Uuid =
        sqlx::query_scalar("select id from workspaces order by created_at asc limit 1")
            .fetch_one(&pool)
            .await
            .unwrap();
    let (cookie, _) = login_and_capture_cookie(&app, "root", "change-me").await;

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/console/frontstage/components?limit=20")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let page: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
    // Hooks, message/theme/version exports, and icons must never be inferred as components.
    assert_eq!(page["data"]["total"], 0);
    assert_eq!(page["data"]["items"], json!([]));
}

#[tokio::test]
async fn dependency_lock_resolver_route_is_not_exposed() {
    let (app, _) = test_frontend_block_app_with_database_url().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/frontstage/component-dependency-lock")
                .header("cookie", &cookie)
                .header("x-csrf-token", csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "source_code": "export default 1;" }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn retained_asset_authorization_does_not_read_block_dependency_lock() {
    let (app, database_url) = test_frontend_block_app_with_database_url().await;
    let installation_id = seed_frontend_block(&database_url, true, false).await;
    let pool = PgPool::connect(&database_url).await.unwrap();
    let old_bytes = b"export function RetainedButton() {}\n";
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

    let (cookie, _) = login_and_capture_cookie(&app, "root", "change-me").await;
    let asset_url = format!("/api/console/frontstage/component-module-assets/{old_sha256}");
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
}

#[tokio::test]
async fn frontend_block_catalog_route_has_no_system_builtin_jsx_block() {
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
    assert!(entries.iter().all(|entry| {
        entry["provider_code"] != "1flowbase"
            || entry["contribution_code"] != "frontstage.js-ui-block"
    }));
}

#[tokio::test]
async fn frontend_block_catalog_route_lists_assigned_workspace_blocks() {
    let (app, database_url) = test_frontend_block_app_with_database_url().await;
    let (cookie, _) = login_and_capture_cookie(&app, "root", "change-me").await;
    seed_frontend_block(&database_url, false, false).await;
    seed_frontend_block(&database_url, true, false).await;

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

    assert_eq!(entries.len(), 1);
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
    assert!(entry.get("code_modules").is_none());
    assert_eq!(entry["isolated_entry_asset"], Value::Null);
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
async fn isolated_catalog_projects_its_entry_asset_without_module_dependencies() {
    let (app, database_url) = test_frontend_block_app_with_database_url().await;
    let installation_id = seed_frontend_block(&database_url, true, false).await;
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
    assert!(isolated.get("code_modules").is_none());
    assert_eq!(
        isolated["isolated_entry_asset"]["sha256"],
        "b5e317e6a0049e9af18eae918c3347af3626f7f3a1bbf0d32567d005260480e0"
    );
}
