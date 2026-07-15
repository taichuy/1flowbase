use std::sync::{Arc, OnceLock};

use control_plane::{
    errors::ControlPlaneError,
    ports::{ApplicationRepository, CreateApplicationInput, FlowRepository},
};
use domain::{ApplicationType, FlowChangeKind, FlowVersionTrigger};
use serde_json::json;
use sqlx::{Connection, PgConnection};
use storage_postgres::{connect, run_migrations, PgControlPlaneStore};
use tokio::sync::Semaphore;
use uuid::Uuid;

fn repository_test_semaphore() -> Arc<Semaphore> {
    static SEMAPHORE: OnceLock<Arc<Semaphore>> = OnceLock::new();
    SEMAPHORE
        .get_or_init(|| Arc::new(Semaphore::new(1)))
        .clone()
}

fn base_database_url() -> String {
    std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:1flowbase@127.0.0.1:35432/1flowbase".into())
}

async fn isolated_database_url() -> String {
    let mut admin_connection = PgConnection::connect(&base_database_url()).await.unwrap();
    let schema = format!("test_{}", Uuid::now_v7().simple());
    sqlx::query(&format!("create schema if not exists {schema}"))
        .execute(&mut admin_connection)
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

async fn seed_agent_flow_application(
    store: &PgControlPlaneStore,
    workspace_id: Uuid,
    actor_user_id: Uuid,
) -> domain::ApplicationRecord {
    <PgControlPlaneStore as ApplicationRepository>::create_application(
        store,
        &CreateApplicationInput {
            actor_user_id,
            workspace_id,
            application_type: ApplicationType::AgentFlow,
            workflow_trigger_type: None,
            workflow_trigger_config: None,
            name: "Support Agent".into(),
            description: "customer support".into(),
            icon: None,
            icon_type: None,
            icon_background: None,
        },
    )
    .await
    .unwrap()
}

#[tokio::test]
async fn get_or_create_editor_state_bootstraps_default_draft_and_first_version() {
    let _permit = repository_test_semaphore().acquire_owned().await.unwrap();
    let pool = connect(&isolated_database_url().await).await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let workspace_id = seed_workspace(&store, "Flow Workspace").await;
    let actor_user_id = seed_user(&store, workspace_id, "flow-owner").await;
    let application = seed_agent_flow_application(&store, workspace_id, actor_user_id).await;

    let state = <PgControlPlaneStore as FlowRepository>::get_or_create_editor_state(
        &store,
        workspace_id,
        application.id,
        actor_user_id,
    )
    .await
    .unwrap();

    assert_eq!(
        state.draft.document["graph"]["nodes"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
    let start_node = state.draft.document["graph"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["type"] == "start")
        .expect("default draft should include a start node");
    assert_eq!(start_node["outputs"], json!([]));
    assert_eq!(start_node["config"]["input_fields"], json!([]));
    assert_eq!(state.versions.len(), 1);
    assert_eq!(state.versions[0].trigger, FlowVersionTrigger::Autosave);
    let draft_scope: (Uuid, Uuid) =
        sqlx::query_as("select scope_id, created_by from flow_drafts where id = $1")
            .bind(state.draft.id)
            .fetch_one(store.pool())
            .await
            .unwrap();
    assert_eq!(draft_scope, (workspace_id, actor_user_id));

    let detail = <PgControlPlaneStore as ApplicationRepository>::get_application(
        &store,
        workspace_id,
        application.id,
    )
    .await
    .unwrap()
    .unwrap();

    assert_eq!(detail.sections.orchestration.status, "ready");
    assert_eq!(
        detail.sections.orchestration.current_subject_id,
        Some(state.flow.id)
    );
    assert_eq!(
        detail.sections.orchestration.current_draft_id,
        Some(state.draft.id)
    );
    assert_eq!(detail.sections.logs.status, "ready");
    assert_eq!(detail.sections.logs.runs_capability_status, "queryable");
}

#[tokio::test]
async fn save_draft_only_appends_history_for_logical_changes() {
    let _permit = repository_test_semaphore().acquire_owned().await.unwrap();
    let pool = connect(&isolated_database_url().await).await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let workspace_id = seed_workspace(&store, "Flow Workspace").await;
    let actor_user_id = seed_user(&store, workspace_id, "flow-owner").await;
    let application = seed_agent_flow_application(&store, workspace_id, actor_user_id).await;
    let initial = <PgControlPlaneStore as FlowRepository>::get_or_create_editor_state(
        &store,
        workspace_id,
        application.id,
        actor_user_id,
    )
    .await
    .unwrap();

    let mut layout_only = initial.draft.document.clone();
    layout_only["editor"]["viewport"] = json!({ "x": 240, "y": 32, "zoom": 0.8 });

    let layout_state = <PgControlPlaneStore as FlowRepository>::save_draft(
        &store,
        workspace_id,
        application.id,
        actor_user_id,
        layout_only,
        FlowChangeKind::Layout,
        "viewport update",
    )
    .await
    .unwrap();

    assert_eq!(layout_state.versions.len(), 1);

    let mut logical_change = layout_state.draft.document.clone();
    logical_change["graph"]["nodes"][1]["bindings"]["prompt_messages"]["value"][0]["content"]
        ["value"] = json!("You are a support agent.");

    let logical_state = <PgControlPlaneStore as FlowRepository>::save_draft(
        &store,
        workspace_id,
        application.id,
        actor_user_id,
        logical_change,
        FlowChangeKind::Logical,
        "update llm prompt",
    )
    .await
    .unwrap();

    assert_eq!(logical_state.versions.len(), 2);
    assert_eq!(logical_state.versions[1].summary, "update llm prompt");
    assert_eq!(
        logical_state.versions[1].change_kind,
        FlowChangeKind::Logical
    );

    let protected_version_id = logical_state.versions[0].id;
    let protected_state = <PgControlPlaneStore as FlowRepository>::update_version_metadata(
        &store,
        workspace_id,
        application.id,
        actor_user_id,
        protected_version_id,
        Some("stable baseline".to_string()),
        Some(true),
        Some(true),
    )
    .await
    .unwrap();

    assert_eq!(protected_state.versions[0].summary, "stable baseline");
    assert!(protected_state.versions[0].summary_is_custom);
    assert!(protected_state.versions[0].is_user_protected);

    let mut current_document = protected_state.draft.document.clone();
    let mut current_state = protected_state;

    for index in 0..=domain::FLOW_HISTORY_LIMIT {
        current_document["graph"]["nodes"][1]["bindings"]["prompt_messages"]["value"][0]
            ["content"]["value"] = json!(format!("Prompt {index}"));
        current_state = <PgControlPlaneStore as FlowRepository>::save_draft(
            &store,
            workspace_id,
            application.id,
            actor_user_id,
            current_document.clone(),
            FlowChangeKind::Logical,
            &format!("update {index}"),
        )
        .await
        .unwrap();
    }

    assert!(current_state
        .versions
        .iter()
        .any(|version| version.id == protected_version_id && version.is_user_protected));
    assert!(
        current_state
            .versions
            .iter()
            .filter(|version| !version.is_user_protected)
            .count()
            <= domain::FLOW_HISTORY_LIMIT
    );
}

#[tokio::test]
async fn update_version_metadata_rejects_an_eleventh_user_protected_version() {
    let _permit = repository_test_semaphore().acquire_owned().await.unwrap();
    let pool = connect(&isolated_database_url().await).await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let workspace_id = seed_workspace(&store, "Flow Protection Limit Workspace").await;
    let actor_user_id = seed_user(&store, workspace_id, "flow-protection-owner").await;
    let application = seed_agent_flow_application(&store, workspace_id, actor_user_id).await;
    let mut state = <PgControlPlaneStore as FlowRepository>::get_or_create_editor_state(
        &store,
        workspace_id,
        application.id,
        actor_user_id,
    )
    .await
    .unwrap();
    let mut document = state.draft.document.clone();

    while state.versions.len() <= domain::FLOW_USER_PROTECTION_LIMIT {
        let sequence = state.versions.len();
        document["graph"]["nodes"][1]["bindings"]["prompt_messages"]["value"][0]["content"]
            ["value"] = json!(format!("Protected prompt {sequence}"));
        state = <PgControlPlaneStore as FlowRepository>::save_draft(
            &store,
            workspace_id,
            application.id,
            actor_user_id,
            document.clone(),
            FlowChangeKind::Logical,
            &format!("protected update {sequence}"),
        )
        .await
        .unwrap();
    }

    let user_protected_version_ids = state
        .versions
        .iter()
        .take(domain::FLOW_USER_PROTECTION_LIMIT)
        .map(|version| version.id)
        .collect::<Vec<_>>();
    for version_id in user_protected_version_ids {
        state = <PgControlPlaneStore as FlowRepository>::update_version_metadata(
            &store,
            workspace_id,
            application.id,
            actor_user_id,
            version_id,
            None,
            None,
            Some(true),
        )
        .await
        .unwrap();
    }

    let unprotected_version = state
        .versions
        .iter()
        .find(|version| !version.is_user_protected)
        .unwrap();
    let error = <PgControlPlaneStore as FlowRepository>::update_version_metadata(
        &store,
        workspace_id,
        application.id,
        actor_user_id,
        unprotected_version.id,
        None,
        None,
        Some(true),
    )
    .await
    .unwrap_err();

    assert_eq!(
        error.downcast_ref::<ControlPlaneError>(),
        Some(&ControlPlaneError::Conflict("flow_user_protection_limit"))
    );
}

#[tokio::test]
async fn save_draft_trim_keeps_current_publication_flow_version() {
    let _permit = repository_test_semaphore().acquire_owned().await.unwrap();
    let pool = connect(&isolated_database_url().await).await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool.clone());
    let workspace_id = seed_workspace(&store, "Flow Publication Workspace").await;
    let actor_user_id = seed_user(&store, workspace_id, "flow-publication-owner").await;
    let application = seed_agent_flow_application(&store, workspace_id, actor_user_id).await;
    let initial = <PgControlPlaneStore as FlowRepository>::get_or_create_editor_state(
        &store,
        workspace_id,
        application.id,
        actor_user_id,
    )
    .await
    .unwrap();
    let published_version_id = initial.versions[0].id;
    let compiled_plan_id = Uuid::now_v7();
    let publication_id = Uuid::now_v7();

    sqlx::query(
        r#"
        insert into flow_compiled_plans (
            id, flow_id, flow_draft_id, schema_version, document_hash,
            document_updated_at, plan, scope_id, created_by, updated_by
        )
        select $1, $2, $3, $4, 'sha256:published', updated_at, $5, (select scope_id from flows where id = $2), $6, $6
        from flow_drafts
        where id = $3
        "#,
    )
    .bind(compiled_plan_id)
    .bind(initial.flow.id)
    .bind(initial.draft.id)
    .bind(domain::FLOW_SCHEMA_VERSION)
    .bind(json!({"schema_version": domain::FLOW_SCHEMA_VERSION}))
    .bind(actor_user_id)
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        r#"
        insert into application_publication_versions (
            id,
            application_id,
            scope_id,
            flow_id,
            flow_version_id,
            compiled_plan_id,
            version_sequence,
            active,
            api_enabled,
            flow_schema_version,
            document_hash,
            document_snapshot,
            mapping_snapshot,
            runtime_profile_snapshot,
            output_selector,
            dependency_snapshot,
            created_by,
            updated_by
        ) values (
            $1, $2, (select scope_id from applications where id = $2), $3, $4, $5, 1, true, true, $6, 'sha256:published',
            $7, $8, '{}', '{}', '[]', $9, $9
        )
        "#,
    )
    .bind(publication_id)
    .bind(application.id)
    .bind(initial.flow.id)
    .bind(published_version_id)
    .bind(compiled_plan_id)
    .bind(domain::FLOW_SCHEMA_VERSION)
    .bind(&initial.draft.document)
    .bind(json!({
        "input": {
            "query_target": "start.query",
            "model_target": null,
            "inputs_target": null,
            "history_target": null,
            "attachments_target": null
        },
        "output": {
            "answer_selector": null,
            "usage_selector": null,
            "files_selector": null,
            "error_selector": null
        }
    }))
    .bind(actor_user_id)
    .execute(&pool)
    .await
    .unwrap();

    let mut current_document = initial.draft.document.clone();
    let mut current_state = initial;

    for index in 0..=domain::FLOW_HISTORY_LIMIT {
        current_document["graph"]["nodes"][1]["bindings"]["prompt_messages"]["value"][0]
            ["content"]["value"] = json!(format!("Prompt {index}"));
        current_state = <PgControlPlaneStore as FlowRepository>::save_draft(
            &store,
            workspace_id,
            application.id,
            actor_user_id,
            current_document.clone(),
            FlowChangeKind::Logical,
            &format!("update {index}"),
        )
        .await
        .unwrap();
    }

    assert!(current_state.versions.iter().any(|version| {
        version.id == published_version_id
            && version.is_current_publication
            && !version.is_user_protected
    }));
    assert!(
        current_state
            .versions
            .iter()
            .filter(|version| version.id != published_version_id && !version.is_user_protected)
            .count()
            <= domain::FLOW_HISTORY_LIMIT
    );
}

#[tokio::test]
async fn restore_version_replaces_current_draft_and_appends_restore_history() {
    let _permit = repository_test_semaphore().acquire_owned().await.unwrap();
    let pool = connect(&isolated_database_url().await).await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let workspace_id = seed_workspace(&store, "Flow Workspace").await;
    let actor_user_id = seed_user(&store, workspace_id, "flow-owner").await;
    let application = seed_agent_flow_application(&store, workspace_id, actor_user_id).await;
    let initial = <PgControlPlaneStore as FlowRepository>::get_or_create_editor_state(
        &store,
        workspace_id,
        application.id,
        actor_user_id,
    )
    .await
    .unwrap();

    let mut logical_change = initial.draft.document.clone();
    logical_change["graph"]["nodes"][1]["bindings"]["prompt_messages"]["value"][0]["content"]
        ["value"] = json!("You are a support agent.");
    let updated = <PgControlPlaneStore as FlowRepository>::save_draft(
        &store,
        workspace_id,
        application.id,
        actor_user_id,
        logical_change,
        FlowChangeKind::Logical,
        "update llm prompt",
    )
    .await
    .unwrap();

    let restored = <PgControlPlaneStore as FlowRepository>::restore_version(
        &store,
        workspace_id,
        application.id,
        actor_user_id,
        updated.versions[0].id,
    )
    .await
    .unwrap();

    assert_eq!(restored.draft.document, initial.draft.document);
    assert_eq!(restored.versions.len(), 3);
    assert_eq!(
        restored.versions.last().unwrap().trigger,
        FlowVersionTrigger::Restore
    );
}
