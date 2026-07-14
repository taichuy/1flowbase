use control_plane::{
    errors::ControlPlaneError,
    ports::{
        AppendCreditLedgerInput, AppendModelFailoverAttemptLedgerInput, AppendRunEventInput,
        AppendRuntimeEventInput, AppendRuntimeSpanInput, AppendUsageLedgerInput,
        ApplicationRepository, AttachCompiledPlanToFlowRunInput,
        ClearModelProviderRequestLogsBatchInput, CompleteNodeRunInput, CreateApplicationInput,
        CreateCallbackTaskInput, CreateCheckpointInput, CreateFlowRunInput,
        CreateFlowRunShellInput, CreateNodeRunInput, CreateRuntimeDebugArtifactInput,
        DeleteModelProviderRequestLogsInput, FinishFlowRunCallbackResumeAttemptInput,
        FlowRepository, GetApplicationRunMonitoringReportInput, GetRuntimeDebugArtifactInput,
        LinkUsageLedgerToModelFailoverAttemptInput, ListApplicationConversationRunsPageInput,
        ListApplicationRunConversationMessageItemsPageInput, ListApplicationRunsPageInput,
        ListModelProviderRequestLogsPageInput, OrchestrationRuntimeRepository,
        ProviderRequestLogTask, RecordFlowRunCallbackResumeAttemptInput, UpdateFlowRunInput,
        UpdateFlowRunPayloadsInput, UpdateNodeRunInput, UpdateNodeRunPayloadsInput,
        UpdateRunEventPayloadInput, UpsertCompiledPlanInput, UpsertDataModelSideEffectReceiptInput,
    },
};
use domain::{
    ApplicationType, CallbackTaskStatus, FlowRunCallbackResumeAttemptStatus, FlowRunMode,
    FlowRunStatus, NodeRunStatus,
};
use serde_json::json;
use sqlx::PgPool;
use std::sync::Arc;
use storage_postgres::{connect, run_migrations, PgControlPlaneStore};
use time::{macros::datetime, Duration, OffsetDateTime};
use tokio::sync::Barrier;
use uuid::Uuid;

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

async fn root_tenant_id(store: &PgControlPlaneStore) -> Uuid {
    sqlx::query_scalar("select id from tenants where code = 'root-tenant'")
        .fetch_one(store.pool())
        .await
        .unwrap()
}

async fn seed_workspace(store: &PgControlPlaneStore, name: &str) -> Uuid {
    let workspace_id = Uuid::now_v7();
    sqlx::query(
        "insert into workspaces (id, tenant_id, name, created_by, updated_by) values ($1, $2, $3, null, null)",
    )
    .bind(workspace_id)
    .bind(root_tenant_id(store).await)
    .bind(name)
    .execute(store.pool())
    .await
    .unwrap();
    workspace_id
}

async fn seed_user(store: &PgControlPlaneStore, workspace_id: Uuid, account_prefix: &str) -> Uuid {
    let user_id = Uuid::now_v7();
    let account = format!("{account_prefix}-{}", user_id.simple());
    sqlx::query(
        r#"
        insert into users (
            id, account, email, phone, password_hash, name, nickname, avatar_url, introduction,
            default_display_role, email_login_enabled, phone_login_enabled, status, session_version,
            created_by, updated_by
        ) values (
            $1, $2, $3, null, 'hash', $4, $5, null, '', 'member', true, false, 'active', 1, null, null
        )
        "#,
    )
    .bind(user_id)
    .bind(&account)
    .bind(format!("{account}@example.com"))
    .bind(&account)
    .bind(&account)
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

    user_id
}

#[derive(Debug, Clone)]
struct RuntimeSeedState {
    workspace_id: Uuid,
    application_id: Uuid,
    actor_user_id: Uuid,
    flow_id: Uuid,
    draft_id: Uuid,
    draft_updated_at: OffsetDateTime,
}

async fn seed_runtime_base(store: &PgControlPlaneStore) -> RuntimeSeedState {
    seed_runtime_base_with_workspace_name(store, "Runtime").await
}

async fn seed_runtime_base_with_workspace_name(
    store: &PgControlPlaneStore,
    workspace_name: &str,
) -> RuntimeSeedState {
    let workspace_id = seed_workspace(store, workspace_name).await;
    let actor_user_id = seed_user(store, workspace_id, "runtime-owner").await;
    let application = <PgControlPlaneStore as ApplicationRepository>::create_application(
        store,
        &CreateApplicationInput {
            actor_user_id,
            workspace_id,
            application_type: ApplicationType::AgentFlow,
            workflow_trigger_type: None,
            name: "Runtime App".into(),
            description: "runtime".into(),
            icon: None,
            icon_type: None,
            icon_background: None,
        },
    )
    .await
    .unwrap();
    let editor_state = <PgControlPlaneStore as FlowRepository>::get_or_create_editor_state(
        store,
        workspace_id,
        application.id,
        actor_user_id,
    )
    .await
    .unwrap();

    RuntimeSeedState {
        workspace_id,
        application_id: application.id,
        actor_user_id,
        flow_id: editor_state.flow.id,
        draft_id: editor_state.draft.id,
        draft_updated_at: editor_state.draft.updated_at,
    }
}

async fn seed_compiled_plan(
    store: &PgControlPlaneStore,
    seeded: &RuntimeSeedState,
) -> domain::CompiledPlanRecord {
    <PgControlPlaneStore as OrchestrationRuntimeRepository>::upsert_compiled_plan(
        store,
        &UpsertCompiledPlanInput {
            actor_user_id: seeded.actor_user_id,
            flow_id: seeded.flow_id,
            flow_draft_id: seeded.draft_id,
            schema_version: "1flowbase.flow/v2".into(),
            document_hash: "test-document-hash".to_string(),
            document_updated_at: seeded.draft_updated_at,
            plan: json!({
                "schema_version": "1flowbase.flow/v2",
                "topological_order": ["node-start", "node-llm"]
            }),
        },
    )
    .await
    .unwrap()
}

async fn seed_flow_run(
    store: &PgControlPlaneStore,
    seeded: &RuntimeSeedState,
    compiled: &domain::CompiledPlanRecord,
    started_at: OffsetDateTime,
) -> domain::FlowRunRecord {
    seed_flow_run_with_mode(
        store,
        seeded,
        compiled,
        started_at,
        FlowRunMode::DebugNodePreview,
        Some("node-llm".into()),
    )
    .await
}

async fn seed_flow_run_with_mode(
    store: &PgControlPlaneStore,
    seeded: &RuntimeSeedState,
    compiled: &domain::CompiledPlanRecord,
    started_at: OffsetDateTime,
    run_mode: FlowRunMode,
    target_node_id: Option<String>,
) -> domain::FlowRunRecord {
    <PgControlPlaneStore as OrchestrationRuntimeRepository>::create_flow_run(
        store,
        &CreateFlowRunInput {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            flow_id: seeded.flow_id,
            flow_draft_id: seeded.draft_id,
            compiled_plan_id: compiled.id,
            debug_session_id: "test-debug-session".to_string(),
            flow_schema_version: compiled.schema_version.clone(),
            document_hash: "test-document-hash".to_string(),
            run_mode,
            target_node_id,
            title: "总结退款政策".to_string(),
            status: FlowRunStatus::Running,
            input_payload: json!({ "node-start": { "query": "总结退款政策" } }),
            started_at,
            api_key_id: None,
            publication_version_id: None,
            external_user: None,
            external_conversation_id: None,
            external_trace_id: None,
            compatibility_mode: None,
            idempotency_key: None,
        },
    )
    .await
    .unwrap()
}

async fn seed_application_api_key(store: &PgControlPlaneStore, seeded: &RuntimeSeedState) -> Uuid {
    let api_key_id = Uuid::now_v7();
    sqlx::query(
        r#"
        insert into api_keys (
            id,
            name,
            token_hash,
            token_prefix,
            creator_user_id,
            tenant_id,
            scope_kind,
            scope_id,
            key_kind,
            application_id
        ) values ($1, $2, $3, $4, $5, $6, 'workspace', $7, 'application_api_key', $8)
        "#,
    )
    .bind(api_key_id)
    .bind("application api key")
    .bind(format!("hash-{}", api_key_id.simple()))
    .bind("fb_test")
    .bind(seeded.actor_user_id)
    .bind(root_tenant_id(store).await)
    .bind(seeded.workspace_id)
    .bind(seeded.application_id)
    .execute(store.pool())
    .await
    .unwrap();

    api_key_id
}

async fn upsert_terminal_summary_tokens(
    store: &PgControlPlaneStore,
    flow_run_id: Uuid,
    total_tokens: i64,
) {
    sqlx::query(
        r#"
        insert into application_run_log_summaries (
            flow_run_id,
            scope_id,
            application_id,
            run_mode,
            status,
            target_node_id,
            title,
            input_payload,
            total_tokens,
            unique_node_count,
            tool_callback_count,
            started_at,
            finished_at,
            created_at,
            updated_at
        )
        select
            flow_runs.id,
            applications.workspace_id,
            flow_runs.application_id,
            flow_runs.run_mode,
            'succeeded',
            flow_runs.target_node_id,
            flow_runs.title,
            '{}'::jsonb,
            $2,
            0,
            0,
            flow_runs.started_at,
            flow_runs.started_at + interval '1 second',
            flow_runs.created_at,
            flow_runs.started_at + interval '1 second'
        from flow_runs
        join applications on applications.id = flow_runs.application_id
        where flow_runs.id = $1
        on conflict (flow_run_id) do update
        set status = excluded.status,
            total_tokens = excluded.total_tokens,
            finished_at = excluded.finished_at,
            updated_at = excluded.updated_at
        "#,
    )
    .bind(flow_run_id)
    .bind(total_tokens)
    .execute(store.pool())
    .await
    .unwrap();
}

async fn seed_node_run(
    store: &PgControlPlaneStore,
    flow_run: &domain::FlowRunRecord,
    started_at: OffsetDateTime,
) -> domain::NodeRunRecord {
    seed_node_run_for(
        store,
        flow_run,
        "node-llm",
        "llm",
        "LLM",
        json!({ "prompt_messages": ["总结退款政策"] }),
        started_at,
    )
    .await
}

async fn seed_node_run_for(
    store: &PgControlPlaneStore,
    flow_run: &domain::FlowRunRecord,
    node_id: &str,
    node_type: &str,
    node_alias: &str,
    input_payload: serde_json::Value,
    started_at: OffsetDateTime,
) -> domain::NodeRunRecord {
    <PgControlPlaneStore as OrchestrationRuntimeRepository>::create_node_run(
        store,
        &CreateNodeRunInput {
            flow_run_id: flow_run.id,
            node_id: node_id.into(),
            node_type: node_type.into(),
            node_alias: node_alias.into(),
            status: NodeRunStatus::Running,
            input_payload,
            debug_payload: json!({}),
            started_at,
        },
    )
    .await
    .unwrap()
}

mod anthropic_internal_runs;
mod application_logs;
mod application_trace_projection;
mod debug_artifacts;
mod flow_runs;
mod public_runs;
mod request_logs;
mod runtime_events;
mod runtime_facts;
