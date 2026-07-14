use crate::_tests::support::{login_and_capture_cookie, test_app_with_database_url};
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
            'available', null, '/tmp/plugins/fixture_frontend_blocks/0.1.0', null, null,
            'unsigned', null, null, null, $4, $5
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
        insert into frontend_block_catalog (
            id, installation_id, provider_code, plugin_id, plugin_version, contribution_code,
            title, runtime, entry, context_contract, permission_network, permission_storage,
            permission_secrets, ui_capabilities, code_template, code_template_version, code_template_language, code_modules
        ) values (
            $1, $2, $3, $4, '0.1.0',
            'hero_banner', 'Hero Banner', 'iframe', 'blocks/hero/index.html',
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
        "source": "@1flowbase/block-sdk",
        "type_declarations": "export declare function defineBlock(input: unknown): unknown;"
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
async fn frontend_block_catalog_route_lists_only_assigned_workspace_blocks() {
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

    assert_eq!(entries.len(), 1);
    assert_eq!(
        entries[0]["contribution_code"].as_str(),
        Some("hero_banner")
    );
    assert_eq!(entries[0]["runtime"].as_str(), Some("iframe"));
    assert_eq!(
        entries[0]["code_template"].as_str(),
        Some("export default function HeroBanner() { return <section>Hero</section>; }")
    );
    assert_eq!(entries[0]["code_template_version"].as_str(), Some("1.0.0"));
    assert_eq!(entries[0]["code_template_language"].as_str(), Some("tsx"));
    assert_eq!(
        entries[0]["code_modules"][0]["source"].as_str(),
        Some("@1flowbase/block-sdk")
    );
    assert_eq!(
        entries[0]["context_contract"]["primitives"][0].as_str(),
        Some("text")
    );
}
