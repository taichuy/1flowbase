use control_plane::ports::{
    CreatePluginAssignmentInput, FrontendBlockCatalogRegistryInput, FrontendBlockCatalogRepository,
    PluginRepository, ReplaceInstallationFrontendBlocksInput, UpsertPluginInstallationInput,
};
use domain::{
    PluginArtifactStatus, PluginAvailabilityStatus, PluginDesiredState, PluginRuntimeStatus,
    PluginVerificationStatus,
};
use serde_json::json;
use storage_postgres::{run_migrations, PgControlPlaneStore};
use uuid::Uuid;

const FRONTEND_BLOCK_CODE_TEMPLATES_MIGRATION_SQL: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/migrations/20260710163000_add_frontend_block_code_templates.sql"
));

fn base_database_url() -> String {
    std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:1flowbase@127.0.0.1:35432/1flowbase".into())
}

async fn isolated_database() -> postgres_test_support::PostgresTestSchema {
    postgres_test_support::PostgresTestSchema::create(&base_database_url())
        .await
        .unwrap()
}

async fn seed_store() -> (
    PgControlPlaneStore,
    domain::WorkspaceRecord,
    domain::UserRecord,
) {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);

    let tenant = store.upsert_root_tenant().await.unwrap();
    let workspace = store
        .upsert_workspace(tenant.id, "1flowbase")
        .await
        .unwrap();
    store
        .upsert_permission_catalog(&access_control::permission_catalog())
        .await
        .unwrap();
    store.upsert_builtin_roles(workspace.id).await.unwrap();
    store
        .upsert_authenticator(&domain::AuthenticatorRecord {
            id: domain::PASSWORD_LOCAL_AUTHENTICATOR_ID,
            auth_type: "password-local".into(),
            title: "Password".into(),
            enabled: true,
            is_builtin: true,
            sort_order: 0,
            options: json!({}),
        })
        .await
        .unwrap();
    let actor = store
        .upsert_root_user(
            workspace.id,
            "root",
            "root@example.com",
            "$argon2id$v=19$m=19456,t=2,p=1$test$test",
            "Root",
            "Root",
        )
        .await
        .unwrap();

    (store, workspace, actor)
}

#[tokio::test]
async fn frontend_block_code_template_migration_backfills_existing_rows() {
    let pool = isolated_database().await.connect().await.unwrap();
    sqlx::raw_sql(
        r#"
        create table frontend_block_catalog (
            id uuid primary key
        );
        insert into frontend_block_catalog (id) values ('00000000-0000-0000-0000-000000000001');
        "#,
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::raw_sql(FRONTEND_BLOCK_CODE_TEMPLATES_MIGRATION_SQL)
        .execute(&pool)
        .await
        .unwrap();

    let existing_row = sqlx::query(
        r#"
        select code_template, code_template_version, code_template_language, code_modules
        from frontend_block_catalog
        where id = '00000000-0000-0000-0000-000000000001'
        "#,
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        sqlx::Row::get::<Option<String>, _>(&existing_row, "code_template"),
        None
    );
    assert_eq!(
        sqlx::Row::get::<Option<String>, _>(&existing_row, "code_template_version"),
        None
    );
    assert_eq!(
        sqlx::Row::get::<Option<String>, _>(&existing_row, "code_template_language"),
        None
    );
    assert_eq!(
        sqlx::Row::get::<serde_json::Value, _>(&existing_row, "code_modules"),
        json!([])
    );

    let code_modules_column = sqlx::query(
        r#"
        select is_nullable, column_default
        from information_schema.columns
        where table_schema = current_schema()
          and table_name = 'frontend_block_catalog'
          and column_name = 'code_modules'
        "#,
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        sqlx::Row::get::<String, _>(&code_modules_column, "is_nullable"),
        "NO"
    );
    assert_eq!(
        sqlx::Row::get::<Option<String>, _>(&code_modules_column, "column_default").as_deref(),
        Some("'[]'::jsonb")
    );
}

#[tokio::test]
async fn frontend_block_catalog_repository_lists_builtin_and_assigned_workspace_blocks() {
    let (store, workspace, actor) = seed_store().await;
    let installation = PluginRepository::upsert_installation(
        &store,
        &UpsertPluginInstallationInput {
            installation_id: Uuid::now_v7(),
            provider_code: "fixture_frontend_blocks".into(),
            plugin_id: "fixture_frontend_blocks@0.1.0".into(),
            plugin_version: "0.1.0".into(),
            contract_version: "1flowbase.capability/v1".into(),
            protocol: "stdio_json".into(),
            display_name: "Fixture Frontend Blocks".into(),
            source_kind: "uploaded".into(),
            trust_level: "checksum_only".into(),
            verification_status: PluginVerificationStatus::Valid,
            desired_state: PluginDesiredState::ActiveRequested,
            artifact_status: PluginArtifactStatus::Ready,
            runtime_status: PluginRuntimeStatus::Inactive,
            availability_status: PluginAvailabilityStatus::Available,
            package_path: None,
            installed_path: "/tmp/plugins/fixture_frontend_blocks/0.1.0".into(),
            checksum: None,
            manifest_fingerprint: None,
            signature_status: None,
            signature_algorithm: None,
            signing_key_id: None,
            last_load_error: None,
            metadata_json: json!({}),
            actor_user_id: actor.id,
        },
    )
    .await
    .unwrap();

    FrontendBlockCatalogRepository::replace_installation_frontend_blocks(
        &store,
        &ReplaceInstallationFrontendBlocksInput {
            installation_id: installation.id,
            provider_code: installation.provider_code.clone(),
            plugin_id: installation.plugin_id.clone(),
            plugin_version: installation.plugin_version.clone(),
            entries: vec![FrontendBlockCatalogRegistryInput {
                contribution_code: "hero_banner".into(),
                title: "Hero Banner".into(),
                runtime: "iframe".into(),
                entry: "blocks/hero/index.html".into(),
                code_template: Some("export default {}".into()),
                code_template_version: Some("1.0.0".into()),
                code_template_language: Some("tsx".into()),
                code_modules: vec![domain::FrontendBlockCodeModule {
                    source: "@1flowbase/block-sdk".into(),
                    type_declarations: "export declare function defineBlock(): unknown;".into(),
                }],
                context_contract: domain::FrontendBlockContextContract {
                    primitives: vec!["text".into(), "image".into()],
                    input_schema: json!({ "type": "object" }),
                },
                permissions: domain::FrontendBlockPermissions {
                    network: "none".into(),
                    storage: "none".into(),
                    secrets: "none".into(),
                },
                ui_capabilities: vec!["responsive".into()],
            }],
        },
    )
    .await
    .unwrap();

    assert!(
        FrontendBlockCatalogRepository::list_workspace_frontend_blocks(&store, workspace.id)
            .await
            .unwrap()
            .is_empty()
    );

    sqlx::query("update plugin_installations set source_kind = 'builtin' where id = $1")
        .bind(installation.id)
        .execute(store.pool())
        .await
        .unwrap();
    let builtin_entries =
        FrontendBlockCatalogRepository::list_workspace_frontend_blocks(&store, workspace.id)
            .await
            .unwrap();
    assert_eq!(builtin_entries.len(), 1);
    assert_eq!(builtin_entries[0].contribution_code, "hero_banner");

    sqlx::query("update plugin_installations set source_kind = 'uploaded' where id = $1")
        .bind(installation.id)
        .execute(store.pool())
        .await
        .unwrap();

    PluginRepository::create_assignment(
        &store,
        &CreatePluginAssignmentInput {
            installation_id: installation.id,
            workspace_id: workspace.id,
            provider_code: installation.provider_code.clone(),
            actor_user_id: actor.id,
        },
    )
    .await
    .unwrap();

    let entries =
        FrontendBlockCatalogRepository::list_workspace_frontend_blocks(&store, workspace.id)
            .await
            .unwrap();

    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].contribution_code, "hero_banner");
    assert_eq!(
        entries[0].context_contract.primitives,
        vec!["text", "image"]
    );
    assert_eq!(
        entries[0].code_template.as_deref(),
        Some("export default {}")
    );
    assert_eq!(entries[0].code_template_version.as_deref(), Some("1.0.0"));
    assert_eq!(entries[0].code_template_language.as_deref(), Some("tsx"));
    assert_eq!(entries[0].code_modules.len(), 1);

    let code_modules_column = sqlx::query(
        r#"
        select is_nullable, column_default
        from information_schema.columns
        where table_schema = current_schema()
          and table_name = 'frontend_block_catalog'
          and column_name = 'code_modules'
        "#,
    )
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(
        sqlx::Row::get::<String, _>(&code_modules_column, "is_nullable"),
        "NO"
    );
    assert_eq!(
        sqlx::Row::get::<Option<String>, _>(&code_modules_column, "column_default").as_deref(),
        Some("'[]'::jsonb")
    );

    let missing_version = sqlx::query(
        r#"
        update frontend_block_catalog
        set code_template_version = null
        where installation_id = $1
          and contribution_code = 'hero_banner'
        "#,
    )
    .bind(installation.id)
    .execute(store.pool())
    .await;
    assert!(missing_version.is_err());

    let oversized_template = sqlx::query(
        r#"
        update frontend_block_catalog
        set code_template = repeat('x', 262145)
        where installation_id = $1
          and contribution_code = 'hero_banner'
        "#,
    )
    .bind(installation.id)
    .execute(store.pool())
    .await;
    assert!(oversized_template.is_err());

    let unsupported_language = sqlx::query(
        r#"
        update frontend_block_catalog
        set code_template_language = 'javascript'
        where installation_id = $1
          and contribution_code = 'hero_banner'
        "#,
    )
    .bind(installation.id)
    .execute(store.pool())
    .await;
    assert!(unsupported_language.is_err());
}
