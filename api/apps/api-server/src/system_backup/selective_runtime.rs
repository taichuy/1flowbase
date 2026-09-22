use super::*;
use control_plane::system_recovery::{RecoveryImpactPreview, RecoveryPreflightFailure};
use control_plane_contracts::system_backup::selective::{
    SelectiveBackupCategory, SelectiveBackupSelection, SELECTIVE_BACKUP_CONTENT_TYPE,
};

impl SystemBackupRuntime {
    pub fn is_selective(sealed: &SealedBackupManifest) -> bool {
        sealed.manifest().components().iter().any(|component| {
            component.kind == domain::BackupComponentKind::PostgreSql
                && component.content_type == SELECTIVE_BACKUP_CONTENT_TYPE
        })
    }

    pub async fn selective_catalog(
        &self,
    ) -> Result<Vec<SelectiveBackupCategory>, SystemBackupRuntimeError> {
        Ok(self.selective.catalog().await?)
    }

    pub async fn queue_selected_backup(
        self: &Arc<Self>,
        actor_user_id: Uuid,
        backup_password: Option<String>,
        selection: Vec<SelectiveBackupSelection>,
        include_file_bytes: bool,
    ) -> Result<QueuedSystemBackup, SystemBackupRuntimeError> {
        // Validate the selection before reserving maintenance or publishing a queued job.
        let row_source = self.selective.source(selection.clone()).await?;
        let queued = QueuedSystemBackup {
            backup_job_id: BackupJobId::new(),
            backup_set_id: BackupSetId::new(),
        };
        let lease = self
            .maintenance
            .begin(
                SystemMaintenanceOperation::Backup(queued.backup_job_id),
                time::OffsetDateTime::now_utc(),
            )
            .map_err(|_| SystemBackupRuntimeError::MaintenanceBusy)?;
        if let Err(error) = self
            .service
            .queue_manual_backup(queued.backup_job_id, queued.backup_set_id, actor_user_id)
            .await
        {
            lease.finish();
            return Err(error.into());
        }
        let runtime = self.clone();
        tokio::spawn(async move {
            let result = async {
                lease
                    .wait_for_drain(std::time::Duration::from_secs(30))
                    .await
                    .map_err(|_| SystemBackupRuntimeError::Drain)?;
                let mut sources = vec![row_source];
                if include_file_bytes {
                    let scope = runtime.selective.object_scope(&selection).await?;
                    sources.extend(
                        BusinessObjectBackupExporter::new(
                            runtime.store.clone(),
                            runtime.file_storage_registry.clone(),
                        )
                        .sources_selected(
                            &scope.file_table_ids.into_iter().collect(),
                            scope.include_runtime_debug_artifacts,
                        )
                        .await
                        .map_err(|_| SystemBackupRuntimeError::SourceInventory)?,
                    );
                }
                let command = CreateSystemBackupCommand {
                    actor_user_id,
                    application_build: runtime.application_build.clone(),
                    migration_head: storage_durable_postgres::migration_head(runtime.store.pool())
                        .await
                        .map_err(|_| SystemBackupRuntimeError::PostgreSqlPreflight)?,
                    master_key_fingerprint: runtime.master_key_fingerprint.clone(),
                    portable_source_master_key_base64: Some(
                        runtime.portable_source_master_key_base64.clone(),
                    ),
                    backup_password,
                };
                let sealed = runtime
                    .service
                    .create_queued_backup_under_existing_maintenance(
                        queued.backup_job_id,
                        queued.backup_set_id,
                        command,
                        sources,
                    )
                    .await?;
                runtime
                    .repository
                    .record_verification(sealed.manifest().backup_set_id(), true)
                    .await
                    .map_err(|_| SystemBackupRuntimeError::Repository)?;
                Ok::<(), SystemBackupRuntimeError>(())
            }
            .await;
            if let Err(error) = result {
                tracing::warn!(backup_job_id = %queued.backup_job_id.as_uuid(), error = %error, "selected backup failed");
                // The service owns terminal creation failures; avoid a second terminal event.
                if !matches!(error, SystemBackupRuntimeError::Service(_)) {
                    let _ = runtime
                        .service
                        .fail_queued_manual_backup(
                            queued.backup_job_id,
                            queued.backup_set_id,
                            actor_user_id,
                            "selective_backup_failed",
                        )
                        .await;
                }
            }
            lease.finish();
        });
        Ok(queued)
    }

    pub(super) async fn selective_preflight(
        &self,
        sealed: SealedBackupManifest,
        password: Option<&str>,
    ) -> RecoveryPlan {
        let backup_set_id = sealed.manifest().backup_set_id();
        let mut failures = Vec::new();
        let preview = match self
            .selective_service
            .preflight(backup_set_id, password, &self.target_master_key)
            .await
        {
            Ok(preview) => Some(preview),
            Err(error) => {
                tracing::warn!(backup_set_id = %backup_set_id.as_uuid(), error = %error, "selective recovery preflight failed");
                failures.push(RecoveryPreflightFailure::BackupIntegrity);
                None
            }
        };
        let required_space_bytes = sealed.manifest().total_size_bytes().saturating_mul(3);
        let staging_root = std::env::temp_dir();
        let disks = sysinfo::Disks::new_with_refreshed_list();
        let available_space_bytes = disks
            .list()
            .iter()
            .filter(|disk| staging_root.starts_with(disk.mount_point()))
            .max_by_key(|disk| disk.mount_point().as_os_str().len())
            .map(|disk| disk.available_space());
        match available_space_bytes {
            None => failures.push(RecoveryPreflightFailure::TargetProbe),
            Some(available) if available < required_space_bytes => {
                failures.push(RecoveryPreflightFailure::InsufficientSpace)
            }
            _ => {}
        }
        RecoveryPlan {
            backup_set_id,
            required_space_bytes,
            available_space_bytes: available_space_bytes.unwrap_or(0),
            impact: RecoveryImpactPreview {
                database_replaced: false,
                business_object_count: sealed
                    .manifest()
                    .components()
                    .iter()
                    .filter(|component| {
                        component.kind == domain::BackupComponentKind::BusinessObject
                    })
                    .count() as u64,
                extension_artifact_count: 0,
                mcp_artifact_count: 0,
                active_work: Vec::new(),
            },
            failures,
            selective: preview,
        }
    }

    pub async fn restore_selected_backup(
        self: &Arc<Self>,
        intent: ConfirmedRecoveryIntent,
        password: Option<String>,
        confirm_missing_plugins: bool,
    ) -> Result<(), SystemBackupRuntimeError> {
        let runtime = self.clone();
        // The worker owns maintenance and staged files even if the HTTP caller disconnects.
        tokio::spawn(async move {
            runtime
                .restore_selected_backup_owned(intent, password.as_deref(), confirm_missing_plugins)
                .await
        })
        .await
        .map_err(|error| SystemBackupRuntimeError::Selective(error.into()))?
    }

    async fn restore_selected_backup_owned(
        &self,
        intent: ConfirmedRecoveryIntent,
        password: Option<&str>,
        confirm_missing_plugins: bool,
    ) -> Result<(), SystemBackupRuntimeError> {
        let lease = self.reserve_recovery_maintenance(intent.recovery_job_id())?;
        lease
            .wait_for_drain(std::time::Duration::from_secs(30))
            .await
            .map_err(|_| SystemBackupRuntimeError::Drain)?;
        let result = async {
            let plan = self
                .preflight_with_password(intent.backup_set_id(), password)
                .await;
            if !plan.is_compatible()
                || plan.selective.is_none()
                || control_plane::system_recovery::recovery_plan_digest(&plan)?
                    != *intent.plan_digest()
            {
                anyhow::bail!("selective recovery plan changed");
            }
            self.append_selective_recovery_event(
                &intent,
                BackupJournalEventKind::RecoveryStateChanged {
                    state: domain::RecoveryJobState::Restoring,
                },
            )
            .await?;
            self.selective_service
                .restore(
                    intent.backup_set_id(),
                    password,
                    &self.target_master_key,
                    confirm_missing_plugins,
                    intent.recovery_job_id(),
                )
                .await?;
            if let Err(error) = self.append_selective_recovery_event(
                &intent,
                BackupJournalEventKind::RecoveryStateChanged {
                    state: domain::RecoveryJobState::Succeeded,
                },
            ).await {
                // Database and files have committed. Retrying import because an audit write failed
                // would misrepresent the durable outcome.
                tracing::error!(recovery_job_id = %intent.recovery_job_id().as_uuid(), error = %error, "selective restore succeeded but journal update failed");
            }
            Ok::<(), anyhow::Error>(())
        }
        .await;
        let needs_repair = result.as_ref().err().is_some_and(|error| {
            error
                .downcast_ref::<control_plane::system_backup::SelectiveRestoreNeedsRepair>()
                .is_some()
        });
        if needs_repair {
            let _ = self
                .append_selective_recovery_event(
                    &intent,
                    BackupJournalEventKind::RecoveryStateChanged {
                        state: domain::RecoveryJobState::ManualRecoveryRequired,
                    },
                )
                .await;
        }
        if result.is_err() {
            let _ = self
                .append_selective_recovery_event(
                    &intent,
                    BackupJournalEventKind::TerminalFailure {
                        code: "selective_restore_failed".to_owned(),
                    },
                )
                .await;
        }
        if needs_repair {
            lease.retain();
        } else {
            lease.finish();
        }
        result.map_err(Into::into)
    }

    async fn append_selective_recovery_event(
        &self,
        intent: &ConfirmedRecoveryIntent,
        event: BackupJournalEventKind,
    ) -> anyhow::Result<()> {
        let subject = BackupJournalSubject::Recovery(intent.recovery_job_id());
        let events = self.repository.read_journal(subject).await?;
        self.repository
            .append_journal_event(&BackupJournalEvent {
                event_id: Uuid::now_v7(),
                sequence: events.last().map_or(0, |event| event.sequence + 1),
                subject,
                backup_set_id: intent.backup_set_id(),
                actor_user_id: Some(intent.actor_user_id()),
                occurred_at: time::OffsetDateTime::now_utc(),
                event,
            })
            .await?;
        Ok(())
    }
}
