use control_plane::{
    file_management::FileManagementBootstrapService,
    ports::{
        AddModelFieldInput, CreateFileStorageInput, CreateModelDefinitionInput,
        FileManagementRepository, ModelDefinitionRepository,
    },
    system_metadata::SystemMetadataBootstrapService,
};
use domain::{DataModelScopeKind, ModelFieldKind, DEFAULT_SCOPE_ID};
use runtime_core::runtime_engine::RuntimeModelError;
use runtime_core::runtime_record_repository::{
    RuntimeListQuery, RuntimeRecordRepository, RuntimeSortInput,
};
use serde_json::json;
use sqlx::PgPool;
use storage_postgres::{connect, run_migrations, PgControlPlaneStore};
use time::macros::datetime;
use uuid::Uuid;

fn base_database_url() -> String {
    std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:1flowbase@127.0.0.1:35432/1flowbase".into())
}

async fn isolated_database_url() -> String {
    let admin_pool = PgPool::connect(&base_database_url()).await.unwrap();
    let schema = format!("test_{}", Uuid::now_v7().to_string().replace('-', ""));
    sqlx::query(&format!("create schema if not exists {schema}"))
        .execute(&admin_pool)
        .await
        .unwrap();

    format!("{}?options=-csearch_path%3D{schema}", base_database_url())
}

async fn root_tenant_id(store: &PgControlPlaneStore) -> Uuid {
    sqlx::query_scalar("select id from tenants where code = 'root-tenant'")
        .fetch_one(store.pool())
        .await
        .unwrap()
}

async fn insert_user(store: &PgControlPlaneStore, user_id: Uuid, account: &str) {
    let unique_account = format!("{account}-{}", user_id.simple());
    sqlx::query(
        r#"
        insert into users (
            id, account, email, phone, password_hash, name, nickname, avatar_url, introduction,
            default_display_role, email_login_enabled, phone_login_enabled, status, session_version,
            created_by, updated_by
        )
        values (
            $1, $2, $3, null, 'hash', $4, $5, null, '', 'manager', true, false, 'active', 1, null, null
        )
        "#,
    )
    .bind(user_id)
    .bind(&unique_account)
    .bind(format!("{unique_account}@example.com"))
    .bind(&unique_account)
    .bind(&unique_account)
    .execute(store.pool())
    .await
    .unwrap();
}

async fn insert_workspace(store: &PgControlPlaneStore, workspace_id: Uuid) {
    let tenant_id = root_tenant_id(store).await;
    let workspace_name = format!("Core Workspace {}", workspace_id.simple());
    sqlx::query(
        "insert into workspaces (id, tenant_id, name, created_by, updated_by) values ($1, $2, $3, null, null)",
    )
    .bind(workspace_id)
    .bind(tenant_id)
    .bind(&workspace_name)
    .execute(store.pool())
    .await
    .unwrap();
}

struct RuntimeReadModelSeed {
    workspace_id: Uuid,
    application_id: Uuid,
    flow_run_id: Uuid,
    node_run_id: Uuid,
}

async fn seed_runtime_read_model_rows(store: &PgControlPlaneStore) -> RuntimeReadModelSeed {
    let workspace_id = Uuid::now_v7();
    let user_id = Uuid::now_v7();
    let application_id = Uuid::now_v7();
    let flow_id = Uuid::now_v7();
    let draft_id = Uuid::now_v7();
    let compiled_plan_id = Uuid::now_v7();
    let flow_run_id = Uuid::now_v7();
    let node_run_id = Uuid::now_v7();
    let started_at = datetime!(2026-05-29 08:00:00 UTC);

    insert_workspace(store, workspace_id).await;
    insert_user(store, user_id, "runtime-read-model").await;

    sqlx::query(
        r#"
        insert into applications (
            id, workspace_id, application_type, name, description, created_by, updated_by
        ) values ($1, $2, 'agent_flow', 'Runtime Read Model App', '', $3, $3)
        "#,
    )
    .bind(application_id)
    .bind(workspace_id)
    .bind(user_id)
    .execute(store.pool())
    .await
    .unwrap();
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
        r#"
        insert into flow_drafts (
            id, flow_id, scope_id, schema_version, document, created_by, updated_by
        ) values ($1, $2, (select scope_id from flows where id = $2), '1flowbase.flow/v2', '{}', $3, $3)
        "#,
    )
    .bind(draft_id)
    .bind(flow_id)
    .bind(user_id)
    .execute(store.pool())
    .await
    .unwrap();
    sqlx::query(
        r#"
        insert into flow_compiled_plans (
            id, flow_id, flow_draft_id, schema_version, document_hash,
            document_updated_at, plan, scope_id, created_by, updated_by
        ) values ($1, $2, $3, '1flowbase.flow/v2', 'hash', $4, '{}', (select scope_id from flows where id = $2), $5, $5)
        "#,
    )
    .bind(compiled_plan_id)
    .bind(flow_id)
    .bind(draft_id)
    .bind(started_at)
    .bind(user_id)
    .execute(store.pool())
    .await
    .unwrap();
    sqlx::query(
        r#"
        insert into flow_runs (
            id, application_id, flow_id, flow_draft_id, compiled_plan_id,
            debug_session_id, flow_schema_version, document_hash, run_mode,
            title, status, input_payload, output_payload, created_by,
            started_at, finished_at, created_at, updated_at
        ) values (
            $1, $2, $3, $4, $5, 'runtime-read-model', '1flowbase.flow/v2',
            'hash', 'debug_flow_run', 'Alpha refund run', 'succeeded',
            '{"query":"refund"}', '{"answer":"done"}', $6, $7, $8, $7, $8
        )
        "#,
    )
    .bind(flow_run_id)
    .bind(application_id)
    .bind(flow_id)
    .bind(draft_id)
    .bind(compiled_plan_id)
    .bind(user_id)
    .bind(started_at)
    .bind(started_at + time::Duration::seconds(2))
    .execute(store.pool())
    .await
    .unwrap();
    sqlx::query(
        r#"
        insert into application_run_log_summaries (
            flow_run_id, scope_id, application_id, run_mode, status, title,
            input_payload, unique_node_count, tool_callback_count,
            started_at, finished_at, created_at, updated_at
        ) values (
            $1, $2, $3, 'debug_flow_run', 'succeeded', 'Alpha refund run',
            '{}', 1, 1, $4, $5, $4, $5
        )
        "#,
    )
    .bind(flow_run_id)
    .bind(workspace_id)
    .bind(application_id)
    .bind(started_at)
    .bind(started_at + time::Duration::seconds(2))
    .execute(store.pool())
    .await
    .unwrap();
    sqlx::query(
        r#"
        insert into node_runs (
            id, scope_id, flow_run_id, node_id, node_type, node_alias, status,
            input_payload, output_payload, metrics_payload, debug_payload, started_at, created_at
        ) values (
            $1, $2, $3, 'node-llm', 'llm', 'LLM', 'succeeded',
            '{"large":"input"}', '{"large":"output"}', '{"tokens":12}',
            '{"large":"debug"}', $4, $4
        )
        "#,
    )
    .bind(node_run_id)
    .bind(workspace_id)
    .bind(flow_run_id)
    .bind(started_at)
    .execute(store.pool())
    .await
    .unwrap();
    sqlx::query(
        r#"
        insert into flow_run_events (
            id, scope_id, flow_run_id, node_run_id, sequence, event_type, payload, created_at
        ) values (
            $1, $2, $3, $4, 1, 'node_run_completed', '{"large":"event"}', $5
        )
        "#,
    )
    .bind(Uuid::now_v7())
    .bind(workspace_id)
    .bind(flow_run_id)
    .bind(node_run_id)
    .bind(started_at)
    .execute(store.pool())
    .await
    .unwrap();
    sqlx::query(
        r#"
        insert into flow_run_checkpoints (
            id, scope_id, flow_run_id, node_run_id, status, reason,
            locator_payload, variable_snapshot, external_ref_payload, created_at
        ) values (
            $1, $2, $3, $4, 'waiting_human', 'review',
            '{"large":"locator"}', '{"large":"variables"}', '{"large":"external"}', $5
        )
        "#,
    )
    .bind(Uuid::now_v7())
    .bind(workspace_id)
    .bind(flow_run_id)
    .bind(node_run_id)
    .bind(started_at)
    .execute(store.pool())
    .await
    .unwrap();
    sqlx::query(
        r#"
        insert into flow_run_callback_tasks (
            id, scope_id, flow_run_id, node_run_id, callback_kind, status,
            request_payload, external_ref_payload, created_at
        ) values (
            $1, $2, $3, $4, 'tool', 'pending',
            '{"large":"request"}', '{"large":"external"}', $5
        )
        "#,
    )
    .bind(Uuid::now_v7())
    .bind(workspace_id)
    .bind(flow_run_id)
    .bind(node_run_id)
    .bind(started_at)
    .execute(store.pool())
    .await
    .unwrap();

    RuntimeReadModelSeed {
        workspace_id,
        application_id,
        flow_run_id,
        node_run_id,
    }
}

mod crud;
mod read_models;
mod scopes;
