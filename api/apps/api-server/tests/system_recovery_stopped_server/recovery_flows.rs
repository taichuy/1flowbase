use super::*;

#[test]
fn status_reads_the_external_journal_without_contacting_the_stopped_api_database() {
    let root = std::env::temp_dir().join(format!("system-recovery-cli-{}", Uuid::now_v7()));
    let repository = root.join("repository");
    let output = Command::new(env!("CARGO_BIN_EXE_system_recovery"))
        .args(["status", "--recovery-job-id", &Uuid::now_v7().to_string()])
        .env("API_ENV", "development")
        .env(
            "API_DATABASE_URL",
            "postgres://stopped-api.invalid:1/server_is_stopped",
        )
        .env("API_DATABASE_POOL_MAX_CONNECTIONS", "1")
        .env("BOOTSTRAP_WORKSPACE_NAME", "offline-recovery-fixture")
        .env("BOOTSTRAP_ROOT_ACCOUNT", "root")
        .env("BOOTSTRAP_ROOT_EMAIL", "root@example.com")
        .env("BOOTSTRAP_ROOT_PASSWORD", "unused-offline")
        .env("BOOTSTRAP_ROOT_NAME", "Root")
        .env("BOOTSTRAP_ROOT_NICKNAME", "Root")
        .env("API_SYSTEM_BACKUP_REPOSITORY_ROOT", &repository)
        .env("API_BUSINESS_FILE_LOCAL_ROOT", root.join("objects"))
        .env("API_PROVIDER_INSTALL_ROOT", root.join("providers"))
        .env("API_MCP_TEMPLATE_LIBRARY_ROOT", root.join("mcp"))
        .env(
            "API_HOST_EXTENSION_DROPIN_ROOT",
            root.join("providers/host-extension/dropins"),
        )
        .output()
        .expect("the recovery binary must be executable without an API server");

    assert!(
        output.status.success(),
        "offline status failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let report: Value = serde_json::from_slice(&output.stdout).expect("status must emit JSON");
    assert_eq!(report["journal_location"], "external_backup_repository");
    assert_eq!(report["journal_event_count"], 0);
    assert_eq!(report["status"], Value::Null);

    fs::remove_dir_all(root).expect("the stopped-server fixture must clean its roots");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn real_stopped_server_recovery_covers_success_resume_rollback_and_manual() {
    let harness = DockerPostgresHarness::start().await;

    let success = RecoveryScenario::create(&harness, ScenarioHealth::Healthy).await;
    let output = success.run_cli("restore").await;
    let report = successful_report(&output);
    assert_eq!(
        report["status"], "succeeded",
        "healthy recovery report: {report}"
    );
    assert_eq!(report["maintenance_fence"], "released");
    success.assert_target_object().await;
    success
        .assert_terminal_audit("system.recovery.succeeded", "succeeded")
        .await;
    success.remove_audit_completion_event().await;
    let output = success.run_cli("resume").await;
    let report = successful_report(&output);
    assert_eq!(report["status"], "succeeded");
    assert_eq!(report["executed_steps"], serde_json::json!([]));
    success.assert_single_terminal_audit().await;
    drop(success);

    let rollback = RecoveryScenario::create(&harness, ScenarioHealth::NonRootTarget).await;
    let output = rollback.run_cli("restore").await;
    let report = successful_report(&output);
    assert_eq!(report["status"], "rolled_back");
    assert_eq!(
        report["failure_code"], "post_restore_health_failed",
        "real login/permission health failure must settle through rollback"
    );
    assert_eq!(report["maintenance_fence"], "released");
    rollback.assert_safety_object().await;
    rollback
        .assert_terminal_audit("system.recovery.rolled_back", "rolled_back")
        .await;
    drop(rollback);

    let manual = RecoveryScenario::create(&harness, ScenarioHealth::NonRootTarget).await;
    let rollback_database = manual.rollback_database_name();
    let hold_url = harness.admin_database_url.clone();
    let held_connection =
        tokio::spawn(
            async move { wait_for_database_connection(&hold_url, &rollback_database).await },
        );
    let output = manual
        .spawn_cli("restore")
        .wait_with_output()
        .await
        .unwrap();
    let held_connection = held_connection
        .await
        .expect("rollback connection task must not panic");
    let report = successful_report(&output);
    assert_eq!(report["status"], "manual_recovery_required");
    assert_eq!(report["maintenance_fence"], "retained");
    held_connection.close().await.unwrap();
    manual.restore_held_rollback_database().await;

    let output = manual.run_cli("resume").await;
    let report = successful_report(&output);
    assert_eq!(report["status"], "manual_recovery_required");
    assert_eq!(report["maintenance_fence"], "retained");
    manual
        .assert_terminal_audit(
            "system.recovery.manual_recovery_required",
            "manual_recovery_required",
        )
        .await;
}

/// This is the fresh-machine half of the recovery contract. The source and target use different
/// PostgreSQL databases and backup repositories; only the downloaded bundle is transferred.
/// Persistent object and artifact paths intentionally use the same in-container locations that
/// official Docker deployments mount on both machines.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn portable_bundle_bootstraps_a_fresh_database_without_the_original_environment() {
    let harness = DockerPostgresHarness::start().await;
    let source = RecoveryScenario::create(&harness, ScenarioHealth::Healthy).await;
    let bundle_root = TemporaryRoot::new("portable-system-backup-bundle");
    let bundle_path = bundle_root.path().join("portable.1fb-backup");
    let source_service = SystemBackupService::new(
        source.repository.clone(),
        Arc::new(EnvironmentBackupKeyProvider::from_master_key(PROVIDER_SECRET).unwrap()),
    );
    source_service
        .download(
            source.target_backup_set_id,
            tokio::fs::File::create(&bundle_path).await.unwrap(),
        )
        .await
        .unwrap();

    let target_database = PostgresTestDatabase::create(&harness.admin_database_url)
        .await
        .expect("fixture must allocate a fresh target database");
    let target_root = TemporaryRoot::new("portable-system-backup-target");
    let target_repository = target_root.path().join("repository");
    tokio::fs::create_dir_all(&target_repository).await.unwrap();

    let deployment_env = target_root.path().join("deployment.env");
    fs::write(
        &deployment_env,
        "API_PROVIDER_SECRET_MASTER_KEY=fresh-target-master-key\nBOOTSTRAP_ROOT_PASSWORD=fresh-root-password\n",
    )
    .unwrap();

    let output = source
        .portable_command(
            "bootstrap",
            target_database.database_url(),
            &target_repository,
            &bundle_path,
            "fresh-target-master-key",
        )
        .arg("--deployment-env-file")
        .arg(&deployment_env)
        .output()
        .await
        .unwrap();
    let report = successful_report(&output);
    assert_eq!(report["status"], "succeeded", "bootstrap report: {report}");
    assert_eq!(report["bootstrap"], true);
    let output_text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !output_text.contains(PROVIDER_SECRET),
        "bootstrap output must not expose recovery material"
    );
    assert_eq!(
        fs::read_to_string(&deployment_env).unwrap(),
        format!(
            "API_PROVIDER_SECRET_MASTER_KEY={PROVIDER_SECRET}\nBOOTSTRAP_ROOT_PASSWORD=fresh-root-password\n"
        ),
        "bootstrap must atomically persist the restored master key without printing it"
    );

    let target_pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(target_database.database_url())
        .await
        .unwrap();
    let root_count: i64 = sqlx::query_scalar(
        "select count(*) from users where default_display_role = 'root' and status = 'active'",
    )
    .fetch_one(&target_pool)
    .await
    .unwrap();
    let target_store = storage_durable_postgres::MainDurableStore::new(target_pool.clone());
    let restored_login = AuthKernel::new(
        target_store.clone(),
        SessionIssuer::new(MemorySessionStore::new("portable-restored-login"), 7),
    )
    .login(LoginCommand {
        login_entry_id: domain::BUILTIN_PASSWORD_LOGIN_ENTRY_ID,
        identifier: "root".to_owned(),
        password: "stopped-server-password".to_owned(),
    })
    .await
    .expect("the restored root password must authenticate through the normal login kernel");
    assert_eq!(restored_login.actor.user_id, source.actor_user_id);
    assert!(
        AuthKernel::new(
            target_store.clone(),
            SessionIssuer::new(MemorySessionStore::new("portable-fresh-login"), 7),
        )
        .login(LoginCommand {
            login_entry_id: domain::BUILTIN_PASSWORD_LOGIN_ENTRY_ID,
            identifier: "root".to_owned(),
            password: "fresh-root-password".to_owned(),
        })
        .await
        .is_err(),
        "the target deployment bootstrap password must not replace restored root credentials"
    );
    assert_eq!(
        DataSourceRepository::get_secret_json(
            &target_store,
            source.secret_inventory.data_source_instance_id,
            PROVIDER_SECRET,
        )
        .await
        .unwrap(),
        Some(serde_json::json!({"token": "data-source-secret"}))
    );
    assert_eq!(
        ModelProviderRepository::get_secret_json(
            &target_store,
            source.secret_inventory.model_provider_instance_id,
            PROVIDER_SECRET,
        )
        .await
        .unwrap(),
        Some(serde_json::json!({"api_key": "provider-secret"}))
    );
    assert_eq!(
        McpManagementRepository::get_mcp_upstream_secret(
            &target_store,
            source.secret_inventory.workspace_id,
            source.secret_inventory.mcp_connection_id,
            PROVIDER_SECRET,
        )
        .await
        .unwrap(),
        Some(serde_json::json!({"bearer_token": "mcp-secret"}))
    );
    target_pool.close().await;
    assert_eq!(
        root_count, 1,
        "restored target must be ready for normal login"
    );
    source.assert_target_object().await;
}
