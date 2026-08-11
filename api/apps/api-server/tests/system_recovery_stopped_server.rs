use std::{
    fs,
    net::TcpListener,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    str::FromStr,
    sync::Arc,
    time::Duration as StdDuration,
};

use api_server::system_backup::{EnvironmentBackupKeyProvider, LocalBackupRepository};
use argon2::{password_hash::SaltString, Argon2, PasswordHasher};
use async_trait::async_trait;
use control_plane::{
    bootstrap::{BootstrapConfig, BootstrapService},
    file_management::{CreateFileStorageCommand, FileStorageService},
    plugin_management::{BackupArtifactDisposition, BackupArtifactEntry, BackupArtifactKind},
    ports::{BackupRepository, ExtensionInstallationRepository, UpsertExtensionInstallationInput},
    system_backup::{
        BackupComponentDescriptor, BackupComponentSource, BackupSourceError,
        CreateSystemBackupCommand, SystemBackupService,
    },
};
use domain::{
    ApplicationBuild, ArtifactRebuildability, BackupComponentDisposition, BackupComponentId,
    BackupComponentKind, BackupComponentRestoreTarget, BackupJournalEvent, BackupJournalEventKind,
    BackupJournalSubject, BackupSetId, BackupSourceIdentity, ContentDigest,
    ExtensionApplicationAction, ExtensionCategory, ExtensionInstallationIdentity,
    ExtensionInstallationStatus, ExtensionSignatureStatus, KeyFingerprint, RecoveryJobId,
    RecoveryJobState, RecoveryStepKind,
};
use postgres_test_support::PostgresTestDatabase;
use rand_core::OsRng;
use serde_json::Value;
use sha2::{Digest, Sha256};
use sqlx::{
    postgres::{PgConnectOptions, PgPoolOptions},
    Connection, PgConnection, Row,
};
use time::{Duration, OffsetDateTime};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use uuid::Uuid;

const DOCKER_IMAGE: &str = "postgres:16-alpine";
const POSTGRES_PASSWORD: &str = "stopped-server-fixture";
const BACKUP_KEY_BASE64: &str = "BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc=";
const PROVIDER_SECRET: &str = "stopped-server-provider-secret";
const SYSTEM_BUILD: &str = "test.stopped-server";
const NODE_ID: &str = "stopped-server-node";

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

fn successful_report(output: &std::process::Output) -> Value {
    assert!(
        output.status.success(),
        "offline recovery failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("recovery CLI must emit terminal JSON")
}

struct DockerPostgresHarness {
    docker: PathBuf,
    container_name: String,
    admin_database_url: String,
    tools: PostgreSqlToolPaths,
    _root: TemporaryRoot,
}

impl DockerPostgresHarness {
    async fn start() -> Self {
        let root = TemporaryRoot::new("system-recovery-postgres-tools");
        let docker = std::env::var_os("SYSTEM_RECOVERY_TEST_DOCKER_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/xp/server/docker/docker"));
        assert!(
            docker.is_file(),
            "set SYSTEM_RECOVERY_TEST_DOCKER_PATH to an executable Docker client"
        );
        let port = reserve_loopback_port();
        let container_name = format!("system-recovery-{}", Uuid::now_v7().simple());
        let output = Command::new(&docker)
            .args([
                "run",
                "--pull=never",
                "--detach",
                "--name",
                &container_name,
                "--publish",
                &format!("127.0.0.1:{port}:5432"),
                "--env",
                &format!("POSTGRES_PASSWORD={POSTGRES_PASSWORD}"),
                DOCKER_IMAGE,
            ])
            .output()
            .expect("Docker must start the isolated PostgreSQL fixture");
        assert!(
            output.status.success(),
            "failed to start PostgreSQL fixture: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let tools = PostgreSqlToolPaths::create(root.path(), &docker);
        let harness = Self {
            docker,
            container_name,
            admin_database_url: format!(
                "postgres://postgres:{POSTGRES_PASSWORD}@127.0.0.1:{port}/postgres"
            ),
            tools,
            _root: root,
        };
        let mut ready = false;
        for _ in 0..120 {
            if let Ok(connection) = PgConnection::connect(&harness.admin_database_url).await {
                connection.close().await.unwrap();
                ready = true;
                break;
            }
            tokio::time::sleep(StdDuration::from_millis(250)).await;
        }
        assert!(
            ready,
            "isolated PostgreSQL TCP endpoint did not become ready"
        );
        harness
    }
}

impl RecoveryScenario {
    fn command(&self, action: &str) -> tokio::process::Command {
        let mut command = tokio::process::Command::new(env!("CARGO_BIN_EXE_system_recovery"));
        command
            .args([
                action,
                "--recovery-job-id",
                &self.recovery_job_id.as_uuid().to_string(),
                "--backup-set-id",
                &self.target_backup_set_id.as_uuid().to_string(),
            ])
            .env("API_ENV", "development")
            .env("API_DATABASE_URL", &self.database_url)
            .env("API_DATABASE_POOL_MAX_CONNECTIONS", "1")
            .env("API_NODE_ID", NODE_ID)
            .env("API_SYSTEM_BUILD_IDENTITY", SYSTEM_BUILD)
            .env("API_PROVIDER_SECRET_MASTER_KEY", PROVIDER_SECRET)
            .env("API_SYSTEM_BACKUP_KEY_BASE64", BACKUP_KEY_BASE64)
            .env("API_SYSTEM_BACKUP_REPOSITORY_ROOT", &self.roots.repository)
            .env("API_BUSINESS_FILE_LOCAL_ROOT", &self.roots.objects)
            .env("API_PROVIDER_INSTALL_ROOT", &self.roots.providers)
            .env("API_MCP_TEMPLATE_LIBRARY_ROOT", &self.roots.mcp)
            .env("API_HOST_EXTENSION_DROPIN_ROOT", &self.roots.host_dropins)
            .env("API_POSTGRES_PG_DUMP_PATH", &self.tools.pg_dump)
            .env("API_POSTGRES_PG_RESTORE_PATH", &self.tools.pg_restore)
            .env("PATH", &self.tools.path)
            .env("BOOTSTRAP_WORKSPACE_NAME", "offline-recovery-fixture")
            .env("BOOTSTRAP_ROOT_ACCOUNT", "root")
            .env("BOOTSTRAP_ROOT_EMAIL", "root@example.com")
            .env("BOOTSTRAP_ROOT_PASSWORD", "unused-offline")
            .env("BOOTSTRAP_ROOT_NAME", "Root")
            .env("BOOTSTRAP_ROOT_NICKNAME", "Root")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        command
    }

    fn spawn_cli(&self, action: &str) -> tokio::process::Child {
        self.command(action)
            .spawn()
            .expect("recovery binary must start with the API server stopped")
    }

    async fn run_cli(&self, action: &str) -> std::process::Output {
        self.command(action).output().await.unwrap()
    }

    async fn assert_target_object(&self) {
        assert_eq!(
            read_object(&self.storage_config, &self.object_path).await,
            b"target-object"
        );
    }

    async fn assert_safety_object(&self) {
        assert_eq!(
            read_object(&self.storage_config, &self.object_path).await,
            b"safety-object"
        );
    }

    async fn assert_terminal_audit(&self, event_code: &str, outcome: &str) {
        let pool = PgPoolOptions::new()
            .max_connections(1)
            .connect(&self.database_url)
            .await
            .unwrap();
        let row = sqlx::query(
            r#"
            select id, actor_user_id, payload
              from audit_logs
             where target_type = 'system_recovery'
               and target_id = $1
               and event_code = $2
            "#,
        )
        .bind(self.recovery_job_id.as_uuid())
        .bind(event_code)
        .fetch_one(&pool)
        .await
        .unwrap();
        let audit_id: Uuid = row.get("id");
        let actor_user_id: Option<Uuid> = row.get("actor_user_id");
        let payload: Value = row.get("payload");
        pool.close().await;
        assert_eq!(actor_user_id, Some(self.actor_user_id));
        assert_eq!(payload["source_event_id"], audit_id.to_string());
        assert_eq!(payload["outcome"], outcome);
        assert_eq!(
            payload["before_snapshot"]["backup_set_id"],
            self.safety_backup_set_id.as_uuid().to_string()
        );
        assert_eq!(
            payload["requested_target_snapshot"]["backup_set_id"],
            self.target_backup_set_id.as_uuid().to_string()
        );
        assert_eq!(
            payload["before_snapshot"]["application_build"],
            SYSTEM_BUILD
        );
        assert!(payload["before_snapshot"]["migration_head"].is_string());
        assert!(payload["before_snapshot"]["components"]
            .as_array()
            .is_some_and(|components| components.iter().all(|component| {
                component["component_id"].is_string()
                    && component["content_digest"].is_string()
                    && component["size_bytes"].is_number()
            })));
        match outcome {
            "succeeded" => assert_eq!(
                payload["effective_after_snapshot"]["backup_set_id"],
                self.target_backup_set_id.as_uuid().to_string()
            ),
            "rolled_back" => assert_eq!(
                payload["effective_after_snapshot"]["backup_set_id"],
                self.safety_backup_set_id.as_uuid().to_string()
            ),
            "manual_recovery_required" => {
                assert!(payload["effective_after_snapshot"].is_null())
            }
            _ => panic!("unexpected recovery outcome"),
        }
        let serialized = payload.to_string();
        let forbidden = [
            PROVIDER_SECRET.to_owned(),
            BACKUP_KEY_BASE64.to_owned(),
            self.roots.objects.to_string_lossy().into_owned(),
            self.roots.providers.to_string_lossy().into_owned(),
        ];
        for secret in &forbidden {
            assert!(
                !serialized.contains(secret),
                "audit payload leaked a secret"
            );
        }
        let events = self.recovery_events().await;
        let terminal_event = events
            .iter()
            .find(|event| {
                matches!(
                    event.event,
                    BackupJournalEventKind::RecoveryStateChanged { state }
                        if state.is_terminal()
                )
            })
            .unwrap();
        assert_eq!(audit_id, terminal_event.event_id);
    }

    async fn assert_single_terminal_audit(&self) {
        let pool = PgPoolOptions::new()
            .max_connections(1)
            .connect(&self.database_url)
            .await
            .unwrap();
        let count = sqlx::query_scalar::<_, i64>(
            "select count(*) from audit_logs where target_type = 'system_recovery' and target_id = $1",
        )
        .bind(self.recovery_job_id.as_uuid())
        .fetch_one(&pool)
        .await
        .unwrap();
        pool.close().await;
        assert_eq!(count, 1, "terminal-event replay must be idempotent");
    }

    async fn recovery_events(&self) -> Vec<BackupJournalEvent> {
        self.repository
            .read_journal(BackupJournalSubject::Recovery(self.recovery_job_id))
            .await
            .unwrap()
    }

    async fn remove_audit_completion_event(&self) {
        let events = self.recovery_events().await;
        let event = events.last().unwrap();
        assert!(matches!(
            event.event,
            BackupJournalEventKind::RecoveryStepCompleted {
                step: RecoveryStepKind::AuditProjection
            }
        ));
        let path = self
            .roots
            .repository
            .join("journal")
            .join(format!("recovery-{}", self.recovery_job_id.as_uuid()))
            .join(format!("{:020}", event.sequence));
        tokio::fs::remove_dir_all(path).await.unwrap();
    }

    fn rollback_database_name(&self) -> String {
        format!(
            "_1flowbase_rollback_{}",
            self.recovery_job_id.as_uuid().simple()
        )
    }

    async fn restore_held_rollback_database(&self) {
        let pool = PgPoolOptions::new()
            .max_connections(1)
            .connect(&self.admin_database_url)
            .await
            .unwrap();
        let rollback = self.rollback_database_name();
        let target = self.database.database_name();
        let target_exists: bool =
            sqlx::query_scalar("select exists(select 1 from pg_database where datname = $1)")
                .bind(target)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert!(
            !target_exists,
            "failed compensation must not claim a target DB"
        );
        sqlx::query(&format!(
            "alter database {} rename to {}",
            quote_identifier(&rollback),
            quote_identifier(target)
        ))
        .execute(&pool)
        .await
        .unwrap();
        pool.close().await;
    }
}

impl Drop for DockerPostgresHarness {
    fn drop(&mut self) {
        let output = Command::new(&self.docker)
            .args(["rm", "--force", &self.container_name])
            .output();
        if let Ok(output) = output {
            if !output.status.success() {
                eprintln!(
                    "failed to remove PostgreSQL fixture {}: {}",
                    self.container_name,
                    String::from_utf8_lossy(&output.stderr)
                );
            }
        }
    }
}

struct PostgreSqlToolPaths {
    pg_dump: PathBuf,
    pg_restore: PathBuf,
    path: String,
}

impl PostgreSqlToolPaths {
    fn create(root: &Path, docker: &Path) -> Self {
        let tools = root.join("tools");
        fs::create_dir_all(&tools).unwrap();
        let pg_dump = tools.join("pg_dump");
        let pg_restore = tools.join("pg_restore");
        write_postgres_wrapper(&pg_dump, docker, "pg_dump");
        write_postgres_wrapper(&pg_restore, docker, "pg_restore");
        let inherited = std::env::var("PATH").unwrap_or_default();
        Self {
            pg_dump,
            pg_restore,
            path: format!("{}:{inherited}", tools.display()),
        }
    }
}

fn write_postgres_wrapper(path: &Path, docker: &Path, tool: &str) {
    let script = format!(
        "#!/bin/sh\nexec '{}' run --pull=never --rm -i --network host --env PGPASSWORD '{}' {} \"$@\"\n",
        docker.display(), DOCKER_IMAGE, tool
    );
    fs::write(path, script).unwrap();
    let mut permissions = fs::metadata(path).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions).unwrap();
}

fn reserve_loopback_port() -> u16 {
    TcpListener::bind(("127.0.0.1", 0))
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

struct TemporaryRoot {
    path: PathBuf,
}

impl TemporaryRoot {
    fn new(prefix: &str) -> Self {
        let path = std::env::temp_dir().join(format!("{prefix}-{}", Uuid::now_v7()));
        fs::create_dir_all(&path).unwrap();
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TemporaryRoot {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.path) {
            if error.kind() != std::io::ErrorKind::NotFound {
                eprintln!("failed to remove {}: {error}", self.path.display());
            }
        }
    }
}

#[derive(Clone, Copy)]
enum ScenarioHealth {
    Healthy,
    NonRootTarget,
}

struct RecoveryScenario {
    database: PostgresTestDatabase,
    repository: Arc<LocalBackupRepository>,
    database_url: String,
    admin_database_url: String,
    tools: PostgreSqlToolPaths,
    recovery_job_id: RecoveryJobId,
    target_backup_set_id: BackupSetId,
    safety_backup_set_id: BackupSetId,
    actor_user_id: Uuid,
    storage_config: Value,
    object_path: String,
    roots: ScenarioRoots,
    _root: TemporaryRoot,
}

struct ScenarioRoots {
    repository: PathBuf,
    objects: PathBuf,
    providers: PathBuf,
    mcp: PathBuf,
    host_dropins: PathBuf,
}

impl RecoveryScenario {
    async fn create(harness: &DockerPostgresHarness, health: ScenarioHealth) -> Self {
        let root = TemporaryRoot::new("system-recovery-stopped-server");
        let roots = ScenarioRoots {
            repository: root.path().join("repository"),
            objects: root.path().join("objects"),
            providers: root.path().join("providers"),
            mcp: root.path().join("mcp"),
            host_dropins: root.path().join("providers/host-extension/dropins"),
        };
        for path in [
            &roots.repository,
            &roots.objects,
            &roots.providers,
            &roots.mcp,
            &roots.host_dropins,
        ] {
            tokio::fs::create_dir_all(path).await.unwrap();
        }
        let database = PostgresTestDatabase::create(&harness.admin_database_url)
            .await
            .expect("fixture must allocate a whole isolated database");
        let database_url = database.database_url().to_owned();
        let runtime =
            storage_durable::build_main_durable_postgres_with_max_connections(&database_url, 2)
                .await
                .unwrap();
        let store = runtime.store;
        let salt = SaltString::generate(&mut OsRng);
        let root_password_hash = Argon2::default()
            .hash_password(b"stopped-server-password", &salt)
            .unwrap()
            .to_string();
        let bootstrap = BootstrapService::new(store.clone())
            .run(&BootstrapConfig {
                workspace_name: "Stopped server".to_owned(),
                root_account: "root".to_owned(),
                root_email: "root@example.com".to_owned(),
                root_password_hash,
                root_name: "Root".to_owned(),
                root_nickname: "target".to_owned(),
            })
            .await
            .unwrap();
        let storage_config = serde_json::json!({
            "root_path": roots.objects.display().to_string()
        });
        let storage = FileStorageService::new(store.clone())
            .create_storage(CreateFileStorageCommand {
                actor_user_id: bootstrap.root_user_id,
                code: "recovery_fixture".to_owned(),
                title: "Recovery fixture".to_owned(),
                driver_type: "local".to_owned(),
                enabled: true,
                is_default: true,
                config_json: storage_config.clone(),
                rule_json: serde_json::json!({}),
            })
            .await
            .unwrap();
        let storage_id = storage.id;
        let artifact_bytes = b"builtin-mcp-artifact";
        let artifact_path = roots.mcp.join("builtin-fixture.bundle");
        tokio::fs::write(&artifact_path, artifact_bytes)
            .await
            .unwrap();
        let artifact_checksum = format!("sha256:{:x}", Sha256::digest(artifact_bytes));
        ExtensionInstallationRepository::upsert_extension_installation(
            &store,
            &UpsertExtensionInstallationInput {
                installation_id: Uuid::now_v7(),
                identity: ExtensionInstallationIdentity {
                    category: ExtensionCategory::Mcp,
                    organization: "acme".to_owned(),
                    artifact_id: "builtin-fixture".to_owned(),
                    version: "1.0.0".to_owned(),
                },
                node_id: NODE_ID.to_owned(),
                source_kind: "builtin".to_owned(),
                trust_level: "verified_official".to_owned(),
                local_path: artifact_path.display().to_string(),
                expected_checksum: Some(artifact_checksum.clone()),
                local_checksum: artifact_checksum,
                signature_status: ExtensionSignatureStatus::Verified,
                signature_algorithm: None,
                signing_key_id: None,
                warnings: Vec::new(),
                receipt: serde_json::json!({"fixture": "stopped_server_recovery"}),
                application_action: ExtensionApplicationAction::None,
                status: ExtensionInstallationStatus::Installed,
                is_current: true,
                created_by: bootstrap.root_user_id,
            },
        )
        .await
        .unwrap();

        let object_path = "recovery/fixture.bin".to_owned();
        put_object(&storage_config, &object_path, b"target-object").await;
        let repository = Arc::new(
            LocalBackupRepository::open(
                &roots.repository,
                &[
                    roots.objects.clone(),
                    roots.providers.clone(),
                    roots.mcp.clone(),
                    roots.host_dropins.clone(),
                ],
            )
            .await
            .unwrap(),
        );
        let key_provider =
            Arc::new(EnvironmentBackupKeyProvider::from_base64(BACKUP_KEY_BASE64).unwrap());
        let toolchain = storage_durable::PostgreSqlToolchain::discover(
            &harness.tools.pg_dump,
            &harness.tools.pg_restore,
        )
        .await
        .unwrap();
        let migration_head = storage_durable::migration_head(store.pool()).await.unwrap();
        let master_key_fingerprint =
            KeyFingerprint::try_from(format!("{:x}", Sha256::digest(PROVIDER_SECRET.as_bytes())))
                .unwrap();
        if matches!(health, ScenarioHealth::NonRootTarget) {
            sqlx::query("update roles set code = 'broken_root' where code = 'root'")
                .execute(store.pool())
                .await
                .unwrap();
        }
        let service = SystemBackupService::new(repository.clone(), key_provider.clone());
        let target = service
            .create(
                CreateSystemBackupCommand {
                    actor_user_id: bootstrap.root_user_id,
                    application_build: ApplicationBuild::try_from(SYSTEM_BUILD).unwrap(),
                    migration_head: migration_head.clone(),
                    master_key_fingerprint: master_key_fingerprint.clone(),
                },
                recovery_sources(
                    &database_url,
                    &toolchain,
                    storage_id,
                    &object_path,
                    b"target-object",
                ),
            )
            .await
            .unwrap();
        if matches!(health, ScenarioHealth::NonRootTarget) {
            sqlx::query("update roles set code = 'root' where code = 'broken_root'")
                .execute(store.pool())
                .await
                .unwrap();
        }
        sqlx::query("update users set nickname = 'safety' where id = $1")
            .bind(bootstrap.root_user_id)
            .execute(store.pool())
            .await
            .unwrap();
        put_object(&storage_config, &object_path, b"safety-object").await;
        let safety = service
            .create(
                CreateSystemBackupCommand {
                    actor_user_id: bootstrap.root_user_id,
                    application_build: ApplicationBuild::try_from(SYSTEM_BUILD).unwrap(),
                    migration_head,
                    master_key_fingerprint,
                },
                recovery_sources(
                    &database_url,
                    &toolchain,
                    storage_id,
                    &object_path,
                    b"safety-object",
                ),
            )
            .await
            .unwrap();
        let recovery_job_id = RecoveryJobId::new();
        seed_recovery_journal(
            repository.as_ref(),
            recovery_job_id,
            target.manifest().backup_set_id(),
            safety.manifest().backup_set_id(),
            bootstrap.root_user_id,
        )
        .await;
        store.pool().close().await;

        Self {
            database,
            repository,
            database_url,
            admin_database_url: harness.admin_database_url.clone(),
            tools: PostgreSqlToolPaths {
                pg_dump: harness.tools.pg_dump.clone(),
                pg_restore: harness.tools.pg_restore.clone(),
                path: harness.tools.path.clone(),
            },
            recovery_job_id,
            target_backup_set_id: target.manifest().backup_set_id(),
            safety_backup_set_id: safety.manifest().backup_set_id(),
            actor_user_id: bootstrap.root_user_id,
            storage_config,
            object_path,
            roots,
            _root: root,
        }
    }
}

struct BytesBackupSource {
    descriptor: BackupComponentDescriptor,
    bytes: Vec<u8>,
}

#[async_trait]
impl BackupComponentSource for BytesBackupSource {
    fn descriptor(&self) -> BackupComponentDescriptor {
        self.descriptor.clone()
    }

    async fn write_to(
        &self,
        mut destination: control_plane::ports::BackupComponentWriter,
    ) -> Result<(), BackupSourceError> {
        destination
            .write_all(&self.bytes)
            .await
            .map_err(|_| BackupSourceError::Unavailable)?;
        destination
            .shutdown()
            .await
            .map_err(|_| BackupSourceError::Unavailable)
    }
}

fn recovery_sources(
    database_url: &str,
    toolchain: &storage_durable::PostgreSqlToolchain,
    storage_id: Uuid,
    object_path: &str,
    object_bytes: &[u8],
) -> Vec<Arc<dyn BackupComponentSource>> {
    vec![
        Arc::new(storage_durable::PostgreSqlLogicalBackup::new(
            database_url,
            toolchain.clone(),
        )),
        Arc::new(BytesBackupSource {
            descriptor: BackupComponentDescriptor {
                component_id: BackupComponentId::try_from("business-object-fixture").unwrap(),
                kind: BackupComponentKind::BusinessObject,
                source_identity: BackupSourceIdentity::try_from(format!(
                    "business-object:{storage_id}/{object_path}"
                ))
                .unwrap(),
                content_type: "application/octet-stream".to_owned(),
                disposition: BackupComponentDisposition::Embedded,
                rebuildability: ArtifactRebuildability::NotApplicable,
                restore_target: BackupComponentRestoreTarget::BusinessObject {
                    storage_id,
                    object_path: object_path.to_owned(),
                },
            },
            bytes: object_bytes.to_vec(),
        }),
        Arc::new(BackupArtifactEntry {
            identity: "extension:mcp/acme/builtin-fixture@1.0.0".to_owned(),
            kind: BackupArtifactKind::Mcp,
            category: "mcp".to_owned(),
            organization: "acme".to_owned(),
            artifact_id: "builtin-fixture".to_owned(),
            source_kind: "builtin".to_owned(),
            version: "1.0.0".to_owned(),
            expected_checksum: None,
            disposition: BackupArtifactDisposition::RebuildableIdentity,
            artifact_path: None,
        }),
    ]
}

async fn put_object(config_json: &Value, object_path: &str, bytes: &[u8]) {
    let registry = storage_object::builtin_driver_registry();
    let driver = registry.get("local").unwrap();
    driver
        .put_object_stream(storage_object::FileStoragePutStreamInput {
            config_json,
            object_path,
            content_type: Some("application/octet-stream"),
            content_length: bytes.len() as u64,
            reader: Box::pin(std::io::Cursor::new(bytes.to_vec())),
        })
        .await
        .unwrap();
}

async fn read_object(config_json: &Value, object_path: &str) -> Vec<u8> {
    let registry = storage_object::builtin_driver_registry();
    let driver = registry.get("local").unwrap();
    let mut opened = driver
        .open_read_stream(storage_object::OpenReadInput {
            config_json,
            object_path,
        })
        .await
        .unwrap();
    let mut bytes = Vec::new();
    opened.reader.read_to_end(&mut bytes).await.unwrap();
    driver
        .verify_read_unchanged(storage_object::VerifyReadUnchangedInput {
            config_json,
            object_path,
            snapshot: &opened.snapshot,
        })
        .await
        .unwrap();
    bytes
}

async fn seed_recovery_journal(
    repository: &dyn BackupRepository,
    recovery_job_id: RecoveryJobId,
    target_backup_set_id: BackupSetId,
    safety_backup_set_id: BackupSetId,
    actor_user_id: Uuid,
) {
    let plan_digest = ContentDigest::try_from("a".repeat(64)).unwrap();
    let intent_id = Uuid::now_v7();
    let events = vec![
        BackupJournalEventKind::RecoveryStateChanged {
            state: RecoveryJobState::Preflight,
        },
        BackupJournalEventKind::RecoveryStateChanged {
            state: RecoveryJobState::AwaitingConfirmation,
        },
        BackupJournalEventKind::RecoveryIntentConfirmed {
            intent_id,
            target_backup_set_id,
            plan_digest: plan_digest.clone(),
            confirmed_at: OffsetDateTime::now_utc(),
            expires_at: OffsetDateTime::now_utc() + Duration::minutes(5),
        },
        BackupJournalEventKind::RecoveryStateChanged {
            state: RecoveryJobState::SafetyBackup,
        },
        BackupJournalEventKind::RecoverySafetyBackupVerified {
            safety_backup_set_id,
            plan_digest: plan_digest.clone(),
        },
        BackupJournalEventKind::RecoveryStateChanged {
            state: RecoveryJobState::Fencing,
        },
        BackupJournalEventKind::RecoveryStateChanged {
            state: RecoveryJobState::Draining,
        },
        BackupJournalEventKind::RecoveryOfflineHandoffReady {
            target_backup_set_id,
            safety_backup_set_id,
            plan_digest,
        },
    ];
    for (sequence, event) in events.into_iter().enumerate() {
        repository
            .append_journal_event(&BackupJournalEvent {
                event_id: Uuid::now_v7(),
                sequence: sequence as u64,
                subject: BackupJournalSubject::Recovery(recovery_job_id),
                backup_set_id: target_backup_set_id,
                actor_user_id: Some(actor_user_id),
                occurred_at: OffsetDateTime::now_utc(),
                event,
            })
            .await
            .unwrap();
    }
}

async fn wait_for_database_connection(
    admin_database_url: &str,
    database_name: &str,
) -> PgConnection {
    let started = std::time::Instant::now();
    loop {
        let options = PgConnectOptions::from_str(admin_database_url)
            .unwrap()
            .database(database_name);
        if let Ok(connection) = PgConnection::connect_with(&options).await {
            return connection;
        }
        assert!(
            started.elapsed() < StdDuration::from_secs(30),
            "recovery never created rollback database {database_name}"
        );
        tokio::time::sleep(StdDuration::from_millis(5)).await;
    }
}

fn quote_identifier(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}
