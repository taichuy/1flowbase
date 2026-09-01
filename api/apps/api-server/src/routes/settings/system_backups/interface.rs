use std::sync::Arc;

use argon2::{
    password_hash::{PasswordHash, PasswordVerifier},
    Argon2,
};
use control_plane::{errors::ControlPlaneError, system_recovery::ConfirmedRecoveryIntent};
use domain::{BackupSetId, ContentDigest, RecoveryJobId};
use interface_runtime::{InterfaceContract, UserPrincipal};
use storage_durable_postgres::MainDurableStore;
use time::OffsetDateTime;
use tokio::io::{AsyncWriteExt, DuplexStream};
use uuid::Uuid;

use super::{
    canonical_backup_name, detail_response, mutation_response, preflight_response,
    require_compatible_digest, validate_exact_name, BackupJobStatusResponse,
    BackupMutationResponse, BackupSetDetailResponse, BackupSetListResponse,
    BackupSetSummaryResponse, BackupVerificationResponse, CreateRecoveryIntentRequest,
    QueuedBackupResponse, RecoveryIntentResponse, RecoveryPreflightResponse, RecoveryReauthRequest,
    RecoveryReauthResponse, RecoveryStatusResponse,
};
use crate::{
    error_response::{ApiError, ApiServiceUnavailable},
    middleware::require_settings_feature_permission::authorize_compiled_console_access,
    recovery_authorization::{
        consume_reauth_challenge, issue_reauth_challenge, recovery_intent_ttl,
    },
    routes::console_interface::{
        self, ConsoleInterfaceDeclaration, ConsoleInterfaceFuture, ConsoleInterfacePort,
        ConsoleInterfaceTargetError,
    },
    system_backup::SystemBackupRuntime,
};

pub(crate) enum SystemBackupsInput {
    List,
    Create {
        backup_password: Option<String>,
    },
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
    RecoveryPreflight {
        backup_set_id: Uuid,
        backup_password: Option<String>,
    },
    RecoveryReauth {
        request: RecoveryReauthRequest,
    },
    RecoveryIntent {
        backup_set_id: Uuid,
        request: CreateRecoveryIntentRequest,
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
    Listed(BackupSetListResponse),
    Created(QueuedBackupResponse),
    Imported(BackupMutationResponse),
    JobStatus(BackupJobStatusResponse),
    Detail(BackupSetDetailResponse),
    Deleted,
    Verified(BackupVerificationResponse),
    Download(BackupDownload),
    RecoveryStatus(RecoveryStatusResponse),
    RecoveryPreflight(RecoveryPreflightResponse),
    RecoveryReauth(RecoveryReauthResponse),
    RecoveryIntent(RecoveryIntentResponse),
}

impl InterfaceContract for SystemBackupsOutput {
    const CONTRACT_ID: &'static str = "console-system-backups-output";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) struct SystemBackupsDependencies {
    pub(crate) runtime: Option<Arc<SystemBackupRuntime>>,
    pub(crate) store: MainDurableStore,
    pub(crate) backup_status_access: BackupStatusAccess,
}

#[derive(Clone)]
pub(crate) struct BackupStatusAccess {
    pub(crate) operation_id: String,
    pub(crate) policy_group: access_control::ConsolePolicyGroup,
    pub(crate) authorization: access_control::ConsoleAuthorization,
    pub(crate) resource_access: Option<access_control::ResourceAccessRegistration>,
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

    async fn authorize_backup_status(&self, principal: &UserPrincipal) -> Result<(), ApiError> {
        let actor = principal.actor();
        if actor.is_root
            || matches!(
                self.0.backup_status_access.authorization,
                access_control::ConsoleAuthorization::Authenticated
            )
        {
            return Ok(());
        }
        let policies = self
            .0
            .store
            .load_console_policy_for_bound_role(
                actor.user_id,
                actor.current_workspace_id,
                &actor.effective_display_role,
            )
            .await?;
        let access = access_control::ConsoleRouteAccess {
            operation_id: &self.0.backup_status_access.operation_id,
            policy_group: &self.0.backup_status_access.policy_group,
            authorization: &self.0.backup_status_access.authorization,
            resource_access: self.0.backup_status_access.resource_access.as_ref(),
        };
        if !authorize_compiled_console_access(&access, actor, &policies) {
            return Err(
                ControlPlaneError::PermissionDenied("console_operation_permission_denied").into(),
            );
        }
        Ok(())
    }

    async fn recovery_reauth(
        &self,
        principal: &UserPrincipal,
        body: RecoveryReauthRequest,
    ) -> Result<RecoveryReauthResponse, ApiError> {
        Self::require_root_cookie(principal)?;
        validate_exact_name(body.backup_set_id, &body.exact_backup_name)?;
        let plan = self
            .runtime()?
            .preflight_with_password(body.backup_set_id, body.backup_password.as_deref())
            .await;
        require_compatible_digest(&plan, &body.plan_digest)?;
        let user = self
            .0
            .store
            .find_user_by_id(principal.actor().user_id)
            .await?
            .ok_or(ControlPlaneError::PermissionDenied(
                "recovery_reauth_failed",
            ))?;
        let parsed = PasswordHash::new(&user.password_hash)
            .map_err(|_| ControlPlaneError::PermissionDenied("recovery_reauth_failed"))?;
        Argon2::default()
            .verify_password(body.password.as_bytes(), &parsed)
            .map_err(|_| ControlPlaneError::PermissionDenied("recovery_reauth_failed"))?;
        let digest = ContentDigest::try_from(body.plan_digest)
            .map_err(|_| ControlPlaneError::InvalidInput("plan_digest"))?;
        let session = principal
            .authenticated_session()
            .expect("root cookie checked");
        let challenge = issue_reauth_challenge(
            principal.actor().user_id,
            session.expose_to_trusted_handler(),
            body.backup_set_id,
            digest,
            &body.exact_backup_name,
        );
        Ok(RecoveryReauthResponse {
            challenge_token: challenge.token,
            expires_at: challenge.expires_at.to_string(),
        })
    }

    async fn recovery_intent(
        &self,
        principal: &UserPrincipal,
        backup_set_id: Uuid,
        body: CreateRecoveryIntentRequest,
    ) -> Result<RecoveryIntentResponse, ApiError> {
        Self::require_root_cookie(principal)?;
        let backup_set_id = BackupSetId::from_uuid(backup_set_id);
        validate_exact_name(backup_set_id, &body.exact_backup_name)?;
        let runtime = self.runtime()?;
        let plan = runtime
            .preflight_with_password(backup_set_id, body.backup_password.as_deref())
            .await;
        let plan_digest = require_compatible_digest(&plan, &body.plan_digest)?;
        let session = principal
            .authenticated_session()
            .expect("root cookie checked");
        consume_reauth_challenge(
            body.challenge_token,
            principal.actor().user_id,
            session.expose_to_trusted_handler(),
            backup_set_id,
            &plan_digest,
            &body.exact_backup_name,
        )
        .map_err(ControlPlaneError::PermissionDenied)?;

        let now = OffsetDateTime::now_utc();
        let expires_at = now + recovery_intent_ttl();
        let intent_id = Uuid::now_v7();
        let recovery_job_id = RecoveryJobId::new();
        let confirmed = ConfirmedRecoveryIntent::try_new(
            intent_id,
            recovery_job_id,
            principal.actor().user_id,
            backup_set_id,
            plan_digest,
            now,
            expires_at,
        )
        .map_err(|_| ControlPlaneError::InvalidInput("recovery_intent"))?;
        let lease = runtime.reserve_recovery_maintenance(recovery_job_id)?;
        let target_backup_password = body.backup_password;
        tokio::spawn(async move {
            if let Err(error) = runtime
                .prepare_recovery_with_maintenance_lease(confirmed, target_backup_password, lease)
                .await
            {
                tracing::error!(intent_id = %intent_id, recovery_job_id = %recovery_job_id.as_uuid(), error = %error, "recovery handoff preparation failed");
            }
        });
        Ok(RecoveryIntentResponse {
            intent_id,
            recovery_job_id,
            backup_set_id,
            status: "preparing".to_owned(),
            expires_at: expires_at.to_string(),
        })
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
            SystemBackupsInput::List => {
                let items = self
                    .runtime()?
                    .list()
                    .await?
                    .into_iter()
                    .map(|entry| BackupSetSummaryResponse {
                        exact_backup_name: canonical_backup_name(entry.backup_set_id),
                        backup_set_id: entry.backup_set_id,
                        created_at: entry.created_at.to_string(),
                        availability: entry.availability,
                        total_size_bytes: entry.total_size_bytes,
                        envelope_digest: entry
                            .envelope_digest
                            .map(|value| value.as_str().to_owned()),
                    })
                    .collect();
                Ok(SystemBackupsOutput::Listed(BackupSetListResponse { items }))
            }
            SystemBackupsInput::Create { backup_password } => {
                self.authorize_backup_status(principal).await?;
                let queued = self
                    .runtime()?
                    .queue_manual_backup(principal.actor().user_id, backup_password)
                    .await?;
                Ok(SystemBackupsOutput::Created(QueuedBackupResponse {
                    backup_job_id: queued.backup_job_id,
                    backup_set_id: queued.backup_set_id,
                }))
            }
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
            SystemBackupsInput::RecoveryPreflight {
                backup_set_id,
                backup_password,
            } => {
                Self::require_root_cookie(principal)?;
                let plan = self
                    .runtime()?
                    .preflight_with_password(
                        BackupSetId::from_uuid(backup_set_id),
                        backup_password.as_deref(),
                    )
                    .await;
                Ok(SystemBackupsOutput::RecoveryPreflight(preflight_response(
                    &plan,
                )?))
            }
            SystemBackupsInput::RecoveryReauth { request } => {
                Ok(SystemBackupsOutput::RecoveryReauth(
                    self.recovery_reauth(principal, request).await?,
                ))
            }
            SystemBackupsInput::RecoveryIntent {
                backup_set_id,
                request,
            } => Ok(SystemBackupsOutput::RecoveryIntent(
                self.recovery_intent(principal, backup_set_id, request)
                    .await?,
            )),
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
        interface_id: "system_backups.list",
        binding_id: "http.console.settings.system-backups.list.get.v1",
        method: "GET",
        path: "/api/console/settings/system-backups",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "system_backups.create",
        binding_id: "http.console.settings.system-backups.create.v1",
        method: "POST",
        path: "/api/console/settings/system-backups",
        mutating: true,
    },
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
    ConsoleInterfaceDeclaration {
        interface_id: "system_backups.recovery.preflight",
        binding_id: "http.console.settings.system-backups.recovery-preflight.v1",
        method: "POST",
        path: "/api/console/settings/system-backups/:backup_set_id/recovery/preflight",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "system_backups.recovery.reauth",
        binding_id: "http.console.settings.system-backups.recovery-reauth.v1",
        method: "POST",
        path: "/api/console/settings/system-backups/recovery/reauth",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "system_backups.recovery.intent",
        binding_id: "http.console.settings.system-backups.recovery-intent.v1",
        method: "POST",
        path: "/api/console/settings/system-backups/:backup_set_id/recovery/intents",
        mutating: true,
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
        for operation in [
            "system_backups.list",
            "system_backups.create",
            "system_backups.recovery.preflight",
            "system_backups.recovery.reauth",
            "system_backups.recovery.intent",
        ] {
            assert!(DECLARATIONS
                .iter()
                .any(|declaration| declaration.interface_id == operation));
        }
    }
}
