use std::{
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU8, Ordering},
        Arc,
    },
};

use anyhow::{anyhow, bail, Context, Result};
use api_server::{
    config::ApiConfig,
    system_backup::{
        discover_postgres_toolchain, EnvironmentBackupKeyProvider, LocalBackupRepository,
        PostgreSqlPostRestoreHealthVerifier, PostgreSqlPostRestoreReconciler,
        PostgreSqlRecoveryAuditProjector, StoppedServerRecoveryEphemeralState,
    },
};
use async_trait::async_trait;
use control_plane::{
    file_management::{
        BusinessObjectRecoveryTarget, RecoveryObjectStorage, RecoveryObjectStorageResolver,
    },
    plugin_management::{
        ArtifactRecoveryCoordinate, ArtifactRecoveryResolver, FilesystemArtifactRecoveryTarget,
    },
    ports::{BackupRepository, BackupRepositoryError},
    system_backup::{read_backup_bundle_manifest, recover_source_master_key, SystemBackupService},
    system_recovery::{
        ExecuteOfflineRecoveryCommand, OfflineRecoveryExecutor, OfflineRecoveryTargets,
        PostRestoreRecoveryError, PostRestoreRecoveryOutcome, PostRestoreRecoveryService,
        RecoveryCompletionLease, RecoveryStepTargetError,
    },
};
use domain::{
    BackupJournalEvent, BackupJournalEventKind, BackupJournalSubject, BackupSetId, ContentDigest,
    KeyFingerprint, RecoveryJobId, RecoveryJobState,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::{postgres::PgPoolOptions, Row};
use storage_durable::PostgreSqlRecoveryTarget;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Command {
    Import,
    SourceMaster,
    Bootstrap,
    Preflight,
    Status,
    Restore,
    Resume,
    Rollback,
    Finalize,
    Report,
}

struct Arguments {
    command: Command,
    backup_set_id: Option<BackupSetId>,
    recovery_job_id: Option<RecoveryJobId>,
    backup_file: Option<PathBuf>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let arguments = parse_arguments(std::env::args().skip(1))?;
    let config = ApiConfig::from_env()?;
    let repository = open_repository(&config).await?;
    let key_provider = Arc::new(
        EnvironmentBackupKeyProvider::from_master_key_with_legacy(
            &config.provider_secret_master_key,
            config.legacy_system_backup_key_base64.as_deref(),
        )
        .map_err(|_| anyhow!("system backup key is unavailable"))?,
    );

    let output = match arguments.command {
        Command::SourceMaster => {
            let material = source_master(key_provider, required_backup_file(&arguments)?).await?;
            print!("{material}");
            return Ok(());
        }
        Command::Import => {
            import_backup(repository, key_provider, required_backup_file(&arguments)?).await?
        }
        Command::Bootstrap => {
            bootstrap_from_file(
                &config,
                repository,
                key_provider,
                required_backup_file(&arguments)?,
            )
            .await?
        }
        Command::Preflight => {
            preflight(
                &config,
                repository.clone(),
                key_provider,
                required_backup_set_id(&arguments)?,
            )
            .await?
        }
        Command::Status => {
            status(repository.as_ref(), required_recovery_job_id(&arguments)?).await?
        }
        Command::Report => {
            report(repository.as_ref(), required_recovery_job_id(&arguments)?).await?
        }
        Command::Restore | Command::Resume | Command::Rollback | Command::Finalize => {
            let command = ExecuteOfflineRecoveryCommand {
                recovery_job_id: required_recovery_job_id(&arguments)?,
                backup_set_id: required_backup_set_id(&arguments)?,
            };
            let backup_password = std::env::var("API_SYSTEM_BACKUP_PASSWORD")
                .ok()
                .filter(|value| !value.is_empty());
            let runtime =
                build_recovery_runtime(&config, repository, key_provider, backup_password).await?;
            match arguments.command {
                Command::Restore | Command::Resume => runtime.restore_or_resume(command).await?,
                Command::Rollback => {
                    runtime.executor.rollback_promoted_targets(command).await?;
                    json!({"status": "rolled_back", "recovery_job_id": command.recovery_job_id})
                }
                Command::Finalize => {
                    runtime.executor.finalize_promoted_targets(command).await?;
                    json!({"status": "finalized", "recovery_job_id": command.recovery_job_id})
                }
                _ => unreachable!(),
            }
        }
    };
    println!("{}", serde_json::to_string_pretty(&output)?);
    Ok(())
}

fn parse_arguments(arguments: impl IntoIterator<Item = String>) -> Result<Arguments> {
    let mut arguments = arguments.into_iter();
    let Some(command) = arguments.next() else {
        print_usage();
        bail!("a recovery command is required");
    };
    if matches!(command.as_str(), "help" | "--help" | "-h") {
        print_usage();
        std::process::exit(0);
    }
    let command = match command.as_str() {
        "import" => Command::Import,
        "source-master" => Command::SourceMaster,
        "bootstrap" => Command::Bootstrap,
        "preflight" => Command::Preflight,
        "status" => Command::Status,
        "restore" => Command::Restore,
        "resume" => Command::Resume,
        "rollback" => Command::Rollback,
        "finalize" => Command::Finalize,
        "report" => Command::Report,
        _ => bail!("unknown recovery command `{command}`"),
    };
    let mut backup_set_id = None;
    let mut recovery_job_id = None;
    let mut backup_file = None;
    while let Some(flag) = arguments.next() {
        let value = arguments
            .next()
            .with_context(|| format!("missing value for {flag}"))?;
        match flag.as_str() {
            "--backup-set-id" => {
                backup_set_id = Some(BackupSetId::from_uuid(parse_uuid(&value, &flag)?));
            }
            "--recovery-job-id" => {
                recovery_job_id = Some(RecoveryJobId::from_uuid(parse_uuid(&value, &flag)?));
            }
            "--backup-file" => backup_file = Some(PathBuf::from(value)),
            _ => bail!("unknown recovery option `{flag}`"),
        }
    }
    Ok(Arguments {
        command,
        backup_set_id,
        recovery_job_id,
        backup_file,
    })
}

fn parse_uuid(value: &str, flag: &str) -> Result<Uuid> {
    Uuid::parse_str(value).with_context(|| format!("invalid UUID for {flag}"))
}

fn required_backup_set_id(arguments: &Arguments) -> Result<BackupSetId> {
    arguments
        .backup_set_id
        .context("--backup-set-id is required for this command")
}

fn required_recovery_job_id(arguments: &Arguments) -> Result<RecoveryJobId> {
    arguments
        .recovery_job_id
        .context("--recovery-job-id is required for this command")
}

fn required_backup_file(arguments: &Arguments) -> Result<PathBuf> {
    arguments
        .backup_file
        .clone()
        .context("--backup-file is required")
}

fn print_usage() {
    eprintln!(
        "usage: system_recovery <source-master|import|bootstrap|preflight|status|restore|resume|rollback|finalize|report> \
         [--backup-file <path>] [--backup-set-id <uuid>] [--recovery-job-id <uuid>]"
    );
}

/// Restore a backup into a deliberately fresh deployment before the API service is started.
///
/// Unlike an operator-initiated recovery this has no running application to fence and no target
/// data worth a safety backup.  It is intentionally only exposed through the offline binary: the
/// caller must provide the mounted backup file and the target database must be empty.  The normal
/// executor and post-restore service still own journal, rollback, reconciliation and health.
async fn bootstrap_from_file(
    config: &ApiConfig,
    repository: Arc<LocalBackupRepository>,
    key_provider: Arc<EnvironmentBackupKeyProvider>,
    backup_file: PathBuf,
) -> Result<Value> {
    ensure_fresh_recovery_target(config).await?;
    let imported = import_backup(repository.clone(), key_provider.clone(), backup_file).await?;
    let backup_set_id = imported["backup_set_id"]
        .as_str()
        .context("portable backup import returned no backup set")
        .and_then(|value| Uuid::parse_str(value).context("portable backup set id is invalid"))
        .map(BackupSetId::from_uuid)?;

    // Verify compatibility before any target data is replaced.  This also makes unsupported
    // formats and migration paths fail closed in the deployment path.
    preflight(
        config,
        repository.clone(),
        key_provider.clone(),
        backup_set_id,
    )
    .await?;
    let recovery_job_id = RecoveryJobId::new();
    append_bootstrap_journal(repository.as_ref(), recovery_job_id, backup_set_id).await?;
    let password = std::env::var("API_SYSTEM_BACKUP_PASSWORD")
        .ok()
        .filter(|value| !value.is_empty());
    let runtime = build_recovery_runtime(config, repository, key_provider, password).await?;
    let receipt = runtime
        .restore_bootstrap(
            ExecuteOfflineRecoveryCommand {
                recovery_job_id,
                backup_set_id,
            },
            &config.database_url,
        )
        .await?;
    Ok(json!({
        "status": receipt["status"],
        "backup_set_id": backup_set_id,
        "recovery_job_id": recovery_job_id,
        "bootstrap": true,
    }))
}

async fn ensure_fresh_recovery_target(config: &ApiConfig) -> Result<()> {
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&config.database_url)
        .await
        .context("bootstrap recovery target database is unavailable")?;
    // A 1flowbase database always has at least one application relation after normal startup.
    // Refuse an existing target instead of silently overwriting a second deployment.
    let existing: i64 = sqlx::query_scalar(
        "select count(*) from information_schema.tables where table_schema = 'public' and table_type = 'BASE TABLE'",
    )
    .fetch_one(&pool)
    .await
    .context("could not inspect bootstrap recovery target")?;
    pool.close().await;
    if existing != 0 {
        bail!("bootstrap recovery requires a fresh target database")
    }
    Ok(())
}

async fn append_bootstrap_journal(
    repository: &dyn BackupRepository,
    recovery_job_id: RecoveryJobId,
    backup_set_id: BackupSetId,
) -> Result<()> {
    // A fresh deployment has no user session or pre-existing data to protect. The journal still
    // follows the offline executor's ordered handoff contract so that resume, rollback and the
    // post-restore checks use the same durable state machine as an operator initiated recovery.
    // The target backup doubles as the immutable bootstrap baseline; it is never presented as a
    // user-facing safety backup because a fresh database has no prior application state.
    let plan_digest = ContentDigest::try_from("0".repeat(64))
        .map_err(|_| anyhow!("could not initialize bootstrap recovery plan"))?;
    let events = [
        BackupJournalEventKind::RecoveryStateChanged {
            state: RecoveryJobState::Preflight,
        },
        BackupJournalEventKind::RecoveryStateChanged {
            state: RecoveryJobState::AwaitingConfirmation,
        },
        BackupJournalEventKind::RecoveryStateChanged {
            state: RecoveryJobState::SafetyBackup,
        },
        BackupJournalEventKind::RecoverySafetyBackupVerified {
            safety_backup_set_id: backup_set_id,
            plan_digest: plan_digest.clone(),
        },
        BackupJournalEventKind::RecoveryStateChanged {
            state: RecoveryJobState::Fencing,
        },
        BackupJournalEventKind::RecoveryStateChanged {
            state: RecoveryJobState::Draining,
        },
        BackupJournalEventKind::RecoveryOfflineHandoffReady {
            target_backup_set_id: backup_set_id,
            safety_backup_set_id: backup_set_id,
            plan_digest,
        },
    ];
    for (sequence, event) in events.into_iter().enumerate() {
        repository
            .append_journal_event(&BackupJournalEvent {
                event_id: Uuid::now_v7(),
                sequence: sequence as u64,
                subject: BackupJournalSubject::Recovery(recovery_job_id),
                backup_set_id,
                // The restored database may not have a root user until the PostgreSQL step has
                // completed. A stable system actor keeps this pre-start journal self-contained.
                actor_user_id: Some(Uuid::nil()),
                occurred_at: time::OffsetDateTime::now_utc(),
                event,
            })
            .await
            .map_err(|_| anyhow!("could not initialize external bootstrap recovery journal"))?;
    }
    Ok(())
}

async fn import_backup(
    repository: Arc<LocalBackupRepository>,
    key_provider: Arc<EnvironmentBackupKeyProvider>,
    backup_file: PathBuf,
) -> Result<Value> {
    let file = tokio::fs::File::open(&backup_file)
        .await
        .context("portable backup file is unavailable")?;
    let password = std::env::var("API_SYSTEM_BACKUP_PASSWORD")
        .ok()
        .filter(|value| !value.is_empty());
    let service = SystemBackupService::new(repository, key_provider);
    let sealed = service
        .import_with_password(file, password.as_deref())
        .await
        .context("portable backup import failed")?;
    Ok(json!({
        "status": "imported",
        "backup_set_id": sealed.manifest().backup_set_id(),
    }))
}

async fn source_master(
    key_provider: Arc<EnvironmentBackupKeyProvider>,
    backup_file: PathBuf,
) -> Result<String> {
    let file = tokio::fs::File::open(&backup_file)
        .await
        .context("portable backup file is unavailable")?;
    let sealed = read_backup_bundle_manifest(file)
        .await
        .context("portable backup manifest is invalid")?;
    let password = std::env::var("API_SYSTEM_BACKUP_PASSWORD")
        .ok()
        .filter(|value| !value.is_empty());
    recover_source_master_key(
        key_provider.as_ref(),
        sealed.manifest(),
        password.as_deref(),
    )
    .await
    .context("portable recovery material is unavailable")
}

async fn open_repository(config: &ApiConfig) -> Result<Arc<LocalBackupRepository>> {
    let protected_roots = protected_roots(config);
    for root in &protected_roots {
        tokio::fs::create_dir_all(root).await?;
    }
    LocalBackupRepository::open(&config.system_backup_repository_root, &protected_roots)
        .await
        .map(Arc::new)
        .map_err(|_| anyhow!("system backup repository is unavailable or overlaps protected data"))
}

async fn preflight(
    config: &ApiConfig,
    repository: Arc<LocalBackupRepository>,
    key_provider: Arc<EnvironmentBackupKeyProvider>,
    backup_set_id: BackupSetId,
) -> Result<Value> {
    let service = SystemBackupService::new(repository, key_provider);
    let password = std::env::var("API_SYSTEM_BACKUP_PASSWORD")
        .ok()
        .filter(|value| !value.is_empty());
    service
        .verify_with_password(backup_set_id, password.as_deref())
        .await?;
    let sealed = service.get(backup_set_id).await?;
    let toolchain = discover_postgres_toolchain().await?;
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&config.database_url)
        .await
        .context("offline recovery target database is unavailable")?;
    toolchain.verify_server_compatibility(&pool).await?;
    pool.close().await;
    let migration_head = storage_durable::current_migration_head()?;
    let supported_source_migration_heads = storage_durable::supported_migration_heads()?;
    let master_key_fingerprint = KeyFingerprint::try_from(format!(
        "{:x}",
        Sha256::digest(config.provider_secret_master_key.as_bytes())
    ))?;
    let manifest = sealed.manifest();
    let target = domain::BackupCompatibilityTarget {
        format_version: domain::SYSTEM_BACKUP_FORMAT_VERSION,
        application_build: config.application_build.clone(),
        migration_head: migration_head.clone(),
        supported_source_migration_heads,
        master_key_fingerprint,
    };
    if domain::strict_backup_compatibility(manifest, &target).is_err() {
        bail!("backup is incompatible with the offline recovery target");
    }
    Ok(json!({
        "status": "compatible",
        "backup_set_id": backup_set_id,
        "postgres_client_major": toolchain.major_version(),
        "application_build": config.application_build,
        "migration_head": migration_head,
        "manifest_verified": true,
        "uses_primary_database_journal": false,
    }))
}

async fn status(
    repository: &dyn BackupRepository,
    recovery_job_id: RecoveryJobId,
) -> Result<Value> {
    let events = recovery_events(repository, recovery_job_id).await?;
    let state = events.iter().rev().find_map(|event| match event.event {
        domain::BackupJournalEventKind::RecoveryStateChanged { state } => Some(state),
        _ => None,
    });
    Ok(json!({
        "recovery_job_id": recovery_job_id,
        "status": state,
        "journal_event_count": events.len(),
        "journal_location": "external_backup_repository",
    }))
}

async fn report(
    repository: &dyn BackupRepository,
    recovery_job_id: RecoveryJobId,
) -> Result<Value> {
    let events = recovery_events(repository, recovery_job_id).await?;
    Ok(json!({
        "recovery_job_id": recovery_job_id,
        "journal_location": "external_backup_repository",
        "events": events,
    }))
}

async fn recovery_events(
    repository: &dyn BackupRepository,
    recovery_job_id: RecoveryJobId,
) -> Result<Vec<BackupJournalEvent>> {
    repository
        .read_journal(BackupJournalSubject::Recovery(recovery_job_id))
        .await
        .map_err(|error| match error {
            BackupRepositoryError::Integrity => anyhow!("external recovery journal is corrupt"),
            _ => anyhow!("external recovery journal is unavailable"),
        })
}

async fn build_recovery_runtime(
    config: &ApiConfig,
    repository: Arc<LocalBackupRepository>,
    key_provider: Arc<EnvironmentBackupKeyProvider>,
    backup_password: Option<String>,
) -> Result<StoppedServerRecoveryRuntime> {
    let toolchain = discover_postgres_toolchain().await?;
    let compatibility_pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&config.database_url)
        .await
        .context("offline recovery target database is unavailable")?;
    toolchain
        .verify_server_compatibility(&compatibility_pool)
        .await?;
    compatibility_pool.close().await;
    let expected_migration_head = storage_durable::current_migration_head()?;
    let postgres = Arc::new(
        PostgreSqlRecoveryTarget::try_new(&config.database_url, toolchain)
            .map_err(|_| anyhow!("offline PostgreSQL recovery target is invalid"))?,
    );
    let database = Arc::new(OfflineDatabaseResolver {
        database_url: config.database_url.clone(),
    });
    let object_registry = Arc::new(storage_object::builtin_driver_registry());
    let business_objects = Arc::new(BusinessObjectRecoveryTarget::new(
        database.clone(),
        object_registry.clone(),
    ));
    let artifact_resolver = Arc::new(OfflineArtifactResolver {
        database: database.clone(),
        node_id: config.api_node_id.clone(),
        allowed_roots: vec![
            PathBuf::from(&config.provider_install_root),
            PathBuf::from(&config.mcp_template_library_root),
            PathBuf::from(&config.host_extension_dropin_root),
        ],
    });
    let extension_artifacts = Arc::new(FilesystemArtifactRecoveryTarget::new(
        artifact_resolver.clone(),
    ));
    let executor = Arc::new(OfflineRecoveryExecutor::new_with_password(
        repository.clone(),
        key_provider,
        OfflineRecoveryTargets {
            postgres,
            business_objects,
            extension_artifacts,
        },
        backup_password,
    ));
    let post_restore = PostRestoreRecoveryService::new(
        repository.clone(),
        executor.clone(),
        Arc::new(PostgreSqlPostRestoreReconciler::new(
            &config.database_url,
            &config.api_node_id,
            &config.provider_install_root,
        )),
        Arc::new(PostgreSqlPostRestoreHealthVerifier::new(
            &config.database_url,
            &config.api_node_id,
            expected_migration_head,
            repository.clone(),
            database,
            object_registry,
            artifact_resolver,
        )),
        Arc::new(StoppedServerRecoveryEphemeralState::new(
            &config.database_url,
        )),
        Arc::new(PostgreSqlRecoveryAuditProjector::new(&config.database_url)),
    );
    Ok(StoppedServerRecoveryRuntime {
        repository,
        executor,
        post_restore,
    })
}

struct StoppedServerRecoveryRuntime {
    repository: Arc<LocalBackupRepository>,
    executor: Arc<OfflineRecoveryExecutor>,
    post_restore: PostRestoreRecoveryService,
}

impl StoppedServerRecoveryRuntime {
    async fn restore_or_resume(&self, command: ExecuteOfflineRecoveryCommand) -> Result<Value> {
        self.restore(command, None).await
    }

    async fn restore_bootstrap(
        &self,
        command: ExecuteOfflineRecoveryCommand,
        database_url: &str,
    ) -> Result<Value> {
        self.restore(command, Some(database_url)).await
    }

    async fn restore(
        &self,
        command: ExecuteOfflineRecoveryCommand,
        bootstrap_database_url: Option<&str>,
    ) -> Result<Value> {
        let events = recovery_events(self.repository.as_ref(), command.recovery_job_id).await?;
        let state = events
            .iter()
            .rev()
            .find_map(|event| match event.event {
                domain::BackupJournalEventKind::RecoveryStateChanged { state } => Some(state),
                _ => None,
            })
            .context("external recovery journal has no state")?;
        let (executed_steps, resumed_steps, offline_failure) = match state {
            RecoveryJobState::Draining | RecoveryJobState::Restoring => {
                match self.executor.execute(command).await {
                    Ok(receipt) => (receipt.executed_steps, receipt.resumed_steps, None),
                    Err(error) => (Vec::new(), Vec::new(), Some(error)),
                }
            }
            RecoveryJobState::Reconciling
            | RecoveryJobState::Verifying
            | RecoveryJobState::Succeeded
            | RecoveryJobState::RolledBack
            | RecoveryJobState::ManualRecoveryRequired => (Vec::new(), Vec::new(), None),
            _ => bail!("external recovery journal is not ready for offline restore"),
        };

        if offline_failure.is_none() {
            if let Some(database_url) = bootstrap_database_url {
                self.assign_bootstrap_actor(command, database_url).await?;
            }
        }

        let lease_state = Arc::new(AtomicU8::new(LEASE_PENDING));
        let lease = Box::new(StoppedServerRecoveryLease {
            state: lease_state.clone(),
        });
        let result = match offline_failure {
            Some(error) => {
                self.post_restore
                    .settle_offline_failure(command, error, lease)
                    .await
            }
            None => self.post_restore.run(command, lease).await,
        };
        let maintenance_fence = lease_disposition(&lease_state)?;
        match result {
            Ok(receipt) => {
                let status = match receipt.outcome {
                    PostRestoreRecoveryOutcome::Succeeded => "succeeded",
                    PostRestoreRecoveryOutcome::RolledBack => "rolled_back",
                };
                Ok(json!({
                    "status": status,
                    "recovery_job_id": receipt.recovery_job_id,
                    "backup_set_id": receipt.backup_set_id,
                    "failure_code": receipt.failure_code,
                    "executed_steps": executed_steps,
                    "resumed_steps": resumed_steps,
                    "maintenance_fence": maintenance_fence,
                }))
            }
            Err(PostRestoreRecoveryError::ManualRecoveryRequired { code }) => Ok(json!({
                "status": "manual_recovery_required",
                "recovery_job_id": command.recovery_job_id,
                "backup_set_id": command.backup_set_id,
                "failure_code": code,
                "executed_steps": executed_steps,
                "resumed_steps": resumed_steps,
                "maintenance_fence": maintenance_fence,
            })),
            Err(error) => Err(anyhow!(error)),
        }
    }

    async fn assign_bootstrap_actor(
        &self,
        command: ExecuteOfflineRecoveryCommand,
        database_url: &str,
    ) -> Result<()> {
        let events = recovery_events(self.repository.as_ref(), command.recovery_job_id).await?;
        if events.iter().any(|event| {
            matches!(
                event.event,
                BackupJournalEventKind::RecoveryBootstrapActorAssigned { .. }
            )
        }) {
            return Ok(());
        }
        let pool = PgPoolOptions::new()
            .max_connections(1)
            .connect(database_url)
            .await
            .context("bootstrap recovery cannot inspect the restored root actor")?;
        let actor_user_id: Uuid = sqlx::query_scalar(
            "select id from users where default_display_role = 'root' and status = 'active' order by created_at asc limit 1",
        )
        .fetch_optional(&pool)
        .await
        .context("bootstrap recovery cannot read the restored root actor")?
        .context("bootstrap recovery restored no active root actor")?;
        pool.close().await;
        self.repository
            .append_journal_event(&BackupJournalEvent {
                event_id: Uuid::now_v7(),
                sequence: events.len() as u64,
                subject: BackupJournalSubject::Recovery(command.recovery_job_id),
                backup_set_id: command.backup_set_id,
                actor_user_id: Some(actor_user_id),
                occurred_at: time::OffsetDateTime::now_utc(),
                event: BackupJournalEventKind::RecoveryBootstrapActorAssigned { actor_user_id },
            })
            .await
            .map_err(|_| anyhow!("bootstrap recovery cannot record the restored root actor"))
    }
}

const LEASE_PENDING: u8 = 0;
const LEASE_RELEASED: u8 = 1;
const LEASE_RETAINED: u8 = 2;

struct StoppedServerRecoveryLease {
    state: Arc<AtomicU8>,
}

impl RecoveryCompletionLease for StoppedServerRecoveryLease {
    fn release(self: Box<Self>) {
        self.state.store(LEASE_RELEASED, Ordering::SeqCst);
    }

    fn retain(self: Box<Self>) {
        self.state.store(LEASE_RETAINED, Ordering::SeqCst);
    }
}

fn lease_disposition(state: &AtomicU8) -> Result<&'static str> {
    match state.load(Ordering::SeqCst) {
        LEASE_RELEASED => Ok("released"),
        LEASE_RETAINED => Ok("retained"),
        _ => bail!("post-restore service did not settle the maintenance fence"),
    }
}

struct OfflineDatabaseResolver {
    database_url: String,
}

impl OfflineDatabaseResolver {
    async fn pool(&self) -> Result<sqlx::PgPool, RecoveryStepTargetError> {
        PgPoolOptions::new()
            .max_connections(1)
            .connect(&self.database_url)
            .await
            .map_err(|_| RecoveryStepTargetError::Unavailable)
    }
}

#[async_trait]
impl RecoveryObjectStorageResolver for OfflineDatabaseResolver {
    async fn resolve(
        &self,
        storage_id: Uuid,
    ) -> Result<RecoveryObjectStorage, RecoveryStepTargetError> {
        let pool = self.pool().await?;
        let row = sqlx::query("select driver_type, config_json from file_storages where id = $1")
            .bind(storage_id)
            .fetch_optional(&pool)
            .await
            .map_err(|_| RecoveryStepTargetError::Unavailable)?
            .ok_or(RecoveryStepTargetError::InvalidTarget)?;
        pool.close().await;
        Ok(RecoveryObjectStorage {
            driver_type: row
                .try_get("driver_type")
                .map_err(|_| RecoveryStepTargetError::InvalidTarget)?,
            config_json: row
                .try_get("config_json")
                .map_err(|_| RecoveryStepTargetError::InvalidTarget)?,
        })
    }
}

struct OfflineArtifactResolver {
    database: Arc<OfflineDatabaseResolver>,
    node_id: String,
    allowed_roots: Vec<PathBuf>,
}

#[async_trait]
impl ArtifactRecoveryResolver for OfflineArtifactResolver {
    async fn embedded_target(
        &self,
        coordinate: &ArtifactRecoveryCoordinate,
    ) -> Result<PathBuf, RecoveryStepTargetError> {
        let pool = self.database.pool().await?;
        let path = sqlx::query_scalar::<_, Option<String>>(
            r#"
            select coalesce(artifact.package_path, artifact.local_path)
            from extension_installations installation
            join extension_artifact_instances artifact
              on artifact.installation_id = installation.id
             and artifact.node_id = $1
             and artifact.is_current
            where installation.category = $2
              and installation.organization = $3
              and installation.artifact_id = $4
              and installation.artifact_version = $5
            "#,
        )
        .bind(&self.node_id)
        .bind(&coordinate.category)
        .bind(&coordinate.organization)
        .bind(&coordinate.artifact_id)
        .bind(&coordinate.version)
        .fetch_optional(&pool)
        .await
        .map_err(|_| RecoveryStepTargetError::Unavailable)?
        .flatten()
        .ok_or(RecoveryStepTargetError::InvalidTarget)?;
        pool.close().await;
        let path = PathBuf::from(path);
        if !self
            .allowed_roots
            .iter()
            .any(|root| path_is_within(&path, root))
        {
            return Err(RecoveryStepTargetError::InvalidTarget);
        }
        Ok(path)
    }

    async fn verify_rebuildable(
        &self,
        coordinate: &ArtifactRecoveryCoordinate,
    ) -> Result<(), RecoveryStepTargetError> {
        let pool = self.database.pool().await?;
        let exists = sqlx::query_scalar::<_, bool>(
            r#"
            select exists(
                select 1
                from extension_installations installation
                where installation.category = $1
                  and installation.organization = $2
                  and installation.artifact_id = $3
                  and installation.artifact_version = $4
                  and installation.source_kind in (
                      'builtin', 'official_registry', 'official_repository', 'mirror_registry'
                  )
            )
            "#,
        )
        .bind(&coordinate.category)
        .bind(&coordinate.organization)
        .bind(&coordinate.artifact_id)
        .bind(&coordinate.version)
        .fetch_one(&pool)
        .await
        .map_err(|_| RecoveryStepTargetError::Unavailable)?;
        pool.close().await;
        if exists {
            Ok(())
        } else {
            Err(RecoveryStepTargetError::InvalidTarget)
        }
    }
}

fn protected_roots(config: &ApiConfig) -> Vec<PathBuf> {
    [
        &config.business_file_local_root,
        &config.provider_install_root,
        &config.mcp_template_library_root,
        &config.host_extension_dropin_root,
    ]
    .into_iter()
    .map(PathBuf::from)
    .collect()
}

fn path_is_within(path: &Path, root: &Path) -> bool {
    path.is_absolute() && root.is_absolute() && path.starts_with(root)
}
