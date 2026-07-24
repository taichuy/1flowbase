use control_plane::ports::{
    ApplicationRepository, ApplicationVisibility, CreateApplicationInput, DeleteApplicationInput,
};
use storage_postgres::{run_migrations, PgControlPlaneStore};
use uuid::Uuid;

fn base_database_url() -> String {
    std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:1flowbase@127.0.0.1:35432/1flowbase".into())
}

async fn isolated_database() -> postgres_test_support::PostgresTestSchema {
    postgres_test_support::PostgresTestSchema::create(&base_database_url())
        .await
        .unwrap()
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

#[tokio::test]
async fn list_applications_scopes_rows_by_workspace_and_owner() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let workspace_id = seed_workspace(&store, "Applications").await;
    let actor_user_id = seed_user(&store, workspace_id, "owner").await;
    let other_user_id = seed_user(&store, workspace_id, "other").await;

    let owned = <PgControlPlaneStore as ApplicationRepository>::create_application(
        &store,
        &CreateApplicationInput {
            actor_user_id,
            workspace_id,
            application_type: domain::ApplicationType::AgentFlow,
            workflow_trigger_type: None,
            workflow_trigger_config: None,
            name: "Owned App".into(),
            description: "owned".into(),
            icon: Some("RobotOutlined".into()),
            icon_type: Some("iconfont".into()),
            icon_background: Some("#E6F7F2".into()),
        },
    )
    .await
    .unwrap();

    <PgControlPlaneStore as ApplicationRepository>::create_application(
        &store,
        &CreateApplicationInput {
            workflow_trigger_config: None,
            actor_user_id: other_user_id,
            workspace_id,
            application_type: domain::ApplicationType::AgentFlow,
            workflow_trigger_type: None,
            name: "Foreign App".into(),
            description: "foreign".into(),
            icon: None,
            icon_type: None,
            icon_background: None,
        },
    )
    .await
    .unwrap();

    let own_only = <PgControlPlaneStore as ApplicationRepository>::list_applications(
        &store,
        workspace_id,
        actor_user_id,
        ApplicationVisibility::Own,
    )
    .await
    .unwrap();
    let all_rows = <PgControlPlaneStore as ApplicationRepository>::list_applications(
        &store,
        workspace_id,
        actor_user_id,
        ApplicationVisibility::All,
    )
    .await
    .unwrap();

    assert_eq!(own_only.len(), 1);
    assert_eq!(own_only[0].id, owned.id);
    assert_eq!(all_rows.len(), 2);
}

#[tokio::test]
async fn get_application_returns_section_hooks_with_null_runtime_targets() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let workspace_id = seed_workspace(&store, "Applications Detail").await;
    let actor_user_id = seed_user(&store, workspace_id, "detail").await;
    let created = <PgControlPlaneStore as ApplicationRepository>::create_application(
        &store,
        &CreateApplicationInput {
            actor_user_id,
            workspace_id,
            application_type: domain::ApplicationType::AgentFlow,
            workflow_trigger_type: None,
            workflow_trigger_config: None,
            name: "Detail App".into(),
            description: "detail".into(),
            icon: None,
            icon_type: None,
            icon_background: None,
        },
    )
    .await
    .unwrap();

    let detail = <PgControlPlaneStore as ApplicationRepository>::get_application(
        &store,
        workspace_id,
        created.id,
    )
    .await
    .unwrap()
    .unwrap();

    assert_eq!(
        detail.sections.api.invoke_routing_mode,
        "api_key_bound_application"
    );
    assert_eq!(
        detail.sections.api.invoke_path_template.as_deref(),
        Some("/api/agent/v1/runs")
    );
    assert_eq!(detail.sections.api.api_capability_status, "not_published");
    assert_eq!(detail.sections.api.credentials_status, "missing");
    assert_eq!(detail.sections.logs.status, "planned");
    assert_eq!(detail.sections.logs.runs_capability_status, "planned");
    assert_eq!(detail.sections.orchestration.current_draft_id, None);
}

#[tokio::test]
async fn create_workflow_application_persists_trigger_type() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let workspace_id = seed_workspace(&store, "Workflow Trigger Type").await;
    let actor_user_id = seed_user(&store, workspace_id, "workflow-trigger").await;
    let created = <PgControlPlaneStore as ApplicationRepository>::create_application(
        &store,
        &CreateApplicationInput {
            actor_user_id,
            workspace_id,
            application_type: domain::ApplicationType::Workflow,
            workflow_trigger_type: Some(domain::WorkflowTriggerType::Schedule),
            workflow_trigger_config: None,
            name: "Scheduled Workflow".into(),
            description: "scheduled".into(),
            icon: None,
            icon_type: None,
            icon_background: None,
        },
    )
    .await
    .unwrap();

    let detail = <PgControlPlaneStore as ApplicationRepository>::get_application(
        &store,
        workspace_id,
        created.id,
    )
    .await
    .unwrap()
    .unwrap();

    assert_eq!(
        detail.workflow_trigger_type,
        Some(domain::WorkflowTriggerType::Schedule)
    );
}

#[tokio::test]
async fn delete_application_cascades_flow_runtime_and_tag_bindings() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let workspace_id = seed_workspace(&store, "Applications Delete").await;
    let actor_user_id = seed_user(&store, workspace_id, "delete").await;
    let created = <PgControlPlaneStore as ApplicationRepository>::create_application(
        &store,
        &CreateApplicationInput {
            actor_user_id,
            workspace_id,
            application_type: domain::ApplicationType::AgentFlow,
            workflow_trigger_type: None,
            workflow_trigger_config: None,
            name: "Delete App".into(),
            description: "delete".into(),
            icon: None,
            icon_type: None,
            icon_background: None,
        },
    )
    .await
    .unwrap();

    let tag_id = Uuid::now_v7();
    sqlx::query(
        r#"
        insert into application_tags (id, workspace_id, name, normalized_name, created_by, updated_by)
        values ($1, $2, '客服', '客服', $3, $3)
        "#,
    )
    .bind(tag_id)
    .bind(workspace_id)
    .bind(actor_user_id)
    .execute(store.pool())
    .await
    .unwrap();
    sqlx::query(
        "insert into application_tag_bindings (id, scope_id, application_id, tag_id, created_by, updated_by) values ($1, (select scope_id from applications where id = $2), $2, $3, $4, $4)",
    )
    .bind(Uuid::now_v7())
    .bind(created.id)
    .bind(tag_id)
    .bind(actor_user_id)
    .execute(store.pool())
    .await
    .unwrap();

    let flow_id = Uuid::now_v7();
    let draft_id = Uuid::now_v7();
    let plan_id = Uuid::now_v7();
    let run_id = Uuid::now_v7();
    sqlx::query(
        "insert into flows (id, application_id, scope_id, created_by, updated_by) values ($1, $2, (select scope_id from applications where id = $2), $3, $3)",
    )
    .bind(flow_id)
    .bind(created.id)
    .bind(actor_user_id)
    .execute(store.pool())
    .await
    .unwrap();
    sqlx::query(
        r#"
        insert into flow_drafts (
            id, flow_id, scope_id, schema_version, document, created_by, updated_by
        ) values ($1, $2, (select scope_id from flows where id = $2), '1', '{}'::jsonb, $3, $3)
        "#,
    )
    .bind(draft_id)
    .bind(flow_id)
    .bind(actor_user_id)
    .execute(store.pool())
    .await
    .unwrap();
    sqlx::query(
        r#"
        insert into flow_compiled_plans (
            id, flow_id, flow_draft_id, schema_version, document_updated_at, plan, scope_id, created_by, updated_by
        ) values ($1, $2, $3, '1', now(), '{}'::jsonb, (select scope_id from flows where id = $2), $4, $4)
        "#,
    )
    .bind(plan_id)
    .bind(flow_id)
    .bind(draft_id)
    .bind(actor_user_id)
    .execute(store.pool())
    .await
    .unwrap();
    sqlx::query(
        r#"
        insert into flow_runs (
            id, application_id, flow_id, flow_draft_id, compiled_plan_id, run_mode, status,
            created_by
        ) values ($1, $2, $3, $4, $5, 'debug_node_preview', 'succeeded', $6)
        "#,
    )
    .bind(run_id)
    .bind(created.id)
    .bind(flow_id)
    .bind(draft_id)
    .bind(plan_id)
    .bind(actor_user_id)
    .execute(store.pool())
    .await
    .unwrap();

    <PgControlPlaneStore as ApplicationRepository>::delete_application(
        &store,
        &DeleteApplicationInput {
            actor_user_id,
            workspace_id,
            application_id: created.id,
        },
    )
    .await
    .unwrap();

    for table in [
        "applications",
        "application_tag_bindings",
        "flows",
        "flow_drafts",
        "flow_compiled_plans",
        "flow_runs",
    ] {
        let count: i64 = sqlx::query_scalar(&format!("select count(*)::bigint from {table}"))
            .fetch_one(store.pool())
            .await
            .unwrap();
        assert_eq!(count, 0, "{table} should be empty after application delete");
    }
}

#[tokio::test]
async fn workflow_trigger_type_migration_normalizes_and_rejects_manual() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool.clone());
    let workspace_id = seed_workspace(&store, "Workflow Trigger Constraint").await;
    let actor_user_id = seed_user(&store, workspace_id, "workflow-trigger-constraint").await;
    let created = <PgControlPlaneStore as ApplicationRepository>::create_application(
        &store,
        &CreateApplicationInput {
            actor_user_id,
            workspace_id,
            application_type: domain::ApplicationType::Workflow,
            workflow_trigger_type: Some(domain::WorkflowTriggerType::Extension),
            workflow_trigger_config: None,
            name: "Extension Workflow".into(),
            description: "extension".into(),
            icon: None,
            icon_type: None,
            icon_background: None,
        },
    )
    .await
    .unwrap();

    let error =
        sqlx::query("update applications set workflow_trigger_type = 'manual' where id = $1")
            .bind(created.id)
            .execute(&pool)
            .await
            .unwrap_err();

    assert!(error
        .to_string()
        .contains("applications_workflow_trigger_type_check"));
}
