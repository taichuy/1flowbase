use control_plane::ports::{ExtensionInstallationRepository, UpsertExtensionInstallationInput};
use storage_postgres::{run_migrations, PgControlPlaneStore};
use uuid::Uuid;

fn base_database_url() -> String {
    std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:1flowbase@127.0.0.1:35432/1flowbase".into())
}

async fn seed_store() -> (PgControlPlaneStore, domain::UserRecord) {
    let database = postgres_test_support::PostgresTestSchema::create(&base_database_url())
        .await
        .unwrap();
    let pool = database.connect().await.unwrap();
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
            public_ui_block: String::new(),
            options: serde_json::json!({}),
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
    (store, actor)
}

fn input(actor_user_id: Uuid) -> UpsertExtensionInstallationInput {
    UpsertExtensionInstallationInput {
        installation_id: Uuid::now_v7(),
        identity: domain::ExtensionInstallationIdentity {
            category: domain::ExtensionCategory::AgentFlow,
            organization: "taichuy".into(),
            artifact_id: "fixture-provider".into(),
            version: "1.0.0".into(),
        },
        node_id: "node-a".into(),
        source_kind: "official_repository".into(),
        trust_level: "verified_official".into(),
        local_path: "/tmp/api/plugins/installed/runtime-extensions/@taichuy/fixture-provider/1.0.0/artifact.bin".into(),
        expected_checksum: Some("sha256:fixture".into()),
        local_checksum: "sha256:fixture".into(),
        signature_status: domain::ExtensionSignatureStatus::Verified,
        signature_algorithm: Some("ed25519".into()),
        signing_key_id: Some("official-2026".into()),
        warnings: Vec::new(),
        receipt: serde_json::json!({"kind": "install"}),
        application_action: domain::ExtensionApplicationAction::ConfigureModelProvider,
        status: domain::ExtensionInstallationStatus::Installed,
        is_current: true,
        created_by: actor_user_id,
    }
}

#[tokio::test]
async fn root_1545_bf1_repository_lists_newest_stable_identity_version_first() {
    let (store, actor) = seed_store().await;
    let mut older = input(actor.id);
    older.identity.version = "1.1.0".to_string();
    ExtensionInstallationRepository::upsert_extension_installation(&store, &older)
        .await
        .unwrap();
    let mut newer = input(actor.id);
    newer.identity.version = "1.2.0".to_string();
    ExtensionInstallationRepository::upsert_extension_installation(&store, &newer)
        .await
        .unwrap();

    let records =
        ExtensionInstallationRepository::list_extension_installations_for_node(&store, "node-a")
            .await
            .unwrap();
    assert_eq!(records[0].identity.version, "1.2.0");
    assert_eq!(records[1].identity.version, "1.1.0");
}

#[tokio::test]
async fn root_1545_extension_repository_upserts_stable_identity_and_keeps_source_trust_separate() {
    let (store, actor) = seed_store().await;
    let first =
        ExtensionInstallationRepository::upsert_extension_installation(&store, &input(actor.id))
            .await
            .unwrap();
    let mut update = input(actor.id);
    update.source_kind = "uploaded".into();
    update.trust_level = "checksum_only".into();
    update.local_path = "/tmp/local-development-artifact".into();
    let second = ExtensionInstallationRepository::upsert_extension_installation(&store, &update)
        .await
        .unwrap();

    assert_eq!(first.id, second.id);
    assert_eq!(second.source_kind, "uploaded");
    assert_eq!(second.trust_level, "checksum_only");
    assert_eq!(
        second.local_path.as_deref(),
        Some("/tmp/local-development-artifact")
    );
    assert_eq!(
        ExtensionInstallationRepository::list_extension_installations_for_node(&store, "node-a")
            .await
            .unwrap()
            .len(),
        1
    );

    ExtensionInstallationRepository::set_extension_installation_status(
        &store,
        "node-a",
        second.id,
        domain::ExtensionInstallationStatus::Missing,
    )
    .await
    .unwrap();
    let missing = ExtensionInstallationRepository::find_extension_installation(
        &store,
        "node-a",
        &second.identity,
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(missing.status, domain::ExtensionInstallationStatus::Missing);
}

#[tokio::test]
async fn ac_002_repository_install_select_and_remove_maintain_one_explicit_current_version() {
    let (store, actor) = seed_store().await;
    let mut older = input(actor.id);
    older.identity.version = "1.1.0".into();
    let older = ExtensionInstallationRepository::upsert_extension_installation(&store, &older)
        .await
        .unwrap();
    let mut newer = input(actor.id);
    newer.identity.version = "1.2.0".into();
    let newer = ExtensionInstallationRepository::upsert_extension_installation(&store, &newer)
        .await
        .unwrap();

    assert!(newer.is_current);
    assert!(
        !ExtensionInstallationRepository::find_extension_installation(
            &store,
            "node-a",
            &older.identity
        )
        .await
        .unwrap()
        .unwrap()
        .is_current
    );

    let selected = ExtensionInstallationRepository::select_current_extension_installation(
        &store, "node-a", older.id,
    )
    .await
    .unwrap()
    .unwrap();
    assert!(selected.is_current);
    let blocked =
        ExtensionInstallationRepository::extension_deletion_decision(&store, "node-a", older.id)
            .await
            .unwrap()
            .unwrap();
    assert!(!blocked.deletable);
    assert_eq!(blocked.reasons, vec!["current_version"]);

    ExtensionInstallationRepository::select_current_extension_installation(
        &store, "node-a", newer.id,
    )
    .await
    .unwrap()
    .unwrap();

    ExtensionInstallationRepository::remove_extension_installation(&store, "node-a", older.id)
        .await
        .unwrap()
        .unwrap();
    let remaining = ExtensionInstallationRepository::find_extension_installation(
        &store,
        "node-a",
        &newer.identity,
    )
    .await
    .unwrap()
    .unwrap();
    assert!(remaining.is_current);
}

#[tokio::test]
async fn issue_1566_terminal_tasks_do_not_block_deletion_but_active_tasks_do() {
    use control_plane::ports::{CreatePluginTaskInput, PluginRepository};

    let (store, actor) = seed_store().await;
    let mut older = input(actor.id);
    older.identity.version = "1.0.0".into();
    let older = ExtensionInstallationRepository::upsert_extension_installation(&store, &older)
        .await
        .unwrap();
    let mut newer = input(actor.id);
    newer.identity.version = "2.0.0".into();
    ExtensionInstallationRepository::upsert_extension_installation(&store, &newer)
        .await
        .unwrap();

    PluginRepository::create_task(
        &store,
        &CreatePluginTaskInput {
            task_id: Uuid::now_v7(),
            installation_id: Some(older.id),
            workspace_id: None,
            provider_code: "fixture-provider".into(),
            task_kind: domain::PluginTaskKind::Uninstall,
            status: domain::PluginTaskStatus::Succeeded,
            status_message: None,
            detail_json: serde_json::json!({}),
            actor_user_id: Some(actor.id),
        },
    )
    .await
    .unwrap();
    let terminal =
        ExtensionInstallationRepository::extension_deletion_decision(&store, "node-a", older.id)
            .await
            .unwrap()
            .unwrap();
    assert!(
        terminal.deletable,
        "terminal task history must be retained without blocking deletion"
    );

    PluginRepository::create_task(
        &store,
        &CreatePluginTaskInput {
            task_id: Uuid::now_v7(),
            installation_id: Some(older.id),
            workspace_id: None,
            provider_code: "fixture-provider".into(),
            task_kind: domain::PluginTaskKind::Uninstall,
            status: domain::PluginTaskStatus::Running,
            status_message: None,
            detail_json: serde_json::json!({}),
            actor_user_id: Some(actor.id),
        },
    )
    .await
    .unwrap();
    let active =
        ExtensionInstallationRepository::extension_deletion_decision(&store, "node-a", older.id)
            .await
            .unwrap()
            .unwrap();
    assert!(!active.deletable);
    assert!(active.reasons.contains(&"active_task".to_string()));
}

#[tokio::test]
async fn issue_1566_system_reserved_installations_cannot_be_deleted() {
    let (store, actor) = seed_store().await;
    let mut reserved = input(actor.id);
    reserved.source_kind = "builtin".into();
    reserved.identity.version = "builtin".into();
    let reserved =
        ExtensionInstallationRepository::upsert_extension_installation(&store, &reserved)
            .await
            .unwrap();
    let mut newer = input(actor.id);
    newer.identity.version = "2.0.0".into();
    ExtensionInstallationRepository::upsert_extension_installation(&store, &newer)
        .await
        .unwrap();

    let decision =
        ExtensionInstallationRepository::extension_deletion_decision(&store, "node-a", reserved.id)
            .await
            .unwrap()
            .unwrap();
    assert!(!decision.deletable);
    assert!(decision.reasons.contains(&"system_reserved".to_string()));
    assert!(
        ExtensionInstallationRepository::remove_extension_installation(
            &store,
            "node-a",
            reserved.id,
        )
        .await
        .is_err()
    );
}
