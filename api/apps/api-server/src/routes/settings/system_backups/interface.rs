use std::sync::Arc;

use control_plane::errors::ControlPlaneError;
use interface_runtime::{InterfaceContract, UserPrincipal};
use tokio::io::{AsyncWriteExt, DuplexStream};
use uuid::Uuid;

use super::{
    detail_response, mutation_response, BackupJobStatusResponse, BackupMutationResponse,
    BackupSetDetailResponse, BackupVerificationResponse, RecoveryStatusResponse,
};
use crate::{
    error_response::{ApiError, ApiServiceUnavailable},
    routes::console_interface::{
        self, ConsoleInterfaceDeclaration, ConsoleInterfaceFuture, ConsoleInterfacePort,
        ConsoleInterfaceTargetError,
    },
    system_backup::SystemBackupRuntime,
};

pub(crate) enum SystemBackupsInput {
    Import {
        bytes: Vec<u8>,
        backup_password: Option<String>,
    },
    GetJobStatus {
        backup_job_id: Uuid,
    },
    GetDetail {
        backup_set_id: Uuid,
    },
    Delete {
        backup_set_id: Uuid,
    },
    Verify {
        backup_set_id: Uuid,
        backup_password: Option<String>,
    },
    Download {
        backup_set_id: Uuid,
    },
    GetRecoveryStatus {
        recovery_job_id: Option<Uuid>,
    },
}

impl InterfaceContract for SystemBackupsInput {
    const CONTRACT_ID: &'static str = "console-system-backups-input";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) struct BackupDownload {
    pub(crate) status: u16,
    pub(crate) content_type: &'static str,
    pub(crate) content_disposition: String,
    pub(crate) reader: DuplexStream,
}

pub(crate) enum SystemBackupsOutput {
    Imported(BackupMutationResponse),
    JobStatus(BackupJobStatusResponse),
    Detail(BackupSetDetailResponse),
    Deleted,
    Verified(BackupVerificationResponse),
    Download(BackupDownload),
    RecoveryStatus(RecoveryStatusResponse),
}

impl InterfaceContract for SystemBackupsOutput {
    const CONTRACT_ID: &'static str = "console-system-backups-output";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) struct SystemBackupsDependencies {
    pub(crate) runtime: Option<Arc<SystemBackupRuntime>>,
}

struct SystemBackupsAdapter(SystemBackupsDependencies);

pub(crate) fn port(
    dependencies: SystemBackupsDependencies,
) -> Arc<dyn ConsoleInterfacePort<SystemBackupsInput, SystemBackupsOutput>> {
    Arc::new(SystemBackupsAdapter(dependencies))
}

impl SystemBackupsAdapter {
    fn runtime(&self) -> Result<Arc<SystemBackupRuntime>, ApiError> {
        self.0
            .runtime
            .clone()
            .ok_or_else(|| ApiServiceUnavailable("system_backup_unavailable").into())
    }

    fn require_root_cookie(principal: &UserPrincipal) -> Result<(), ApiError> {
        if !principal.actor().is_root {
            return Err(ControlPlaneError::PermissionDenied("root_recovery_required").into());
        }
        if principal.authenticated_session().is_none() {
            return Err(ControlPlaneError::PermissionDenied("cookie_session_required").into());
        }
        Ok(())
    }

    async fn import(
        &self,
        bytes: Vec<u8>,
        backup_password: Option<String>,
    ) -> Result<BackupMutationResponse, ApiError> {
        let runtime = self.runtime()?;
        let capacity = bytes.len().clamp(1, 256 * 1024);
        let (mut reader, mut writer) = tokio::io::duplex(capacity);
        tokio::spawn(async move {
            let _ = writer.write_all(&bytes).await;
            let _ = writer.shutdown().await;
        });
        let sealed = runtime
            .import_with_password(&mut reader, backup_password.as_deref())
            .await?;
        Ok(mutation_response(sealed.manifest().backup_set_id()))
    }

    async fn download(&self, backup_set_id: Uuid) -> Result<BackupDownload, ApiError> {
        let backup_set_id = domain::BackupSetId::from_uuid(backup_set_id);
        let runtime = self.runtime()?;
        runtime.get(backup_set_id).await?;
        let (reader, writer) = tokio::io::duplex(256 * 1024);
        tokio::spawn(async move {
            if let Err(error) = runtime.download(backup_set_id, writer).await {
                tracing::warn!(backup_set_id = %backup_set_id.as_uuid(), error = %error, "backup download stream failed");
            }
        });
        Ok(BackupDownload {
            status: 200,
            content_type: "application/octet-stream",
            content_disposition: format!(
                "attachment; filename=\"{}.1fb-backup\"",
                backup_set_id.as_uuid()
            ),
            reader,
        })
    }

    async fn recovery_status(
        &self,
        principal: &UserPrincipal,
        recovery_job_id: Option<Uuid>,
    ) -> Result<RecoveryStatusResponse, ApiError> {
        Self::require_root_cookie(principal)?;
        let runtime = self.runtime()?;
        let maintenance = runtime.maintenance_status();
        let active = runtime.active_recovery();
        let requested_job_id = recovery_job_id
            .map(domain::RecoveryJobId::from_uuid)
            .or(maintenance.recovery_job_id);
        let journal = match requested_job_id {
            Some(recovery_job_id) => runtime.recovery_journal(recovery_job_id).await?,
            None => Vec::new(),
        };
        let journal_state = journal.iter().rev().find_map(|event| match &event.event {
            domain::BackupJournalEventKind::RecoveryStateChanged { state } => Some(*state),
            _ => None,
        });
        let journal_events = journal
            .into_iter()
            .map(serde_json::to_value)
            .collect::<Result<Vec<_>, _>>()?;
        let phase = match maintenance.phase {
            control_plane::system_recovery::SystemMaintenancePhase::Online => "online",
            control_plane::system_recovery::SystemMaintenancePhase::Draining => "draining",
            control_plane::system_recovery::SystemMaintenancePhase::Active => "active",
        };
        Ok(RecoveryStatusResponse {
            phase: phase.to_owned(),
            recovery_job_id: requested_job_id,
            active_write_count: maintenance.active_write_count() as u64,
            started_at: maintenance.started_at.map(|value| value.to_string()),
            target_backup_set_id: active.as_ref().map(|value| value.target_backup_set_id),
            safety_backup_set_id: active.as_ref().map(|value| value.safety_backup_set_id),
            plan_digest: active.map(|value| value.plan_digest.as_str().to_owned()),
            journal_state,
            journal_events,
        })
    }

    async fn execute_inner(
        &self,
        principal: &UserPrincipal,
        input: SystemBackupsInput,
    ) -> Result<SystemBackupsOutput, ApiError> {
        match input {
            SystemBackupsInput::Import {
                bytes,
                backup_password,
            } => Ok(SystemBackupsOutput::Imported(
                self.import(bytes, backup_password).await?,
            )),
            SystemBackupsInput::GetJobStatus { backup_job_id } => {
                let status = self
                    .runtime()?
                    .backup_job_status(domain::BackupJobId::from_uuid(backup_job_id))
                    .await?
                    .ok_or(ControlPlaneError::NotFound("backup_job"))?;
                Ok(SystemBackupsOutput::JobStatus(BackupJobStatusResponse {
                    backup_job_id: status.backup_job_id,
                    backup_set_id: status.backup_set_id,
                    status: status.state,
                    failure_code: status.failure_code,
                    sealed_components: status.sealed_components,
                }))
            }
            SystemBackupsInput::GetDetail { backup_set_id } => {
                let backup_set_id = domain::BackupSetId::from_uuid(backup_set_id);
                let detail = self.runtime()?.detail(backup_set_id).await?;
                Ok(SystemBackupsOutput::Detail(detail_response(
                    backup_set_id,
                    detail,
                )))
            }
            SystemBackupsInput::Delete { backup_set_id } => {
                self.runtime()?
                    .delete(domain::BackupSetId::from_uuid(backup_set_id))
                    .await?;
                Ok(SystemBackupsOutput::Deleted)
            }
            SystemBackupsInput::Verify {
                backup_set_id,
                backup_password,
            } => {
                let backup_set_id = domain::BackupSetId::from_uuid(backup_set_id);
                self.runtime()?
                    .verify_with_password(backup_set_id, backup_password.as_deref())
                    .await?;
                Ok(SystemBackupsOutput::Verified(BackupVerificationResponse {
                    backup_set_id,
                    verified: true,
                }))
            }
            SystemBackupsInput::Download { backup_set_id } => Ok(SystemBackupsOutput::Download(
                self.download(backup_set_id).await?,
            )),
            SystemBackupsInput::GetRecoveryStatus { recovery_job_id } => {
                Ok(SystemBackupsOutput::RecoveryStatus(
                    self.recovery_status(principal, recovery_job_id).await?,
                ))
            }
        }
    }
}

impl ConsoleInterfacePort<SystemBackupsInput, SystemBackupsOutput> for SystemBackupsAdapter {
    fn execute<'a>(
        &'a self,
        principal: &'a UserPrincipal,
        input: SystemBackupsInput,
    ) -> ConsoleInterfaceFuture<'a, SystemBackupsOutput> {
        Box::pin(async move {
            self.execute_inner(principal, input)
                .await
                .map_err(ConsoleInterfaceTargetError)
        })
    }
}

pub(crate) const DECLARATIONS: &[ConsoleInterfaceDeclaration] = &[
    ConsoleInterfaceDeclaration {
        interface_id: "system_backups.import",
        binding_id: "http.console.settings.system-backups.import.v1",
        method: "POST",
        path: "/api/console/settings/system-backups/import",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "system_backups.recovery.status",
        binding_id: "http.console.settings.system-backups.recovery-status.get.v1",
        method: "GET",
        path: "/api/console/settings/system-backups/recovery/status",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "system_backups.status",
        binding_id: "http.console.settings.system-backups.job-status.get.v1",
        method: "GET",
        path: "/api/console/settings/system-backups/jobs/status/:backup_job_id",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "system_backups.detail",
        binding_id: "http.console.settings.system-backups.detail.get.v1",
        method: "GET",
        path: "/api/console/settings/system-backups/:backup_set_id",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "system_backups.delete",
        binding_id: "http.console.settings.system-backups.delete.v1",
        method: "DELETE",
        path: "/api/console/settings/system-backups/:backup_set_id",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "system_backups.verify",
        binding_id: "http.console.settings.system-backups.verify.v1",
        method: "POST",
        path: "/api/console/settings/system-backups/:backup_set_id/verify",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "system_backups.download",
        binding_id: "http.console.settings.system-backups.download.get.v1",
        method: "GET",
        path: "/api/console/settings/system-backups/:backup_set_id/download",
        mutating: false,
    },
];

pub(crate) fn compile_registry(
    port: Arc<dyn ConsoleInterfacePort<SystemBackupsInput, SystemBackupsOutput>>,
) -> Result<
    Arc<interface_runtime::CompiledInterfaceRegistry>,
    interface_runtime::RegistryCompilationError,
> {
    console_interface::compile_registry(
        "api-server.console-system-backups",
        "graph:console-system-backups-v1",
        DECLARATIONS,
        port,
    )
}

#[cfg(test)]
struct UnavailableSystemBackupsPort;

#[cfg(test)]
impl ConsoleInterfacePort<SystemBackupsInput, SystemBackupsOutput>
    for UnavailableSystemBackupsPort
{
    fn execute<'a>(
        &'a self,
        _principal: &'a UserPrincipal,
        _input: SystemBackupsInput,
    ) -> ConsoleInterfaceFuture<'a, SystemBackupsOutput> {
        Box::pin(async {
            Err(ConsoleInterfaceTargetError(
                anyhow::anyhow!("system backup fixture unavailable").into(),
            ))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn f13c_registry_freezes_system_backup_bindings() {
        let registry = compile_registry(Arc::new(UnavailableSystemBackupsPort)).unwrap();
        for declaration in DECLARATIONS {
            let binding = registry
                .binding(&interface_runtime::BindingId::new(declaration.binding_id).unwrap())
                .expect("declared system backup binding must be frozen");
            let route = binding.projection().http_route().unwrap();
            assert_eq!(route.method(), declaration.method);
            assert_eq!(route.path(), declaration.path);
        }
        assert_eq!(registry.bindings().count(), DECLARATIONS.len());
    }
}
