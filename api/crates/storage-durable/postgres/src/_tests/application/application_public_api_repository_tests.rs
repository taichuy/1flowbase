use control_plane::{
    application_public_api::{
        mapping::{
            ApplicationApiMappingConfig, ApplicationApiMappingDraft, ApplicationApiMappingInput,
            ApplicationApiMappingOutput, WorkflowExtensionApiConfig, WorkflowExtensionHttpMethod,
            WorkflowExtensionResponseMode,
        },
        publications::ApplicationPublicationJsDependencySnapshot,
    },
    ports::{
        ApplicationApiMappingRepository, ApplicationPublicationRepository,
        CreateApplicationPublicationVersionInput, ReplaceApplicationApiMappingInput,
        ReplaceWorkflowScheduleTriggerInput, SetApplicationApiEnabledInput,
        WorkflowScheduleTriggerRepository,
    },
};
use sqlx::migrate::Migrator;
use sqlx::PgPool;
use std::borrow::Cow;
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

async fn current_schema(pool: &PgPool) -> String {
    sqlx::query_scalar("select current_schema()")
        .fetch_one(pool)
        .await
        .unwrap()
}

const APPLICATION_OPERATION_BINDINGS_MIGRATION_VERSION: i64 = 20260718120000;
const WORKFLOW_EXTENSION_LEGACY_PARAMETERS_MIGRATION_VERSION: i64 = 20260721130000;

fn application_operation_bindings_migrator() -> Migrator {
    let migrations = sqlx::migrate!("./migrations")
        .iter()
        .filter(|migration| migration.version <= APPLICATION_OPERATION_BINDINGS_MIGRATION_VERSION)
        .cloned()
        .collect::<Vec<_>>();
    Migrator {
        migrations: Cow::Owned(migrations),
        ..Migrator::DEFAULT
    }
}

fn before_application_operation_bindings_migrator() -> Migrator {
    let migrations = sqlx::migrate!("./migrations")
        .iter()
        .filter(|migration| migration.version < APPLICATION_OPERATION_BINDINGS_MIGRATION_VERSION)
        .cloned()
        .collect::<Vec<_>>();
    Migrator {
        migrations: Cow::Owned(migrations),
        ..Migrator::DEFAULT
    }
}

fn before_workflow_extension_legacy_parameters_migrator() -> Migrator {
    let migrations = sqlx::migrate!("./migrations")
        .iter()
        .filter(|migration| {
            migration.version < WORKFLOW_EXTENSION_LEGACY_PARAMETERS_MIGRATION_VERSION
        })
        .cloned()
        .collect::<Vec<_>>();
    Migrator {
        migrations: Cow::Owned(migrations),
        ..Migrator::DEFAULT
    }
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

async fn seed_application(
    store: &PgControlPlaneStore,
    workspace_id: Uuid,
    actor_user_id: Uuid,
    name: &str,
) -> Uuid {
    let application_id = Uuid::now_v7();
    sqlx::query(
        r#"
        insert into applications (
            id, workspace_id, application_type, name, description, created_by, updated_by
        ) values ($1, $2, 'agent_flow', $3, '', $4, $4)
        "#,
    )
    .bind(application_id)
    .bind(workspace_id)
    .bind(name)
    .bind(actor_user_id)
    .execute(store.pool())
    .await
    .unwrap();
    application_id
}

fn workflow_extension_mapping(slug: &str) -> ApplicationApiMappingConfig {
    ApplicationApiMappingConfig {
        input: ApplicationApiMappingInput {
            query_target: "node-start.query".into(),
            model_target: None,
            inputs_target: None,
            history_target: None,
            attachments_target: None,
        },
        output: ApplicationApiMappingOutput::default(),
        extension: Some(WorkflowExtensionApiConfig {
            slug: slug.into(),
            method: WorkflowExtensionHttpMethod::Post,
            response_mode: WorkflowExtensionResponseMode::Async,
        }),
    }
}

async fn seed_flow_version_and_compiled_plan(
    store: &PgControlPlaneStore,
    application_id: Uuid,
    actor_user_id: Uuid,
) -> (Uuid, Uuid, Uuid, serde_json::Value) {
    seed_flow_version_and_compiled_plan_with_plan(
        store,
        application_id,
        actor_user_id,
        serde_json::json!({"schema_version": domain::FLOW_SCHEMA_VERSION}),
    )
    .await
}

async fn seed_flow_version_and_compiled_plan_with_plan(
    store: &PgControlPlaneStore,
    application_id: Uuid,
    actor_user_id: Uuid,
    compiled_plan: serde_json::Value,
) -> (Uuid, Uuid, Uuid, serde_json::Value) {
    let flow_id = Uuid::now_v7();
    let draft_id = Uuid::now_v7();
    let version_id = Uuid::now_v7();
    let compiled_plan_id = Uuid::now_v7();
    let document = domain::default_flow_document(flow_id);

    sqlx::query(
        "insert into flows (id, application_id, scope_id, created_by, updated_by) values ($1, $2, (select scope_id from applications where id = $2), $3, $3)",
    )
    .bind(flow_id)
    .bind(application_id)
    .bind(actor_user_id)
    .execute(store.pool())
    .await
    .unwrap();
    sqlx::query(
        "insert into flow_drafts (id, flow_id, scope_id, schema_version, document, created_by, updated_by) values ($1, $2, (select scope_id from flows where id = $2), $3, $4, $5, $5)",
    )
    .bind(draft_id)
    .bind(flow_id)
    .bind(domain::FLOW_SCHEMA_VERSION)
    .bind(&document)
    .bind(actor_user_id)
    .execute(store.pool())
    .await
    .unwrap();
    sqlx::query(
        r#"
        insert into flow_versions (
            id, flow_id, scope_id, sequence, trigger, change_kind, summary,
            summary_is_custom, is_user_protected, document, created_by, updated_by
        ) values ($1, $2, (select scope_id from flows where id = $2), 1, 'autosave', 'logical', 'published', false, true, $3, $4, $4)
        "#,
    )
    .bind(version_id)
    .bind(flow_id)
    .bind(&document)
    .bind(actor_user_id)
    .execute(store.pool())
    .await
    .unwrap();
    sqlx::query(
        r#"
        insert into flow_compiled_plans (
            id, flow_id, flow_draft_id, schema_version, document_hash,
            document_updated_at, plan, scope_id, created_by, updated_by
        )
        select $1, $2, $3, $4, 'sha256:test', updated_at, $5, (select scope_id from flows where id = $2), $6, $6
        from flow_drafts
        where id = $3
        "#,
    )
    .bind(compiled_plan_id)
    .bind(flow_id)
    .bind(draft_id)
    .bind(domain::FLOW_SCHEMA_VERSION)
    .bind(compiled_plan)
    .bind(actor_user_id)
    .execute(store.pool())
    .await
    .unwrap();

    (flow_id, version_id, compiled_plan_id, document)
}

async fn seed_publication_revision(
    store: &PgControlPlaneStore,
    flow_id: Uuid,
    draft_id: Uuid,
    actor_user_id: Uuid,
    sequence: i64,
    document_hash: &str,
) -> (Uuid, Uuid, serde_json::Value) {
    let version_id = Uuid::now_v7();
    let compiled_plan_id = Uuid::now_v7();
    let mut document = domain::default_flow_document(flow_id);
    document["revision"] = serde_json::json!(sequence);

    sqlx::query(
        r#"
        insert into flow_versions (
            id, flow_id, scope_id, sequence, trigger, change_kind, summary,
            summary_is_custom, is_user_protected, document, created_by, updated_by
        ) values ($1, $2, (select scope_id from flows where id = $2), $3, 'autosave', 'logical', 'published', false, true, $4, $5, $5)
        "#,
    )
    .bind(version_id)
    .bind(flow_id)
    .bind(sequence)
    .bind(&document)
    .bind(actor_user_id)
    .execute(store.pool())
    .await
    .unwrap();

    sqlx::query(
        r#"
        insert into flow_compiled_plans (
            id, flow_id, flow_draft_id, schema_version, document_hash,
            document_updated_at, plan, scope_id, created_by, updated_by
        )
        select $1, $2, $3, $4, $5, updated_at, $6, (select scope_id from flows where id = $2), $7, $7
        from flow_drafts
        where id = $3
        "#,
    )
    .bind(compiled_plan_id)
    .bind(flow_id)
    .bind(draft_id)
    .bind(domain::FLOW_SCHEMA_VERSION)
    .bind(document_hash)
    .bind(serde_json::json!({"schema_version": domain::FLOW_SCHEMA_VERSION}))
    .bind(actor_user_id)
    .execute(store.pool())
    .await
    .unwrap();

    (version_id, compiled_plan_id, document)
}

#[tokio::test]
async fn application_public_api_repository_api_keys_key_kind_rejects_legacy_data_model_keys() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool.clone());
    let schema = current_schema(&pool).await;
    let workspace_id = seed_workspace(&store, "Application API Keys").await;
    let actor_user_id = seed_user(&store, workspace_id, "api-key-owner").await;
    let application_id = seed_application(&store, workspace_id, actor_user_id, "Public App").await;

    let api_key_columns: Vec<String> = sqlx::query_scalar(
        r#"
        select column_name
        from information_schema.columns
        where table_schema = $1
          and table_name = 'api_keys'
        "#,
    )
    .bind(&schema)
    .fetch_all(&pool)
    .await
    .unwrap();

    assert!(api_key_columns.contains(&"key_kind".to_string()));
    assert!(api_key_columns.contains(&"application_id".to_string()));

    sqlx::query(
        r#"
        insert into api_keys (
            id, name, token_hash, token_prefix, creator_user_id, tenant_id,
            scope_kind, scope_id, key_kind, application_id, enabled
        ) values
            ($1, 'Application Key', 'apk-hash', 'apk_prefix', $2, $3,
             'workspace', $4, 'application_api_key', $5, true),
            ($6, 'User Key', 'pat-hash', 'pat_prefix', $2, $3,
             'workspace', $4, 'user_api_key', null, true)
        "#,
    )
    .bind(Uuid::now_v7())
    .bind(actor_user_id)
    .bind(root_tenant_id(&store).await)
    .bind(workspace_id)
    .bind(application_id)
    .bind(Uuid::now_v7())
    .execute(&pool)
    .await
    .unwrap();

    let counts: Vec<(String, i64)> = sqlx::query_as(
        r#"
        select key_kind, count(*)::bigint
        from api_keys
        group by key_kind
        order by key_kind
        "#,
    )
    .fetch_all(&pool)
    .await
    .unwrap();

    assert_eq!(
        counts,
        vec![
            ("application_api_key".to_string(), 1),
            ("user_api_key".to_string(), 1),
        ]
    );

    let legacy_data_model_key = sqlx::query(
        r#"
        insert into api_keys (
            id, name, token_hash, token_prefix, creator_user_id, tenant_id,
            scope_kind, scope_id, key_kind, application_id, enabled
        ) values (
            $1, 'Legacy Data Model Key', 'dmk-hash', 'dmk_prefix', $2, $3,
            'workspace', $4, 'data_model_api_key', null, true
        )
        "#,
    )
    .bind(Uuid::now_v7())
    .bind(actor_user_id)
    .bind(root_tenant_id(&store).await)
    .bind(workspace_id)
    .execute(&pool)
    .await;

    assert!(
        legacy_data_model_key.is_err(),
        "legacy data_model_api_key rows must be rejected"
    );

    let invalid_application_key = sqlx::query(
        r#"
        insert into api_keys (
            id, name, token_hash, token_prefix, creator_user_id, tenant_id,
            scope_kind, scope_id, key_kind, application_id, enabled
        ) values (
            $1, 'Broken Application Key', 'apk-broken', 'apk_broken', $2, $3,
            'workspace', $4, 'application_api_key', null, true
        )
        "#,
    )
    .bind(Uuid::now_v7())
    .bind(actor_user_id)
    .bind(root_tenant_id(&store).await)
    .bind(workspace_id)
    .execute(&pool)
    .await;

    assert!(
        invalid_application_key.is_err(),
        "application_api_key rows must carry an application_id"
    );

    let invalid_user_key = sqlx::query(
        r#"
        insert into api_keys (
            id, name, token_hash, token_prefix, creator_user_id, tenant_id,
            scope_kind, scope_id, key_kind, application_id, enabled
        ) values (
            $1, 'Broken User Key', 'pat-broken', 'pat_broken', $2, $3,
            'workspace', $4, 'user_api_key', $5, true
        )
        "#,
    )
    .bind(Uuid::now_v7())
    .bind(actor_user_id)
    .bind(root_tenant_id(&store).await)
    .bind(workspace_id)
    .bind(application_id)
    .execute(&pool)
    .await;

    assert!(
        invalid_user_key.is_err(),
        "user_api_key rows must not carry an application_id"
    );
}

#[tokio::test]
async fn application_public_api_repository_mapping_round_trips_default_and_replacement() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool.clone());
    let workspace_id = seed_workspace(&store, "Application Mapping").await;
    let actor_user_id = seed_user(&store, workspace_id, "mapping-owner").await;
    let application_id = seed_application(&store, workspace_id, actor_user_id, "Public App").await;

    let default_mapping =
        ApplicationApiMappingRepository::get_application_api_mapping(&store, application_id)
            .await
            .unwrap()
            .unwrap_or_else(ApplicationApiMappingDraft::default_native);
    let replacement = ApplicationApiMappingConfig {
        input: ApplicationApiMappingInput {
            query_target: "start.query".into(),
            model_target: None,
            inputs_target: Some("start.inputs".into()),
            history_target: Some("start.history".into()),
            attachments_target: None,
        },
        output: ApplicationApiMappingOutput {
            answer_selector: Some("answer.text".into()),
            usage_selector: None,
            files_selector: None,
            error_selector: None,
        },
        extension: None,
    };
    ApplicationApiMappingRepository::replace_application_api_mapping(
        &store,
        &ReplaceApplicationApiMappingInput {
            actor_user_id,
            application_id,
            mapping: replacement.clone(),
        },
    )
    .await
    .unwrap();
    let stored =
        ApplicationApiMappingRepository::get_application_api_mapping(&store, application_id)
            .await
            .unwrap()
            .unwrap();

    assert_eq!(
        default_mapping,
        ApplicationApiMappingDraft::default_native()
    );
    assert_eq!(stored.mapping, replacement);
}

#[tokio::test]
async fn application_public_api_repository_mapping_extension_slug_round_trips_and_conflicts() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool.clone());
    let workspace_id = seed_workspace(&store, "Application Mapping Extension").await;
    let actor_user_id = seed_user(&store, workspace_id, "mapping-extension-owner").await;
    let first_application_id =
        seed_application(&store, workspace_id, actor_user_id, "Public App A").await;
    let second_application_id =
        seed_application(&store, workspace_id, actor_user_id, "Public App B").await;
    let mapping = workflow_extension_mapping("open-ticket-pg");

    ApplicationApiMappingRepository::replace_application_api_mapping(
        &store,
        &ReplaceApplicationApiMappingInput {
            actor_user_id,
            application_id: first_application_id,
            mapping: mapping.clone(),
        },
    )
    .await
    .unwrap();
    let stored =
        ApplicationApiMappingRepository::get_application_api_mapping(&store, first_application_id)
            .await
            .unwrap()
            .unwrap();
    let owner =
        ApplicationApiMappingRepository::load_application_api_mapping_application_id_by_extension_slug(
            &store,
            "open-ticket-pg",
        )
        .await
        .unwrap();
    let duplicate = ApplicationApiMappingRepository::replace_application_api_mapping(
        &store,
        &ReplaceApplicationApiMappingInput {
            actor_user_id,
            application_id: second_application_id,
            mapping,
        },
    )
    .await;

    assert_eq!(stored.mapping.extension_slug(), Some("open-ticket-pg"));
    assert_eq!(owner, Some(first_application_id));
    assert!(duplicate
        .unwrap_err()
        .to_string()
        .contains("extension_slug"));
}

#[tokio::test]
async fn application_public_api_repository_workflow_schedule_trigger_round_trips_and_updates() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool.clone());
    let workspace_id = seed_workspace(&store, "Workflow Schedule Trigger").await;
    let actor_user_id = seed_user(&store, workspace_id, "workflow-schedule-owner").await;
    let application_id =
        seed_application(&store, workspace_id, actor_user_id, "Scheduled Workflow").await;

    let created = WorkflowScheduleTriggerRepository::replace_workflow_schedule_trigger(
        &store,
        &ReplaceWorkflowScheduleTriggerInput {
            actor_user_id,
            workspace_id,
            application_id,
            enabled: true,
            cron: "0 9 * * *".into(),
            timezone: "Asia/Shanghai".into(),
            input_payload: serde_json::json!({
                "node-workflow-start": { "customer_id": "C-42" }
            }),
        },
    )
    .await
    .unwrap();
    let updated = WorkflowScheduleTriggerRepository::replace_workflow_schedule_trigger(
        &store,
        &ReplaceWorkflowScheduleTriggerInput {
            actor_user_id,
            workspace_id,
            application_id,
            enabled: false,
            cron: "30 10 * * 1".into(),
            timezone: "UTC".into(),
            input_payload: serde_json::json!({
                "node-workflow-start": { "customer_id": "C-84" }
            }),
        },
    )
    .await
    .unwrap();
    let loaded =
        WorkflowScheduleTriggerRepository::get_workflow_schedule_trigger(&store, application_id)
            .await
            .unwrap()
            .unwrap();

    assert_eq!(created.id, updated.id);
    assert_eq!(loaded.workspace_id, workspace_id);
    assert!(!loaded.enabled);
    assert_eq!(loaded.cron, "30 10 * * 1");
    assert_eq!(loaded.timezone, "UTC");
    assert_eq!(
        loaded.input_payload["node-workflow-start"]["customer_id"],
        serde_json::json!("C-84")
    );
}

#[tokio::test]
async fn application_public_api_repository_publication_insert_uses_real_foreign_key_rows() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool.clone());
    let workspace_id = seed_workspace(&store, "Application Publication").await;
    let actor_user_id = seed_user(&store, workspace_id, "publication-owner").await;
    let application_id = seed_application(&store, workspace_id, actor_user_id, "Public App").await;
    let (flow_id, flow_version_id, compiled_plan_id, document) =
        seed_flow_version_and_compiled_plan(&store, application_id, actor_user_id).await;

    let publication =
        ApplicationPublicationRepository::create_active_application_publication_version(
            &store,
            &CreateApplicationPublicationVersionInput {
                actor_user_id,
                application_id,
                mapping_snapshot: ApplicationApiMappingConfig::default_native(),
                extension_slug: None,
                api_enabled: true,
                compiled_plan_id,
                flow_id,
                flow_version_id,
                flow_schema_version: domain::FLOW_SCHEMA_VERSION.to_string(),
                document_hash: "sha256:test".into(),
                document_snapshot: document.clone(),
                runtime_profile_snapshot: serde_json::json!({"profile": "test"}),
                output_selector: serde_json::json!({"answer_selector": "answer.text"}),
                dependency_snapshot: Vec::new(),
            },
        )
        .await
        .unwrap();
    let active = ApplicationPublicationRepository::load_active_application_publication(
        &store,
        application_id,
    )
    .await
    .unwrap()
    .unwrap();
    let stored_api_enabled: bool =
        sqlx::query_scalar("select api_enabled from applications where id = $1")
            .bind(application_id)
            .fetch_one(&pool)
            .await
            .unwrap();

    assert_eq!(publication.flow_id, flow_id);
    assert_eq!(publication.flow_version_id, flow_version_id);
    assert_eq!(publication.compiled_plan_id, compiled_plan_id);
    assert_eq!(publication.document_snapshot, document);
    assert_eq!(active.id, publication.id);
    assert!(stored_api_enabled);
}

#[tokio::test]
async fn application_public_api_repository_publication_extension_slug_lookup_list_and_conflict() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool.clone());
    let workspace_id = seed_workspace(&store, "Application Publication Extension").await;
    let actor_user_id = seed_user(&store, workspace_id, "publication-extension-owner").await;
    let first_application_id =
        seed_application(&store, workspace_id, actor_user_id, "Public App A").await;
    let second_application_id =
        seed_application(&store, workspace_id, actor_user_id, "Public App B").await;
    sqlx::query(
        "update applications set application_type = 'workflow', workflow_trigger_type = 'extension' where id = any($1)",
    )
        .bind(vec![first_application_id, second_application_id])
        .execute(&pool)
        .await
        .unwrap();
    let (first_flow_id, first_flow_version_id, first_compiled_plan_id, first_document) =
        seed_flow_version_and_compiled_plan(&store, first_application_id, actor_user_id).await;
    let (second_flow_id, second_flow_version_id, second_compiled_plan_id, second_document) =
        seed_flow_version_and_compiled_plan(&store, second_application_id, actor_user_id).await;
    let mapping = workflow_extension_mapping("open-ticket-pg-publication");

    let publication =
        ApplicationPublicationRepository::create_active_application_publication_version(
            &store,
            &CreateApplicationPublicationVersionInput {
                actor_user_id,
                application_id: first_application_id,
                mapping_snapshot: mapping.clone(),
                extension_slug: Some("open-ticket-pg-publication".into()),
                api_enabled: true,
                compiled_plan_id: first_compiled_plan_id,
                flow_id: first_flow_id,
                flow_version_id: first_flow_version_id,
                flow_schema_version: domain::FLOW_SCHEMA_VERSION.to_string(),
                document_hash: "sha256:first-extension".into(),
                document_snapshot: first_document,
                runtime_profile_snapshot: serde_json::json!({}),
                output_selector: serde_json::json!({}),
                dependency_snapshot: Vec::new(),
            },
        )
        .await
        .unwrap();
    let loaded =
        ApplicationPublicationRepository::load_active_application_publication_by_extension_slug(
            &store,
            "open-ticket-pg-publication",
        )
        .await
        .unwrap()
        .unwrap();
    let enabled = ApplicationPublicationRepository::list_enabled_extension_publications(&store)
        .await
        .unwrap();
    let duplicate =
        ApplicationPublicationRepository::create_active_application_publication_version(
            &store,
            &CreateApplicationPublicationVersionInput {
                actor_user_id,
                application_id: second_application_id,
                mapping_snapshot: mapping,
                extension_slug: Some("open-ticket-pg-publication".into()),
                api_enabled: true,
                compiled_plan_id: second_compiled_plan_id,
                flow_id: second_flow_id,
                flow_version_id: second_flow_version_id,
                flow_schema_version: domain::FLOW_SCHEMA_VERSION.to_string(),
                document_hash: "sha256:second-extension".into(),
                document_snapshot: second_document,
                runtime_profile_snapshot: serde_json::json!({}),
                output_selector: serde_json::json!({}),
                dependency_snapshot: Vec::new(),
            },
        )
        .await;
    let duplicate_application_api_enabled: bool =
        sqlx::query_scalar("select api_enabled from applications where id = $1")
            .bind(second_application_id)
            .fetch_one(&pool)
            .await
            .unwrap();

    assert_eq!(
        publication.extension_slug.as_deref(),
        Some("open-ticket-pg-publication")
    );
    assert_eq!(loaded.id, publication.id);
    assert_eq!(enabled.len(), 1);
    assert_eq!(
        enabled[0].extension_slug.as_deref(),
        Some("open-ticket-pg-publication")
    );
    assert!(duplicate
        .unwrap_err()
        .to_string()
        .contains("extension_slug"));
    assert!(
        !duplicate_application_api_enabled,
        "publication conflict must roll back the application api_enabled write"
    );
}

#[tokio::test]
async fn application_public_api_repository_republish_updates_single_current_publication() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool.clone());
    let workspace_id = seed_workspace(&store, "Application Republish").await;
    let actor_user_id = seed_user(&store, workspace_id, "republish-owner").await;
    let application_id = seed_application(&store, workspace_id, actor_user_id, "Public App").await;
    let (flow_id, first_flow_version_id, first_compiled_plan_id, first_document) =
        seed_flow_version_and_compiled_plan(&store, application_id, actor_user_id).await;
    let draft_id: Uuid = sqlx::query_scalar("select id from flow_drafts where flow_id = $1")
        .bind(flow_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    let (second_flow_version_id, second_compiled_plan_id, second_document) =
        seed_publication_revision(&store, flow_id, draft_id, actor_user_id, 2, "sha256:second")
            .await;

    let first_publication =
        ApplicationPublicationRepository::create_active_application_publication_version(
            &store,
            &CreateApplicationPublicationVersionInput {
                actor_user_id,
                application_id,
                mapping_snapshot: ApplicationApiMappingConfig::default_native(),
                extension_slug: None,
                api_enabled: true,
                compiled_plan_id: first_compiled_plan_id,
                flow_id,
                flow_version_id: first_flow_version_id,
                flow_schema_version: domain::FLOW_SCHEMA_VERSION.to_string(),
                document_hash: "sha256:first".into(),
                document_snapshot: first_document,
                runtime_profile_snapshot: serde_json::json!({"profile": "first"}),
                output_selector: serde_json::json!({"answer_selector": "answer.text"}),
                dependency_snapshot: Vec::new(),
            },
        )
        .await
        .unwrap();
    let second_publication =
        ApplicationPublicationRepository::create_active_application_publication_version(
            &store,
            &CreateApplicationPublicationVersionInput {
                actor_user_id,
                application_id,
                mapping_snapshot: ApplicationApiMappingConfig::default_native(),
                extension_slug: None,
                api_enabled: false,
                compiled_plan_id: second_compiled_plan_id,
                flow_id,
                flow_version_id: second_flow_version_id,
                flow_schema_version: domain::FLOW_SCHEMA_VERSION.to_string(),
                document_hash: "sha256:second".into(),
                document_snapshot: second_document.clone(),
                runtime_profile_snapshot: serde_json::json!({"profile": "second"}),
                output_selector: serde_json::json!({"answer_selector": "answer.final"}),
                dependency_snapshot: Vec::new(),
            },
        )
        .await
        .unwrap();
    ApplicationPublicationRepository::set_application_api_enabled(
        &store,
        &SetApplicationApiEnabledInput {
            actor_user_id,
            application_id,
            api_enabled: true,
        },
    )
    .await
    .unwrap();

    let publications = ApplicationPublicationRepository::list_application_publication_versions(
        &store,
        application_id,
    )
    .await
    .unwrap();
    let row_count: i64 = sqlx::query_scalar(
        "select count(*)::bigint from application_publication_versions where application_id = $1",
    )
    .bind(application_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let publication_audit: (bool, Uuid) = sqlx::query_as(
        "select api_enabled, updated_by from application_publication_versions where application_id = $1",
    )
    .bind(application_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(second_publication.id, first_publication.id);
    assert_eq!(second_publication.version_sequence, 1);
    assert_eq!(second_publication.flow_version_id, second_flow_version_id);
    assert_eq!(second_publication.compiled_plan_id, second_compiled_plan_id);
    assert_eq!(second_publication.document_snapshot, second_document);
    assert!(!second_publication.api_enabled);
    assert_eq!(publications.len(), 1);
    assert_eq!(publications[0].id, second_publication.id);
    assert!(publications[0].api_enabled);
    assert_eq!(row_count, 1);
    assert_eq!(publication_audit, (true, actor_user_id));
}

#[tokio::test]
async fn application_public_api_js_dependency_snapshot_persists_empty_array_without_selection() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool.clone());
    let workspace_id = seed_workspace(&store, "Application Publication Empty Dependency").await;
    let actor_user_id = seed_user(&store, workspace_id, "publication-empty-dependency-owner").await;
    let application_id = seed_application(&store, workspace_id, actor_user_id, "Public App").await;
    let (flow_id, flow_version_id, compiled_plan_id, document) =
        seed_flow_version_and_compiled_plan(&store, application_id, actor_user_id).await;

    let publication =
        ApplicationPublicationRepository::create_active_application_publication_version(
            &store,
            &CreateApplicationPublicationVersionInput {
                actor_user_id,
                application_id,
                mapping_snapshot: ApplicationApiMappingConfig::default_native(),
                extension_slug: None,
                api_enabled: true,
                compiled_plan_id,
                flow_id,
                flow_version_id,
                flow_schema_version: domain::FLOW_SCHEMA_VERSION.to_string(),
                document_hash: "sha256:test".into(),
                document_snapshot: document,
                runtime_profile_snapshot: serde_json::json!({}),
                output_selector: serde_json::json!({}),
                dependency_snapshot: Vec::new(),
            },
        )
        .await
        .unwrap();
    let reloaded = ApplicationPublicationRepository::get_application_publication_version(
        &store,
        publication.id,
    )
    .await
    .unwrap()
    .unwrap();

    assert_eq!(publication.dependency_snapshot, Vec::new());
    assert_eq!(reloaded.dependency_snapshot, Vec::new());
}

#[tokio::test]
async fn application_public_api_js_dependency_snapshot_persists_on_publication_version() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool.clone());
    let workspace_id = seed_workspace(&store, "Application Publication Dependency").await;
    let actor_user_id = seed_user(&store, workspace_id, "publication-dependency-owner").await;
    let application_id = seed_application(&store, workspace_id, actor_user_id, "Public App").await;
    let (flow_id, flow_version_id, compiled_plan_id, document) =
        seed_flow_version_and_compiled_plan(&store, application_id, actor_user_id).await;
    let dependency_snapshot = vec![ApplicationPublicationJsDependencySnapshot {
        installation_id: Uuid::from_u128(0x90000000000000000000000000000001),
        provider_code: "fixture_js_dependency_pack".into(),
        plugin_id: "fixture_js_dependency_pack@3.24.0".into(),
        plugin_version: "3.24.0".into(),
        alias: "zod".into(),
        package: "zod".into(),
        version: "3.24.0".into(),
        target: "backend_code".into(),
        artifact_path: "artifacts/zod-3.24.0.backend.mjs".into(),
        artifact_hash: "sha256-zod-3.24.0".into(),
        integrity: "sha256-zod-3.24.0".into(),
        permissions: domain::JsDependencyPermissions {
            network: "outbound_only".into(),
            filesystem: "deny".into(),
            env: "deny".into(),
        },
    }];

    let publication =
        ApplicationPublicationRepository::create_active_application_publication_version(
            &store,
            &CreateApplicationPublicationVersionInput {
                actor_user_id,
                application_id,
                mapping_snapshot: ApplicationApiMappingConfig::default_native(),
                extension_slug: None,
                api_enabled: true,
                compiled_plan_id,
                flow_id,
                flow_version_id,
                flow_schema_version: domain::FLOW_SCHEMA_VERSION.to_string(),
                document_hash: "sha256:test".into(),
                document_snapshot: document,
                runtime_profile_snapshot: serde_json::json!({}),
                output_selector: serde_json::json!({}),
                dependency_snapshot: dependency_snapshot.clone(),
            },
        )
        .await
        .unwrap();
    let reloaded = ApplicationPublicationRepository::get_application_publication_version(
        &store,
        publication.id,
    )
    .await
    .unwrap()
    .unwrap();

    assert_eq!(publication.dependency_snapshot, dependency_snapshot);
    assert_eq!(reloaded.dependency_snapshot[0].alias, "zod");
    assert_eq!(
        reloaded.dependency_snapshot[0].artifact_hash,
        "sha256-zod-3.24.0"
    );
    assert_eq!(
        reloaded.dependency_snapshot[0].permissions.network,
        "outbound_only"
    );
}

#[tokio::test]
async fn workflow_extension_legacy_parameters_migration_normalizes_current_and_published_mappings()
{
    let pool = isolated_database().await.connect().await.unwrap();
    before_workflow_extension_legacy_parameters_migrator()
        .run(&pool)
        .await
        .unwrap();
    let store = PgControlPlaneStore::new(pool.clone());
    let workspace_id = seed_workspace(&store, "Legacy Workflow Extension Mapping").await;
    let actor_user_id = seed_user(&store, workspace_id, "legacy-extension-owner").await;
    let application_id =
        seed_application(&store, workspace_id, actor_user_id, "Legacy Extension").await;
    sqlx::query(
        "update applications set application_type = 'workflow', workflow_trigger_type = 'extension' where id = $1",
    )
    .bind(application_id)
    .execute(&pool)
    .await
    .unwrap();
    let (flow_id, flow_version_id, compiled_plan_id, document) =
        seed_flow_version_and_compiled_plan(&store, application_id, actor_user_id).await;
    let mut legacy_mapping =
        serde_json::to_value(workflow_extension_mapping("legacy-openai")).unwrap();
    legacy_mapping["extension"]["parameters"] = serde_json::json!([]);
    assert_eq!(
        legacy_mapping["extension"]["parameters"],
        serde_json::json!([])
    );
    let publication_id = Uuid::now_v7();

    sqlx::query(
        r#"
        insert into application_api_mappings (
            id, application_id, scope_id, extension_slug, mapping_config, operation_bindings,
            created_by, updated_by
        ) values ($1, $2, (select scope_id from applications where id = $2), $3, $4, '{}'::jsonb, $5, $5)
        "#,
    )
    .bind(Uuid::now_v7())
    .bind(application_id)
    .bind("legacy-openai")
    .bind(&legacy_mapping)
    .bind(actor_user_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"
        insert into application_publication_versions (
            id, application_id, scope_id, flow_id, flow_version_id, compiled_plan_id,
            extension_slug, version_sequence, active, api_enabled, flow_schema_version,
            document_hash, document_snapshot, mapping_snapshot, operation_bindings,
            runtime_profile_snapshot, output_selector, dependency_snapshot, created_by, updated_by
        ) values (
            $1, $2, (select scope_id from applications where id = $2), $3, $4, $5,
            $6, 1, true, true, $7, $8, $9, $10, '{}'::jsonb,
            '{}'::jsonb, '{}'::jsonb, '[]'::jsonb, $11, $11
        )
        "#,
    )
    .bind(publication_id)
    .bind(application_id)
    .bind(flow_id)
    .bind(flow_version_id)
    .bind(compiled_plan_id)
    .bind("legacy-openai")
    .bind(domain::FLOW_SCHEMA_VERSION)
    .bind("sha256:legacy-extension")
    .bind(document)
    .bind(&legacy_mapping)
    .bind(actor_user_id)
    .execute(&pool)
    .await
    .unwrap();

    run_migrations(&pool).await.unwrap();

    let current =
        ApplicationApiMappingRepository::get_application_api_mapping(&store, application_id)
            .await
            .unwrap()
            .unwrap();
    let publications =
        ApplicationPublicationRepository::list_enabled_extension_publications(&store)
            .await
            .unwrap();
    let current_json: serde_json::Value = sqlx::query_scalar(
        "select mapping_config from application_api_mappings where application_id = $1",
    )
    .bind(application_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let published_json: serde_json::Value = sqlx::query_scalar(
        "select mapping_snapshot from application_publication_versions where id = $1",
    )
    .bind(publication_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(current.mapping, workflow_extension_mapping("legacy-openai"));
    assert_eq!(publications.len(), 1);
    assert_eq!(publications[0].id, publication_id);
    assert!(current_json["extension"]["parameters"].is_null());
    assert!(published_json["extension"]["parameters"].is_null());
}

/// Root #1366 AC-003 / AC-006: old snapshots backfill Generate only when identity is unique,
/// and the reversible migration restores the old readable snapshot shape.
#[tokio::test]
async fn operation_binding_migration_backfills_unique_generate_and_rolls_back() {
    let pool = isolated_database().await.connect().await.unwrap();
    before_application_operation_bindings_migrator()
        .run(&pool)
        .await
        .unwrap();
    let store = PgControlPlaneStore::new(pool.clone());
    let workspace_id = seed_workspace(&store, "Operation Binding Migration").await;
    let actor_user_id = seed_user(&store, workspace_id, "binding-migration-owner").await;
    let target_inventory = [0_usize, 1, 2];
    let mut publications = Vec::new();

    for target_count in target_inventory {
        let application_id = seed_application(
            &store,
            workspace_id,
            actor_user_id,
            &format!("Binding Migration {target_count}"),
        )
        .await;
        let nodes = (0..target_count)
            .map(|index| {
                let node_id = format!("node-llm-{index}");
                (
                    node_id.clone(),
                    serde_json::json!({
                        "node_id": node_id,
                        "node_type": "llm",
                        "llm_runtime": {
                            "provider_instance_id": Uuid::now_v7().to_string(),
                            "provider_code": "fixture_provider",
                            "protocol": "fixture",
                            "model": "fixture-model"
                        }
                    }),
                )
            })
            .collect::<serde_json::Map<_, _>>();
        let (flow_id, flow_version_id, compiled_plan_id, document) =
            seed_flow_version_and_compiled_plan_with_plan(
                &store,
                application_id,
                actor_user_id,
                serde_json::json!({"nodes": nodes}),
            )
            .await;
        let publication_id = Uuid::now_v7();
        sqlx::query(
            r#"
            insert into application_publication_versions (
                id, application_id, scope_id, flow_id, flow_version_id, compiled_plan_id,
                extension_slug, version_sequence, active, api_enabled, flow_schema_version,
                document_hash, document_snapshot, mapping_snapshot, runtime_profile_snapshot,
                output_selector, dependency_snapshot, created_by, updated_by
            ) values (
                $1, $2, (select scope_id from applications where id = $2), $3, $4, $5,
                null, 1, true, true, $6, $7, $8, $9, '{}'::jsonb, '{}'::jsonb,
                '[]'::jsonb, $10, $10
            )
            "#,
        )
        .bind(publication_id)
        .bind(application_id)
        .bind(flow_id)
        .bind(flow_version_id)
        .bind(compiled_plan_id)
        .bind(domain::FLOW_SCHEMA_VERSION)
        .bind(format!("sha256:binding-{target_count}"))
        .bind(document)
        .bind(serde_json::to_value(ApplicationApiMappingConfig::default_native()).unwrap())
        .bind(actor_user_id)
        .execute(&pool)
        .await
        .unwrap();
        publications.push((target_count, publication_id));
    }

    application_operation_bindings_migrator()
        .run(&pool)
        .await
        .unwrap();

    for (target_count, publication_id) in &publications {
        let bindings: serde_json::Value = sqlx::query_scalar(
            "select operation_bindings from application_publication_versions where id = $1",
        )
        .bind(publication_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(bindings["count_tokens"].is_null());
        assert!(bindings["compact"]["responses_compact"].is_null());
        assert!(bindings["compact"]["responses_compaction_v2"].is_null());
        if *target_count == 1 {
            assert_eq!(
                bindings["generate"]["target_node_id"],
                serde_json::json!("node-llm-0")
            );
        } else {
            assert!(bindings["generate"].is_null());
        }
    }

    application_operation_bindings_migrator()
        .undo(&pool, APPLICATION_OPERATION_BINDINGS_MIGRATION_VERSION - 1)
        .await
        .unwrap();
    let binding_columns: i64 = sqlx::query_scalar(
        r#"
        select count(*)::bigint
        from information_schema.columns
        where table_schema = current_schema()
          and table_name in ('application_api_mappings', 'application_publication_versions')
          and column_name = 'operation_bindings'
        "#,
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let old_mapping_snapshot: serde_json::Value = sqlx::query_scalar(
        "select mapping_snapshot from application_publication_versions where id = $1",
    )
    .bind(publications[1].1)
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(binding_columns, 0);
    assert_eq!(
        serde_json::from_value::<ApplicationApiMappingConfig>(old_mapping_snapshot).unwrap(),
        ApplicationApiMappingConfig::default_native()
    );
}

#[tokio::test]
async fn application_public_api_repository_migration_creates_publication_core_tables_and_indexes() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let schema = current_schema(&pool).await;

    let tables: Vec<String> = sqlx::query_scalar(
        r#"
        select table_name
        from information_schema.tables
        where table_schema = $1
        "#,
    )
    .bind(&schema)
    .fetch_all(&pool)
    .await
    .unwrap();
    let schedule_columns: Vec<String> = sqlx::query_scalar(
        r#"
        select column_name
        from information_schema.columns
        where table_schema = $1
          and table_name = 'workflow_schedule_triggers'
        "#,
    )
    .bind(&schema)
    .fetch_all(&pool)
    .await
    .unwrap();
    let application_columns: Vec<String> = sqlx::query_scalar(
        r#"
        select column_name
        from information_schema.columns
        where table_schema = $1
          and table_name = 'applications'
        "#,
    )
    .bind(&schema)
    .fetch_all(&pool)
    .await
    .unwrap();
    let mapping_columns: Vec<String> = sqlx::query_scalar(
        r#"
        select column_name
        from information_schema.columns
        where table_schema = $1
          and table_name = 'application_api_mappings'
        "#,
    )
    .bind(&schema)
    .fetch_all(&pool)
    .await
    .unwrap();
    let publication_columns: Vec<String> = sqlx::query_scalar(
        r#"
        select column_name
        from information_schema.columns
        where table_schema = $1
          and table_name = 'application_publication_versions'
        "#,
    )
    .bind(&schema)
    .fetch_all(&pool)
    .await
    .unwrap();
    let extension_slug_indexes: Vec<String> = sqlx::query_scalar(
        r#"
        select indexname
        from pg_indexes
        where schemaname = $1
          and indexname in (
              'application_api_mappings_extension_slug_uidx',
              'application_publication_versions_extension_slug_uidx'
          )
        order by indexname
        "#,
    )
    .bind(&schema)
    .fetch_all(&pool)
    .await
    .unwrap();
    let partial_active_publication_indexes: Vec<String> = sqlx::query_scalar(
        r#"
        select indexdef
        from pg_indexes
        where schemaname = $1
          and tablename = 'application_publication_versions'
          and indexdef ilike '%where active%'
        "#,
    )
    .bind(&schema)
    .fetch_all(&pool)
    .await
    .unwrap();
    let current_publication_indexes: Vec<String> = sqlx::query_scalar(
        r#"
        select indexdef
        from pg_indexes
        where schemaname = $1
          and tablename = 'application_publication_versions'
          and indexname = 'application_publication_versions_application_id_idx'
        "#,
    )
    .bind(&schema)
    .fetch_all(&pool)
    .await
    .unwrap();
    let schedule_indexes: Vec<String> = sqlx::query_scalar(
        r#"
        select indexname
        from pg_indexes
        where schemaname = $1
          and tablename = 'workflow_schedule_triggers'
        order by indexname
        "#,
    )
    .bind(&schema)
    .fetch_all(&pool)
    .await
    .unwrap();

    assert!(tables.contains(&"application_api_mappings".to_string()));
    assert!(tables.contains(&"application_publication_versions".to_string()));
    assert!(tables.contains(&"workflow_schedule_triggers".to_string()));
    assert!(application_columns.contains(&"api_enabled".to_string()));
    assert!(mapping_columns.contains(&"extension_slug".to_string()));
    assert!(!mapping_columns.contains(&"operation_bindings".to_string()));
    assert!(!publication_columns.contains(&"operation_bindings".to_string()));
    for expected_column in [
        "application_id",
        "flow_id",
        "flow_version_id",
        "compiled_plan_id",
        "version_sequence",
        "active",
        "api_enabled",
        "document_snapshot",
        "mapping_snapshot",
        "runtime_profile_snapshot",
        "output_selector",
        "dependency_snapshot",
        "extension_slug",
    ] {
        assert!(
            publication_columns.contains(&expected_column.to_string()),
            "missing application_publication_versions.{expected_column}"
        );
    }
    assert!(
        partial_active_publication_indexes.is_empty(),
        "current publication is unique per application and should not rely on a partial active index"
    );
    assert!(
        current_publication_indexes.len() == 1 && current_publication_indexes[0].contains("UNIQUE"),
        "a unique application_id index should enforce one current publication per application"
    );
    assert_eq!(
        extension_slug_indexes,
        vec![
            "application_api_mappings_extension_slug_uidx".to_string(),
            "application_publication_versions_extension_slug_uidx".to_string(),
        ]
    );
    for expected_column in [
        "application_id",
        "scope_id",
        "enabled",
        "cron",
        "timezone",
        "input_payload",
    ] {
        assert!(
            schedule_columns.contains(&expected_column.to_string()),
            "missing workflow_schedule_triggers.{expected_column}"
        );
    }
    assert!(
        schedule_indexes.contains(&"workflow_schedule_triggers_application_id_uidx".to_string())
    );
}
