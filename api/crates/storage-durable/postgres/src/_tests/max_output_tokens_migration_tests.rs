use sqlx::{migrate::Migrator, PgPool};
use std::borrow::Cow;
use storage_postgres::{connect, PgControlPlaneStore};
use uuid::Uuid;

const MIGRATION_VERSION: i64 = 20260722100000;
const MIGRATION_SQL: &str =
    include_str!("../../migrations/20260722100000_migrate_llm_max_output_tokens.sql");

fn base_database_url() -> String {
    std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:1flowbase@127.0.0.1:35432/1flowbase".into())
}

async fn isolated_database_url() -> String {
    let admin_pool = PgPool::connect(&base_database_url()).await.unwrap();
    let schema = format!("test_{}", Uuid::now_v7().simple());
    sqlx::query(&format!("create schema if not exists {schema}"))
        .execute(&admin_pool)
        .await
        .unwrap();
    format!("{}?options=-csearch_path%3D{schema}", base_database_url())
}

fn before_migrator() -> Migrator {
    let migrations = sqlx::migrate!("./migrations")
        .iter()
        .filter(|migration| migration.version < MIGRATION_VERSION)
        .cloned()
        .collect::<Vec<_>>();
    Migrator {
        migrations: Cow::Owned(migrations),
        ..Migrator::DEFAULT
    }
}

async fn seed_owner(store: &PgControlPlaneStore) -> (Uuid, Uuid) {
    let tenant_id: Uuid = sqlx::query_scalar("select id from tenants where code = 'root-tenant'")
        .fetch_one(store.pool())
        .await
        .unwrap();
    let workspace_id = Uuid::now_v7();
    sqlx::query(
        "insert into workspaces (id, tenant_id, name) values ($1, $2, 'Max output migration')",
    )
    .bind(workspace_id)
    .bind(tenant_id)
    .execute(store.pool())
    .await
    .unwrap();

    let user_id = Uuid::now_v7();
    let account = format!("max-output-{}", user_id.simple());
    sqlx::query(
        r#"
        insert into users (
            id, account, email, password_hash, name, nickname, introduction,
            default_display_role, email_login_enabled, phone_login_enabled, status,
            session_version
        ) values ($1, $2, $3, 'hash', $2, $2, '', 'member', true, false, 'active', 1)
        "#,
    )
    .bind(user_id)
    .bind(&account)
    .bind(format!("{account}@example.com"))
    .execute(store.pool())
    .await
    .unwrap();
    sqlx::query(
        "insert into workspace_memberships (id, workspace_id, user_id, introduction) values ($1, $2, $3, '')",
    )
    .bind(Uuid::now_v7())
    .bind(workspace_id)
    .bind(user_id)
    .execute(store.pool())
    .await
    .unwrap();
    (workspace_id, user_id)
}

async fn seed_flow(
    store: &PgControlPlaneStore,
    workspace_id: Uuid,
    user_id: Uuid,
    document: &serde_json::Value,
    plan: &serde_json::Value,
) -> (Uuid, Uuid, Uuid, Uuid) {
    let application_id = Uuid::now_v7();
    sqlx::query(
        r#"
        insert into applications (
            id, workspace_id, application_type, name, description, created_by, updated_by
        ) values ($1, $2, 'agent_flow', 'Max output migration', '', $3, $3)
        "#,
    )
    .bind(application_id)
    .bind(workspace_id)
    .bind(user_id)
    .execute(store.pool())
    .await
    .unwrap();

    let flow_id = Uuid::now_v7();
    let draft_id = Uuid::now_v7();
    let version_id = Uuid::now_v7();
    let compiled_plan_id = Uuid::now_v7();
    sqlx::query(
        "insert into flows (id, application_id, scope_id, created_by, updated_by) values ($1, $2, (select scope_id from applications where id = $2), $3, $3)",
    )
    .bind(flow_id)
    .bind(application_id)
    .bind(user_id)
    .execute(store.pool())
    .await
    .unwrap();
    sqlx::query(
        "insert into flow_drafts (id, flow_id, scope_id, schema_version, document, created_by, updated_by) values ($1, $2, (select scope_id from flows where id = $2), $3, $4, $5, $5)",
    )
    .bind(draft_id)
    .bind(flow_id)
    .bind(domain::FLOW_SCHEMA_VERSION)
    .bind(document)
    .bind(user_id)
    .execute(store.pool())
    .await
    .unwrap();
    sqlx::query(
        r#"
        insert into flow_versions (
            id, flow_id, scope_id, sequence, trigger, change_kind, summary,
            summary_is_custom, is_user_protected, document, created_by, updated_by
        ) values ($1, $2, (select scope_id from flows where id = $2), 1, 'autosave',
            'logical', 'fixture', false, true, $3, $4, $4)
        "#,
    )
    .bind(version_id)
    .bind(flow_id)
    .bind(document)
    .bind(user_id)
    .execute(store.pool())
    .await
    .unwrap();
    sqlx::query(
        r#"
        insert into flow_compiled_plans (
            id, flow_id, flow_draft_id, schema_version, document_hash,
            document_updated_at, plan, scope_id, created_by, updated_by
        ) select $1, $2, $3, $4, 'sha256:compiled', updated_at, $5,
            (select scope_id from flows where id = $2), $6, $6
          from flow_drafts where id = $3
        "#,
    )
    .bind(compiled_plan_id)
    .bind(flow_id)
    .bind(draft_id)
    .bind(domain::FLOW_SCHEMA_VERSION)
    .bind(plan)
    .bind(user_id)
    .execute(store.pool())
    .await
    .unwrap();
    (application_id, flow_id, version_id, compiled_plan_id)
}

fn legacy_item(enabled: bool, value: i64) -> serde_json::Value {
    serde_json::json!({
        "enabled": enabled,
        "value": value,
        "description": "preserve this sibling"
    })
}

fn document(first_item: serde_json::Value, second_item: serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "schema_version": domain::FLOW_SCHEMA_VERSION,
        "graph": {
            "viewport": {"x": 12, "y": 34},
            "nodes": [
                {
                    "id": "enabled-node",
                    "config": {"llm_parameters": {"items": {
                        "max_tokens": first_item,
                        "temperature": {"enabled": true, "value": 0.4}
                    }}}
                },
                {
                    "id": "disabled-node",
                    "config": {"llm_parameters": {"items": {
                        "max_tokens": second_item,
                        "top_p": {"enabled": false, "value": 0.9}
                    }}}
                }
            ],
            "edges": [{"id": "edge-sibling"}]
        }
    })
}

fn plan(first_item: serde_json::Value, second_item: serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "schema_version": domain::FLOW_SCHEMA_VERSION,
        "nodes": {
            "enabled-node": {"config": {"llm_parameters": {"items": {
                "max_tokens": first_item,
                "temperature": {"enabled": true, "value": 0.4}
            }}}},
            "disabled-node": {"config": {"llm_parameters": {"items": {
                "max_tokens": second_item,
                "top_p": {"enabled": false, "value": 0.9}
            }}}}
        },
        "execution_order": ["enabled-node", "disabled-node"]
    })
}

/// AC-003: all mutable persisted flow shapes rename the item without changing its payload or
/// siblings, publication snapshots remain immutable, and applying the SQL again is a no-op.
#[tokio::test]
async fn max_output_tokens_migration_preserves_items_snapshots_and_is_idempotent() {
    let pool = connect(&isolated_database_url().await).await.unwrap();
    before_migrator().run(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool.clone());
    let (workspace_id, user_id) = seed_owner(&store).await;
    let enabled_item = legacy_item(true, 4096);
    let disabled_item = legacy_item(false, 1024);
    let legacy_document = document(enabled_item.clone(), disabled_item.clone());
    let legacy_plan = plan(enabled_item.clone(), disabled_item.clone());
    let (application_id, flow_id, version_id, compiled_plan_id) = seed_flow(
        &store,
        workspace_id,
        user_id,
        &legacy_document,
        &legacy_plan,
    )
    .await;
    let publication_id = Uuid::now_v7();
    sqlx::query(
        r#"
        insert into application_publication_versions (
            id, application_id, scope_id, flow_id, flow_version_id, compiled_plan_id,
            extension_slug, version_sequence, active, api_enabled, flow_schema_version,
            document_hash, document_snapshot, mapping_snapshot, operation_bindings,
            runtime_profile_snapshot, output_selector, dependency_snapshot, created_by, updated_by
        ) values (
            $1, $2, (select scope_id from applications where id = $2), $3, $4, $5,
            'max-output-fixture', 1, true, true, $6, 'sha256:immutable', $7, '{}'::jsonb,
            '{}'::jsonb, '{}'::jsonb, '{}'::jsonb, '[]'::jsonb, $8, $8
        )
        "#,
    )
    .bind(publication_id)
    .bind(application_id)
    .bind(flow_id)
    .bind(version_id)
    .bind(compiled_plan_id)
    .bind(domain::FLOW_SCHEMA_VERSION)
    .bind(&legacy_document)
    .bind(user_id)
    .execute(&pool)
    .await
    .unwrap();

    sqlx::migrate!("./migrations").run(&pool).await.unwrap();

    let draft: serde_json::Value =
        sqlx::query_scalar("select document from flow_drafts where flow_id = $1")
            .bind(flow_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    let version: serde_json::Value =
        sqlx::query_scalar("select document from flow_versions where id = $1")
            .bind(version_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    let compiled: serde_json::Value =
        sqlx::query_scalar("select plan from flow_compiled_plans where id = $1")
            .bind(compiled_plan_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    let (snapshot, hash): (serde_json::Value, String) = sqlx::query_as(
        "select document_snapshot, document_hash from application_publication_versions where id = $1",
    )
    .bind(publication_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    for migrated in [&draft, &version] {
        assert_eq!(
            migrated["graph"]["nodes"][0]["config"]["llm_parameters"]["items"]["max_output_tokens"],
            enabled_item
        );
        assert_eq!(
            migrated["graph"]["nodes"][1]["config"]["llm_parameters"]["items"]["max_output_tokens"],
            disabled_item
        );
        assert!(
            migrated["graph"]["nodes"][0]["config"]["llm_parameters"]["items"]["max_tokens"]
                .is_null()
        );
        assert_eq!(
            migrated["graph"]["nodes"][0]["config"]["llm_parameters"]["items"]["temperature"],
            legacy_document["graph"]["nodes"][0]["config"]["llm_parameters"]["items"]
                ["temperature"]
        );
        assert_eq!(
            migrated["graph"]["nodes"][1]["config"]["llm_parameters"]["items"]["top_p"],
            legacy_document["graph"]["nodes"][1]["config"]["llm_parameters"]["items"]["top_p"]
        );
        assert_eq!(
            migrated["graph"]["edges"],
            legacy_document["graph"]["edges"]
        );
    }
    assert_eq!(
        compiled["nodes"]["enabled-node"]["config"]["llm_parameters"]["items"]["max_output_tokens"],
        enabled_item
    );
    assert_eq!(
        compiled["nodes"]["disabled-node"]["config"]["llm_parameters"]["items"]
            ["max_output_tokens"],
        disabled_item
    );
    assert_eq!(
        compiled["nodes"]["enabled-node"]["config"]["llm_parameters"]["items"]["temperature"],
        legacy_plan["nodes"]["enabled-node"]["config"]["llm_parameters"]["items"]["temperature"]
    );
    assert!(
        compiled["nodes"]["enabled-node"]["config"]["llm_parameters"]["items"]["max_tokens"]
            .is_null()
    );
    assert_eq!(compiled["execution_order"], legacy_plan["execution_order"]);
    assert_eq!(snapshot, legacy_document);
    assert_eq!(hash, "sha256:immutable");

    sqlx::raw_sql(MIGRATION_SQL).execute(&pool).await.unwrap();
    let after_second_run: serde_json::Value =
        sqlx::query_scalar("select plan from flow_compiled_plans where id = $1")
            .bind(compiled_plan_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(after_second_run, compiled);
}

/// AC-003: a dual-key item aborts the migration before any mutable record is rewritten.
#[tokio::test]
async fn max_output_tokens_migration_fails_closed_on_dual_keys() {
    let pool = connect(&isolated_database_url().await).await.unwrap();
    before_migrator().run(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool.clone());
    let (workspace_id, user_id) = seed_owner(&store).await;
    let legacy_item = legacy_item(true, 2048);
    let conflicting_item = serde_json::json!({
        "max_tokens": legacy_item,
        "max_output_tokens": {"enabled": true, "value": 8192},
        "temperature": {"enabled": true, "value": 0.2}
    });
    let clean_document = document(
        serde_json::json!({"enabled": true, "value": 2048}),
        serde_json::json!({"enabled": false, "value": 512}),
    );
    let conflicting_plan = serde_json::json!({
        "nodes": {"conflict": {"config": {"llm_parameters": {"items": conflicting_item}}}}
    });
    let (_, flow_id, _, compiled_plan_id) = seed_flow(
        &store,
        workspace_id,
        user_id,
        &clean_document,
        &conflicting_plan,
    )
    .await;

    let error = sqlx::migrate!("./migrations").run(&pool).await.unwrap_err();
    assert!(
        error.to_string().contains(
            "LLM max output token migration rejected an item containing both max_tokens and max_output_tokens"
        ),
        "unexpected migration error: {error}"
    );
    let unchanged_document: serde_json::Value =
        sqlx::query_scalar("select document from flow_drafts where flow_id = $1")
            .bind(flow_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    let unchanged_plan: serde_json::Value =
        sqlx::query_scalar("select plan from flow_compiled_plans where id = $1")
            .bind(compiled_plan_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(unchanged_document, clean_document);
    assert_eq!(unchanged_plan, conflicting_plan);
}
