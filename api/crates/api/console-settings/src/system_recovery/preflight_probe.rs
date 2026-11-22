use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use async_trait::async_trait;
use control_plane::system_recovery::{
    RecoveryActiveWork, RecoveryPreflightError, RecoveryTargetProbe, RecoveryTargetSnapshot,
    SystemMaintenance, SystemWriteOwner,
};
use domain::{
    ApplicationBuild, BackupCompatibilityTarget, KeyFingerprint, SYSTEM_BACKUP_FORMAT_VERSION,
};
use sha2::{Digest, Sha256};
use storage_durable_postgres::{
    current_migration_head, supported_migration_heads, MainDurableStore, PostgreSqlToolchain,
};
use sysinfo::Disks;

pub struct ApiRecoveryTargetProbe {
    store: MainDurableStore,
    application_build: ApplicationBuild,
    master_key_fingerprint: KeyFingerprint,
    repository_root: PathBuf,
    roots_separated: bool,
    maintenance: Arc<SystemMaintenance>,
    postgres_toolchain: PostgreSqlToolchain,
}

impl ApiRecoveryTargetProbe {
    pub fn new(
        store: MainDurableStore,
        application_build: ApplicationBuild,
        provider_secret_master_key: &str,
        repository_root: impl Into<PathBuf>,
        roots_separated: bool,
        maintenance: Arc<SystemMaintenance>,
        postgres_toolchain: PostgreSqlToolchain,
    ) -> Result<Self, RecoveryPreflightError> {
        let fingerprint = format!(
            "{:x}",
            base16ct::HexDisplay(&Sha256::digest(provider_secret_master_key.as_bytes()))
        );
        let master_key_fingerprint = KeyFingerprint::try_from(fingerprint)
            .map_err(|_| RecoveryPreflightError::TargetProbe)?;
        Ok(Self {
            store,
            application_build,
            master_key_fingerprint,
            repository_root: repository_root.into(),
            roots_separated,
            maintenance,
            postgres_toolchain,
        })
    }
}

#[async_trait]
impl RecoveryTargetProbe for ApiRecoveryTargetProbe {
    async fn snapshot(&self) -> Result<RecoveryTargetSnapshot, RecoveryPreflightError> {
        let migration_head =
            current_migration_head().map_err(|_| RecoveryPreflightError::TargetProbe)?;
        let supported_source_migration_heads =
            supported_migration_heads().map_err(|_| RecoveryPreflightError::TargetProbe)?;
        let postgres_toolchain_compatible = self
            .store
            .verify_postgres_toolchain(&self.postgres_toolchain)
            .await
            .is_ok();
        let postgres_restore_privileges = self
            .store
            .has_postgres_restore_privileges()
            .await
            .map_err(|_| RecoveryPreflightError::TargetProbe)?;
        let snapshot = self.maintenance.snapshot();
        Ok(RecoveryTargetSnapshot {
            compatibility: BackupCompatibilityTarget {
                format_version: SYSTEM_BACKUP_FORMAT_VERSION,
                application_build: self.application_build.clone(),
                migration_head,
                supported_source_migration_heads,
                master_key_fingerprint: self.master_key_fingerprint.clone(),
            },
            available_space_bytes: available_space(&self.repository_root)
                .ok_or(RecoveryPreflightError::TargetProbe)?,
            postgres_toolchain_compatible,
            postgres_restore_privileges,
            target_roots_separated: self.roots_separated,
            active_work: snapshot
                .write_owners
                .into_iter()
                .map(|activity| RecoveryActiveWork {
                    owner_id: owner_id(activity.owner).to_string(),
                    active_count: activity.active_writes as u64,
                    drainable: true,
                })
                .collect(),
        })
    }
}

fn available_space(path: &Path) -> Option<u64> {
    let disks = Disks::new_with_refreshed_list();
    disks
        .list()
        .iter()
        .filter(|disk| path.starts_with(disk.mount_point()))
        .max_by_key(|disk| disk.mount_point().as_os_str().len())
        .or_else(|| disks.list().iter().find(|disk| disk.total_space() > 0))
        .map(|disk| disk.available_space())
}

fn owner_id(owner: SystemWriteOwner) -> &'static str {
    match owner {
        SystemWriteOwner::TraceProjectionPersistence => "trace_projection_persistence",
        SystemWriteOwner::ApiMutation => "api_mutation",
        SystemWriteOwner::ApplicationLogDeletion => "application_log_deletion",
        SystemWriteOwner::ProviderRequestLogPersistence => "provider_request_log_persistence",
        SystemWriteOwner::WorkflowScheduleDispatch => "workflow_schedule_dispatch",
        SystemWriteOwner::WorkflowScheduleExecution => "workflow_schedule_execution",
        SystemWriteOwner::CommunicationReceive => "communication_receive",
        SystemWriteOwner::CommunicationExecution => "communication_execution",
        SystemWriteOwner::EventWorkflowExecution => "event_workflow_execution",
    }
}
