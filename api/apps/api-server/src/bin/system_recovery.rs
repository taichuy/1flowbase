use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::{anyhow, bail, Context, Result};
use api_server::{
    config::ApiConfig,
    system_backup::{EnvironmentBackupKeyProvider, LocalBackupRepository},
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
    system_backup::SystemBackupService,
    system_recovery::{
        ExecuteOfflineRecoveryCommand, OfflineRecoveryExecutor, OfflineRecoveryTargets,
        RecoveryStepTargetError,
    },
};
use domain::{
    ApplicationBuild, BackupJournalEvent, BackupJournalSubject, BackupSetId, KeyFingerprint,
    RecoveryJobId,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::{postgres::PgPoolOptions, Row};
use storage_durable::{PostgreSqlRecoveryTarget, PostgreSqlToolchain};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Command {
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
}

#[tokio::main]
async fn main() -> Result<()> {
    let arguments = parse_arguments(std::env::args().skip(1))?;
    let config = ApiConfig::from_env()?;
    let repository = open_repository(&config).await?;
    let key_provider = Arc::new(
        EnvironmentBackupKeyProvider::from_base64(&config.system_backup_key_base64)
            .map_err(|_| anyhow!("system backup key is unavailable"))?,
    );

    let output = match arguments.command {
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
            let executor = build_executor(&config, repository, key_provider).await?;
            match arguments.command {
                Command::Restore | Command::Resume => {
                    let receipt = executor.execute(command).await?;
                    json!({
                        "status": "reconciling",
                        "recovery_job_id": receipt.recovery_job_id,
                        "backup_set_id": receipt.backup_set_id,
                        "executed_steps": receipt.executed_steps,
                        "resumed_steps": receipt.resumed_steps,
                    })
                }
                Command::Rollback => {
                    executor.rollback_promoted_targets(command).await?;
                    json!({"status": "rolled_back", "recovery_job_id": command.recovery_job_id})
                }
                Command::Finalize => {
                    executor.finalize_promoted_targets(command).await?;
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
            _ => bail!("unknown recovery option `{flag}`"),
        }
    }
    Ok(Arguments {
        command,
        backup_set_id,
        recovery_job_id,
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

fn print_usage() {
    eprintln!(
        "usage: system_recovery <preflight|status|restore|resume|rollback|finalize|report> \
         [--backup-set-id <uuid>] [--recovery-job-id <uuid>]"
    );
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
    service.verify(backup_set_id).await?;
    let sealed = service.get(backup_set_id).await?;
    let toolchain = PostgreSqlToolchain::discover_from_path().await?;
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&config.database_url)
        .await
        .context("offline recovery target database is unavailable")?;
    toolchain.verify_server_compatibility(&pool).await?;
    let migration_head = storage_durable::migration_head(&pool).await?;
    pool.close().await;
    let application_build = ApplicationBuild::try_from(config.system_build_identity.clone())?;
    let master_key_fingerprint = KeyFingerprint::try_from(format!(
        "{:x}",
        Sha256::digest(config.provider_secret_master_key.as_bytes())
    ))?;
    let manifest = sealed.manifest();
    if manifest.application_build() != &application_build
        || manifest.migration_head() != &migration_head
        || manifest.master_key_fingerprint() != &master_key_fingerprint
    {
        bail!("backup is incompatible with the offline recovery target");
    }
    Ok(json!({
        "status": "compatible",
        "backup_set_id": backup_set_id,
        "postgres_client_major": toolchain.major_version(),
        "application_build": application_build,
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

async fn build_executor(
    config: &ApiConfig,
    repository: Arc<LocalBackupRepository>,
    key_provider: Arc<EnvironmentBackupKeyProvider>,
) -> Result<OfflineRecoveryExecutor> {
    let toolchain = PostgreSqlToolchain::discover_from_path().await?;
    let compatibility_pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&config.database_url)
        .await
        .context("offline recovery target database is unavailable")?;
    toolchain
        .verify_server_compatibility(&compatibility_pool)
        .await?;
    compatibility_pool.close().await;
    let postgres = Arc::new(
        PostgreSqlRecoveryTarget::try_new(&config.database_url, toolchain)
            .map_err(|_| anyhow!("offline PostgreSQL recovery target is invalid"))?,
    );
    let database = Arc::new(OfflineDatabaseResolver {
        database_url: config.database_url.clone(),
    });
    let business_objects = Arc::new(BusinessObjectRecoveryTarget::new(
        database.clone(),
        Arc::new(storage_object::builtin_driver_registry()),
    ));
    let extension_artifacts = Arc::new(FilesystemArtifactRecoveryTarget::new(Arc::new(
        OfflineArtifactResolver {
            database,
            node_id: config.api_node_id.clone(),
            allowed_roots: vec![
                PathBuf::from(&config.provider_install_root),
                PathBuf::from(&config.mcp_template_library_root),
                PathBuf::from(&config.host_extension_dropin_root),
            ],
        },
    )));
    Ok(OfflineRecoveryExecutor::new(
        repository,
        key_provider,
        OfflineRecoveryTargets {
            postgres,
            business_objects,
            extension_artifacts,
        },
    ))
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
