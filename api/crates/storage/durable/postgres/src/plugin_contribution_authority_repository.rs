use crate::PgControlPlaneStore;
use anyhow::Result;
use async_trait::async_trait;
use control_plane_contracts::{
    ports::{
        ContributionAuthorityLease, GrantContributionAuthorizationInput, ManagedInstallationSwitch,
        ManagedInstallationSwitchLease, PluginContributionAuthorityRepository,
        RevokeContributionAuthorizationInput,
    },
    ControlPlaneContractError as Error,
};
use domain::{
    ContributionAuthorizationStatus, PluginContributionAuthoritySnapshot,
    PluginContributionAuthorization,
};
use extension_contracts::extension_bus::ManagedContributionSubject;
use sqlx::{Postgres, Row, Transaction};
use std::{future::Future, pin::Pin};
use uuid::Uuid;

struct PgContributionAuthorityLease {
    managed_operations:
        std::sync::Arc<dyn control_plane_contracts::ports::ManagedOperationLifetime>,
    transaction: Transaction<'static, Postgres>,
    snapshots: Vec<PluginContributionAuthoritySnapshot>,
    installations: std::collections::BTreeMap<Uuid, domain::PluginInstallationRecord>,
}
struct PgManagedInstallationSwitchLease {
    authority: PgContributionAuthorityLease,
    workspace_id: Uuid,
    current: Uuid,
    target: Uuid,
    node_id: String,
}
impl ManagedInstallationSwitchLease for PgManagedInstallationSwitchLease {
    fn authority(&self) -> &dyn ContributionAuthorityLease {
        &self.authority
    }
    fn release(self: Box<Self>) -> Pin<Box<dyn Future<Output = Result<()>> + Send>> {
        Box::pin(async move {
            self.authority.transaction.commit().await?;
            Ok(())
        })
    }
    fn commit(
        mut self: Box<Self>,
        audit_log: domain::AuditLogRecord,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send>> {
        Box::pin(async move {
            let revision = self
                .authority
                .snapshots
                .iter()
                .find(|snapshot| snapshot.installation_id == self.target)
                .ok_or(Error::InvalidInput("managed_candidate_authority"))?
                .revision;
            let transaction = &mut self.authority.transaction;
            let changed = sqlx::query("update plugin_assignments set installation_id=$3,assigned_by=$4 where workspace_id=$1 and installation_id=$2")
                .bind(self.workspace_id).bind(self.current).bind(self.target).bind(audit_log.actor_user_id).execute(&mut **transaction).await?.rows_affected();
            if changed != 1 {
                return Err(Error::Conflict("managed_assignment_changed").into());
            }
            sqlx::query("update extension_installations set desired_state='active_requested',updated_at=now(),updated_by=$2 where id=$1")
                .bind(self.target).bind(audit_log.actor_user_id).execute(&mut **transaction).await?;
            sqlx::query("update extension_artifact_instances set is_current=(installation_id=$3),runtime_status=case when installation_id=$3 then 'active' else runtime_status end,availability_status=case when installation_id=$3 then 'available' else availability_status end,last_error=case when installation_id=$3 then null else last_error end,checked_at=now() where node_id=$1 and installation_id in ($2,$3)")
                .bind(&self.node_id).bind(self.current).bind(self.target).execute(&mut **transaction).await?;
            append_audit(transaction, &audit_log, revision).await?;
            self.authority.transaction.commit().await?;
            Ok(())
        })
    }
}

impl ContributionAuthorityLease for PgContributionAuthorityLease {
    fn snapshot(&self) -> &PluginContributionAuthoritySnapshot {
        &self.snapshots[0]
    }
    fn snapshots(&self) -> &[PluginContributionAuthoritySnapshot] {
        &self.snapshots
    }
    fn installation(&self, installation_id: Uuid) -> Option<&domain::PluginInstallationRecord> {
        self.installations.get(&installation_id)
    }
    fn commit_processed_model(
        mut self: Box<Self>,
        subject: ManagedContributionSubject,
        event_id: Uuid,
        effect: extension_contracts::ManagedEventPayload,
        deadline_unix_ms: i64,
    ) -> Pin<Box<dyn Future<Output = Result<extension_contracts::PluginDataResponse>> + Send>> {
        Box::pin(async move {
            use extension_contracts::{
                PluginDataBinding, PluginDataOperation, PluginDataPermission, PluginDataRequest,
                PluginDataTarget, PluginDataValue,
            };
            let installation_id = Uuid::parse_str(subject.installation_id().as_str())?;
            let workspace_id = Uuid::parse_str(subject.workspace_id().as_str())?;
            let installation = self
                .installations
                .get(&installation_id)
                .ok_or(Error::PermissionDenied("managed_effect_installation"))?;
            let authority = self
                .snapshots
                .iter()
                .find(|snapshot| {
                    snapshot.installation_id == installation_id
                        && snapshot.workspace_id == workspace_id
                })
                .ok_or(Error::PermissionDenied("managed_effect_scope"))?;
            if installation.organization != "acme"
                || !matches!(
                    installation.provider_code.as_str(),
                    "acme.composition-b" | "acme.composition-c"
                )
                || installation.desired_state != domain::PluginDesiredState::ActiveRequested
                || !authority.authorizations.iter().any(|grant| {
                    grant.contribution_id == subject.contribution_id().as_str()
                        && grant.point_id == extension_contracts::MANAGED_PROCESSED_EVENT_ID
                        && grant.permission == "plugin_data.owned.write"
                        && grant.permission_contract_id == "plugin-data"
                        && grant.permission_contract_version == "1"
                        && grant.resource_scope
                            == domain::ContributionResourceScope::OwnedCollection {
                                collection_code: "processed_models".into(),
                            }
                        && grant.status == domain::ContributionAuthorizationStatus::Active
                })
            {
                return Err(Error::PermissionDenied(
                    "managed_effect_same_contribution_write_required",
                )
                .into());
            }
            extension_contracts::ManagedEventPublication {
                contract_id: extension_contracts::MANAGED_PROCESSED_EVENT_ID.into(),
                contract_version: "1".into(),
                payload: effect.clone(),
            }
            .validate()?;
            Uuid::parse_str(&effect.model_id)?;
            let binding = PluginDataBinding {
                publisher_namespace: installation.organization.clone(),
                plugin_code: installation.provider_code.clone(),
                plugin_version: installation.plugin_version.clone(),
                storage_binding: "main".into(),
                workspace_id: workspace_id.to_string(),
                actor_id: None,
                // Stable host subject, deliberately independent of worker generation/artifact.
                provider_instance_id: format!(
                    "managed-event:{}:{}:{}",
                    installation_id,
                    workspace_id,
                    subject.contribution_id().as_str()
                ),
                permissions: [PluginDataPermission::Write].into_iter().collect(),
                deadline_unix_ms,
            };
            let request = PluginDataRequest {
                idempotency_key: Some(format!("managed-event/v1/{event_id}")),
                operations: vec![PluginDataOperation::Upsert {
                    target: PluginDataTarget::OwnedCollection {
                        collection_code: "processed_models".into(),
                    },
                    identity: [("model_id".into(), PluginDataValue::Uuid(effect.model_id))]
                        .into_iter()
                        .collect(),
                    values: [
                        ("status".into(), PluginDataValue::String("processed".into())),
                        (
                            "result_reference".into(),
                            effect
                                .result_reference
                                .map(PluginDataValue::String)
                                .unwrap_or(PluginDataValue::Null),
                        ),
                    ]
                    .into_iter()
                    .collect(),
                }],
            };
            let response = crate::plugin_data_repository::execute_request_in_transaction(
                &mut self.transaction,
                &binding,
                &request,
            )
            .await?;
            self.transaction.commit().await?;
            Ok(response)
        })
    }
    fn commit_derived_lifecycle_fact(
        mut self: Box<Self>,
        input: control_plane_contracts::ports::RecordLifecycleFactInput,
        publication: control_plane_contracts::ports::FrozenLifecyclePublication,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<control_plane_contracts::ports::LifecycleOutboxRecord>>
                + Send,
        >,
    > {
        Box::pin(async move {
            let permit = self
                .managed_operations
                .admit(control_plane_contracts::ports::ManagedOwnedOperation::DerivedPublication)?;
            tokio::spawn(async move {
                let _permit = permit;
                let event_id = input.event_id;
                let result = async move {
                    let record =
                crate::lifecycle_outbox_repository::record_derived_lifecycle_fact_in_transaction(
                    &mut self.transaction,
                    &input,
                )
                .await;
                    let record = match record {
                        Ok(record) => record,
                        Err(error) => {
                            self.transaction.rollback().await?;
                            drop(publication);
                            return Err(error);
                        }
                    };
                    self.transaction.commit().await?;
                    drop(publication);
                    Ok(record)
                }
                .await;
                if let Err(error) = &result {
                    tracing::warn!(%event_id,%error,"derived publication transaction owner failed");
                }
                result
            })
            .await
            .map_err(|error| anyhow::anyhow!("derived publication owner terminated: {error}"))?
        })
    }
    fn commit_resume_managed_delivery(
        mut self: Box<Self>,
        input: control_plane_contracts::ports::ResumeManagedLifecycleDelivery,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<control_plane_contracts::ports::LifecycleOutboxRecord>>
                + Send,
        >,
    > {
        Box::pin(async move {
            let record =
                crate::lifecycle_outbox_repository::resume_managed_delivery_in_transaction(
                    &mut self.transaction,
                    &input,
                )
                .await?;
            self.transaction.commit().await?;
            Ok(record)
        })
    }
    fn release(self: Box<Self>) -> Pin<Box<dyn Future<Output = Result<()>> + Send>> {
        Box::pin(async move {
            self.transaction.commit().await?;
            Ok(())
        })
    }
}

async fn begin_locked(
    store: &PgControlPlaneStore,
    installation_id: Uuid,
    workspace_id: Uuid,
) -> Result<Transaction<'static, Postgres>> {
    let mut transaction = store.pool().begin().await?;
    lock_scope(&mut transaction, installation_id, workspace_id).await?;
    Ok(transaction)
}

pub(crate) async fn lock_managed_workspace(
    transaction: &mut Transaction<'_, Postgres>,
    workspace_id: Uuid,
) -> Result<()> {
    sqlx::query(
        "select pg_advisory_xact_lock(hashtextextended('managed-workspace:' || $1::text, 0))",
    )
    .bind(workspace_id.to_string())
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

async fn lock_authority_history(
    transaction: &mut Transaction<'_, Postgres>,
    installation_id: Uuid,
    workspace_id: Uuid,
) -> Result<()> {
    lock_managed_workspace(transaction, workspace_id).await?;
    let installation: Option<Uuid> = sqlx::query_scalar("select id from extension_installations where id=$1 and contract_version='1flowbase.extension-bus/v1' for share")
        .bind(installation_id).fetch_optional(&mut **transaction).await?;
    if installation.is_none() {
        return Err(Error::NotFound("managed_installation").into());
    }
    let assigned: bool = sqlx::query_scalar("select exists(select 1 from plugin_assignments where installation_id=$1 and workspace_id=$2)")
        .bind(installation_id).bind(workspace_id).fetch_one(&mut **transaction).await?;
    if assigned {
        lock_scope(transaction, installation_id, workspace_id).await?;
    } else {
        let revision: Option<i64> = sqlx::query_scalar("select revision from plugin_contribution_authorization_revisions where installation_id=$1 and workspace_id=$2 for update")
            .bind(installation_id).bind(workspace_id).fetch_optional(&mut **transaction).await?;
        if revision.is_none() {
            return Err(Error::PermissionDenied("contribution_workspace_history_required").into());
        }
    }
    Ok(())
}

async fn lock_candidate_grant(
    transaction: &mut Transaction<'_, Postgres>,
    input: &GrantContributionAuthorizationInput,
) -> Result<()> {
    lock_managed_workspace(transaction, input.workspace_id).await?;
    let assigned: bool = sqlx::query_scalar("select exists(select 1 from plugin_assignments where installation_id=$1 and workspace_id=$2)")
        .bind(input.installation_id).bind(input.workspace_id).fetch_one(&mut **transaction).await?;
    if assigned {
        return lock_scope(transaction, input.installation_id, input.workspace_id).await;
    }
    let node_id = input
        .candidate_node_id
        .as_ref()
        .ok_or(Error::PermissionDenied("candidate_node_required"))?;
    let family: Option<Uuid> = sqlx::query_scalar("select a.id from plugin_assignments a join extension_installations assigned_installation on assigned_installation.id=a.installation_id join extension_installations candidate on candidate.id=$1 where a.workspace_id=$2 and a.provider_code=candidate.artifact_id and assigned_installation.organization=candidate.organization and assigned_installation.artifact_id=candidate.artifact_id and assigned_installation.contract_version='1flowbase.extension-bus/v1' and candidate.contract_version='1flowbase.extension-bus/v1' for share of a,assigned_installation,candidate")
        .bind(input.installation_id).bind(input.workspace_id).fetch_optional(&mut **transaction).await?;
    if family.is_none() {
        return Err(Error::PermissionDenied("candidate_workspace_family_required").into());
    }
    let artifact: Option<Uuid> = sqlx::query_scalar("select installation_id from extension_artifact_instances where installation_id=$1 and node_id=$2 and artifact_status='ready' for share")
        .bind(input.installation_id).bind(node_id).fetch_optional(&mut **transaction).await?;
    if artifact.is_none() {
        return Err(Error::PermissionDenied("candidate_node_artifact_required").into());
    }
    sqlx::query("insert into plugin_contribution_authorization_revisions (installation_id,workspace_id) values ($1,$2) on conflict do nothing")
        .bind(input.installation_id).bind(input.workspace_id).execute(&mut **transaction).await?;
    sqlx::query("select revision from plugin_contribution_authorization_revisions where installation_id=$1 and workspace_id=$2 for update")
        .bind(input.installation_id).bind(input.workspace_id).fetch_one(&mut **transaction).await?;
    Ok(())
}

async fn lock_scope(
    transaction: &mut Transaction<'_, Postgres>,
    installation_id: Uuid,
    workspace_id: Uuid,
) -> Result<()> {
    lock_managed_workspace(transaction, workspace_id).await?;
    // Assignment and installation locks also prevent these ownership facts changing while
    // a host admission lease is held. Every authority writer takes locks in this order.
    let installation: Option<Uuid> = sqlx::query_scalar(
        "select id from extension_installations where id=$1 and plugin_id is not null for share",
    )
    .bind(installation_id)
    .fetch_optional(&mut **transaction)
    .await?;
    if installation.is_none() {
        return Err(Error::NotFound("plugin_installation").into());
    }
    let assignment: Option<Uuid> = sqlx::query_scalar(
        "select id from plugin_assignments where installation_id=$1 and workspace_id=$2 for share",
    )
    .bind(installation_id)
    .bind(workspace_id)
    .fetch_optional(&mut **transaction)
    .await?;
    if assignment.is_none() {
        return Err(Error::PermissionDenied("contribution_workspace_assignment_required").into());
    }
    sqlx::query("insert into plugin_contribution_authorization_revisions (installation_id,workspace_id) values ($1,$2) on conflict do nothing")
        .bind(installation_id).bind(workspace_id).execute(&mut **transaction).await?;
    sqlx::query("select revision from plugin_contribution_authorization_revisions where installation_id=$1 and workspace_id=$2 for update")
        .bind(installation_id).bind(workspace_id).fetch_one(&mut **transaction).await?;
    Ok(())
}

async fn snapshot(
    transaction: &mut Transaction<'_, Postgres>,
    installation_id: Uuid,
    workspace_id: Uuid,
    contribution_id: Option<&str>,
) -> Result<PluginContributionAuthoritySnapshot> {
    let revision = sqlx::query_scalar("select revision from plugin_contribution_authorization_revisions where installation_id=$1 and workspace_id=$2")
        .bind(installation_id).bind(workspace_id).fetch_one(&mut **transaction).await?;
    let rows = sqlx::query("select * from plugin_contribution_authorizations where installation_id=$1 and workspace_id=$2 and ($3::text is null or contribution_id=$3) order by contribution_id,permission,id")
        .bind(installation_id).bind(workspace_id).bind(contribution_id).fetch_all(&mut **transaction).await?;
    let authorizations = rows
        .into_iter()
        .map(|row| -> Result<PluginContributionAuthorization> {
            let status: String = row.try_get("status")?;
            Ok(PluginContributionAuthorization {
                id: row.try_get("id")?,
                installation_id: row.try_get("installation_id")?,
                workspace_id: row.try_get("workspace_id")?,
                contribution_id: row.try_get("contribution_id")?,
                point_id: row.try_get("point_id")?,
                permission: row.try_get("permission")?,
                resource_scope: serde_json::from_value(row.try_get("resource_scope")?)?,
                permission_contract_id: row.try_get("permission_contract_id")?,
                permission_contract_version: row.try_get("permission_contract_version")?,
                status: match status.as_str() {
                    "active" => ContributionAuthorizationStatus::Active,
                    "revoked" => ContributionAuthorizationStatus::Revoked,
                    _ => {
                        return Err(Error::InvalidInput("contribution_authorization_status").into())
                    }
                },
                granted_by: row.try_get("granted_by")?,
                revoked_by: row.try_get("revoked_by")?,
                granted_at: row.try_get("granted_at")?,
                revoked_at: row.try_get("revoked_at")?,
                revision: row.try_get("revision")?,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(PluginContributionAuthoritySnapshot {
        installation_id,
        workspace_id,
        revision,
        authorizations,
    })
}

async fn next_revision(
    transaction: &mut Transaction<'_, Postgres>,
    installation_id: Uuid,
    workspace_id: Uuid,
) -> Result<i64> {
    Ok(sqlx::query_scalar("update plugin_contribution_authorization_revisions set revision=revision+1,updated_at=now() where installation_id=$1 and workspace_id=$2 returning revision")
        .bind(installation_id).bind(workspace_id).fetch_one(&mut **transaction).await?)
}

async fn append_audit(
    transaction: &mut Transaction<'_, Postgres>,
    event: &domain::AuditLogRecord,
    revision: i64,
) -> Result<()> {
    let mut payload = event.payload.clone();
    payload["authority_revision"] = serde_json::json!(revision);
    sqlx::query("insert into audit_logs (id,workspace_id,scope_id,actor_user_id,target_type,target_id,event_code,payload,created_by,updated_by,created_at,updated_at) values ($1,$2,$2,$3,$4,$5,$6,$7,$3,$3,$8,$8)")
        .bind(event.id).bind(event.workspace_id).bind(event.actor_user_id).bind(&event.target_type).bind(event.target_id).bind(&event.event_code).bind(&payload).bind(event.created_at)
        .execute(&mut **transaction).await?;
    Ok(())
}

async fn locked_installation(
    transaction: &mut Transaction<'_, Postgres>,
    installation_id: Uuid,
) -> Result<domain::PluginInstallationRecord> {
    let row = sqlx::query("select *, artifact_id as provider_code, artifact_version as plugin_version, receipt->>'legacy_manifest_compatibility' as legacy_manifest_compatibility from extension_installations where id=$1")
        .bind(installation_id).fetch_one(&mut **transaction).await?;
    crate::plugin_repository::map_installation(row)
}

#[async_trait]
impl PluginContributionAuthorityRepository for PgControlPlaneStore {
    async fn lock_managed_installation_switch(
        &self,
        input: &ManagedInstallationSwitch,
    ) -> Result<Box<dyn ManagedInstallationSwitchLease>> {
        let mut transaction = self.pool().begin().await?;
        lock_managed_workspace(&mut transaction, input.workspace_id).await?;
        let mut ids = input
            .installation_ids
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>();
        ids.insert(input.current_installation_id);
        ids.insert(input.target_installation_id);
        for id in &ids {
            sqlx::query("select id from extension_installations where id=$1 for update")
                .bind(id)
                .fetch_one(&mut *transaction)
                .await?;
        }
        let current = locked_installation(&mut transaction, input.current_installation_id).await?;
        let target = locked_installation(&mut transaction, input.target_installation_id).await?;
        if current.organization != target.organization
            || current.provider_code != target.provider_code
            || current.contract_version != "1flowbase.extension-bus/v1"
            || target.contract_version != current.contract_version
        {
            return Err(Error::PermissionDenied("managed_candidate_family_mismatch").into());
        }
        let assignment: Option<Uuid> = sqlx::query_scalar("select id from plugin_assignments where workspace_id=$1 and installation_id=$2 for update")
            .bind(input.workspace_id).bind(current.id).fetch_optional(&mut *transaction).await?;
        if assignment.is_none() {
            return Err(Error::Conflict("managed_assignment_changed").into());
        }
        let artifact: Option<Uuid> = sqlx::query_scalar("select installation_id from extension_artifact_instances where node_id=$1 and installation_id=$2 and artifact_status='ready' for share")
            .bind(&input.node_id).bind(target.id).fetch_optional(&mut *transaction).await?;
        if artifact.is_none() {
            return Err(Error::Conflict("managed_candidate_artifact_unavailable").into());
        }
        let mut active = sqlx::query_scalar::<_,Uuid>("select i.id from plugin_assignments a join extension_installations i on i.id=a.installation_id where a.workspace_id=$1 and i.contract_version='1flowbase.extension-bus/v1' and i.desired_state='active_requested'")
            .bind(input.workspace_id).fetch_all(&mut *transaction).await?.into_iter().collect::<std::collections::BTreeSet<_>>();
        active.insert(current.id);
        active.insert(target.id);
        if active != ids {
            return Err(Error::Conflict("managed_workspace_candidate_changed").into());
        }
        let mut installations = std::collections::BTreeMap::new();
        let mut snapshots = Vec::new();
        for id in ids {
            if id == target.id {
                let revision: Option<i64> = sqlx::query_scalar("select revision from plugin_contribution_authorization_revisions where installation_id=$1 and workspace_id=$2 for update")
                    .bind(id).bind(input.workspace_id).fetch_optional(&mut *transaction).await?;
                if revision.is_none() {
                    return Err(Error::PermissionDenied(
                        "managed_candidate_authorization_required",
                    )
                    .into());
                }
            } else {
                lock_scope(&mut transaction, id, input.workspace_id).await?;
            }
            installations.insert(id, locked_installation(&mut transaction, id).await?);
            snapshots.push(snapshot(&mut transaction, id, input.workspace_id, None).await?);
        }
        Ok(Box::new(PgManagedInstallationSwitchLease {
            authority: PgContributionAuthorityLease {
                managed_operations: self.managed_operations.clone(),
                transaction,
                snapshots,
                installations,
            },
            workspace_id: input.workspace_id,
            current: current.id,
            target: target.id,
            node_id: input.node_id.clone(),
        }))
    }

    async fn lock_contribution_authority_batch(
        &self,
        scopes: &[(Uuid, Uuid)],
    ) -> Result<Box<dyn ContributionAuthorityLease>> {
        let scopes = scopes
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>();
        if scopes.is_empty() {
            return Err(Error::InvalidInput("empty_contribution_authority_batch").into());
        }
        let mut transaction = self.pool().begin().await?;
        for workspace in scopes
            .iter()
            .map(|(_, workspace)| *workspace)
            .collect::<std::collections::BTreeSet<_>>()
        {
            lock_managed_workspace(&mut transaction, workspace).await?;
        }
        let mut snapshots = Vec::new();
        let mut installations = std::collections::BTreeMap::new();
        for (installation_id, workspace_id) in scopes {
            lock_scope(&mut transaction, installation_id, workspace_id).await?;
            installations.insert(
                installation_id,
                locked_installation(&mut transaction, installation_id).await?,
            );
            snapshots.push(snapshot(&mut transaction, installation_id, workspace_id, None).await?);
        }
        Ok(Box::new(PgContributionAuthorityLease {
            managed_operations: self.managed_operations.clone(),
            transaction,
            snapshots,
            installations,
        }))
    }

    async fn contribution_authority_workspaces(&self, installation_id: Uuid) -> Result<Vec<Uuid>> {
        Ok(sqlx::query_scalar("select workspace_id from plugin_assignments where installation_id=$1 order by workspace_id")
            .bind(installation_id).fetch_all(self.pool()).await?)
    }
    async fn lock_installation_contribution_authority(
        &self,
        installation_id: Uuid,
        workspace_id: Uuid,
    ) -> Result<Box<dyn ContributionAuthorityLease>> {
        let mut transaction = begin_locked(self, installation_id, workspace_id).await?;
        let snapshot = snapshot(&mut transaction, installation_id, workspace_id, None).await?;
        let installation = locked_installation(&mut transaction, installation_id).await?;
        Ok(Box::new(PgContributionAuthorityLease {
            managed_operations: self.managed_operations.clone(),
            transaction,
            snapshots: vec![snapshot],
            installations: [(installation_id, installation)].into_iter().collect(),
        }))
    }

    async fn grant_contribution_authorization(
        &self,
        input: &GrantContributionAuthorizationInput,
    ) -> Result<PluginContributionAuthoritySnapshot> {
        let mut transaction = self.pool().begin().await?;
        lock_candidate_grant(&mut transaction, input).await?;
        let observed: time::OffsetDateTime =
            sqlx::query_scalar("select updated_at from extension_installations where id=$1")
                .bind(input.installation_id)
                .fetch_one(&mut *transaction)
                .await?;
        if observed != input.expected_installation_updated_at {
            return Err(Error::Conflict("contribution_installation_changed").into());
        }
        let revision =
            next_revision(&mut transaction, input.installation_id, input.workspace_id).await?;
        sqlx::query("insert into plugin_contribution_authorizations (id,installation_id,workspace_id,contribution_id,point_id,permission,resource_scope,permission_contract_id,permission_contract_version,status,granted_by,revision) values ($1,$2,$3,$4,$5,$6,$7,$8,$9,'active',$10,$11) on conflict (installation_id,workspace_id,contribution_id,point_id,permission,resource_scope,permission_contract_id,permission_contract_version) do update set status='active',granted_by=excluded.granted_by,granted_at=now(),revoked_by=null,revoked_at=null,revision=excluded.revision")
            .bind(Uuid::now_v7()).bind(input.installation_id).bind(input.workspace_id).bind(&input.contribution_id).bind(&input.point_id).bind(&input.permission).bind(serde_json::to_value(&input.resource_scope)?).bind(&input.permission_contract_id).bind(&input.permission_contract_version).bind(input.actor_user_id).bind(revision)
            .execute(&mut *transaction).await?;
        append_audit(&mut transaction, &input.audit_log, revision).await?;
        let snapshot = snapshot(
            &mut transaction,
            input.installation_id,
            input.workspace_id,
            None,
        )
        .await?;
        transaction.commit().await?;
        Ok(snapshot)
    }

    async fn revoke_contribution_authorization(
        &self,
        input: &RevokeContributionAuthorizationInput,
    ) -> Result<PluginContributionAuthoritySnapshot> {
        let mut transaction = self.pool().begin().await?;
        lock_authority_history(&mut transaction, input.installation_id, input.workspace_id).await?;
        let current: i64 = sqlx::query_scalar("select revision from plugin_contribution_authorization_revisions where installation_id=$1 and workspace_id=$2")
            .bind(input.installation_id).bind(input.workspace_id).fetch_one(&mut *transaction).await?;
        if current != input.expected_revision {
            return Err(Error::Conflict("contribution_authority_revision_conflict").into());
        }
        let revision =
            next_revision(&mut transaction, input.installation_id, input.workspace_id).await?;
        let updated = sqlx::query("update plugin_contribution_authorizations set status='revoked',revoked_by=$4,revoked_at=now(),revision=$5 where id=$1 and installation_id=$2 and workspace_id=$3 and status='active'")
            .bind(input.authorization_id).bind(input.installation_id).bind(input.workspace_id).bind(input.actor_user_id).bind(revision).execute(&mut *transaction).await?;
        if updated.rows_affected() != 1 {
            return Err(Error::NotFound("active_contribution_authorization").into());
        }
        let contribution_id: String = sqlx::query_scalar(
            "select contribution_id from plugin_contribution_authorizations where id=$1",
        )
        .bind(input.authorization_id)
        .fetch_one(&mut *transaction)
        .await?;
        crate::lifecycle_outbox_repository::pause_managed_installation_deliveries(
            &mut transaction,
            input.installation_id,
            Some(input.workspace_id),
            Some(&contribution_id),
            control_plane_contracts::ports::LifecycleDeliveryPauseReason::AuthorityRevoked,
        )
        .await?;
        append_audit(&mut transaction, &input.audit_log, revision).await?;
        let snapshot = snapshot(
            &mut transaction,
            input.installation_id,
            input.workspace_id,
            None,
        )
        .await?;
        transaction.commit().await?;
        Ok(snapshot)
    }

    async fn query_contribution_authority(
        &self,
        installation_id: Uuid,
        workspace_id: Uuid,
        audit_log: &domain::AuditLogRecord,
    ) -> Result<PluginContributionAuthoritySnapshot> {
        let mut transaction = self.pool().begin().await?;
        lock_authority_history(&mut transaction, installation_id, workspace_id).await?;
        let snapshot = snapshot(&mut transaction, installation_id, workspace_id, None).await?;
        append_audit(&mut transaction, audit_log, snapshot.revision).await?;
        transaction.commit().await?;
        Ok(snapshot)
    }

    async fn lock_contribution_authority(
        &self,
        subject: &ManagedContributionSubject,
    ) -> Result<Box<dyn ContributionAuthorityLease>> {
        let installation_id = Uuid::parse_str(subject.installation_id().as_str())?;
        let workspace_id = Uuid::parse_str(subject.workspace_id().as_str())?;
        let mut transaction = begin_locked(self, installation_id, workspace_id).await?;
        let snapshot = snapshot(
            &mut transaction,
            installation_id,
            workspace_id,
            Some(subject.contribution_id().as_str()),
        )
        .await?;
        let installation = locked_installation(&mut transaction, installation_id).await?;
        Ok(Box::new(PgContributionAuthorityLease {
            managed_operations: self.managed_operations.clone(),
            transaction,
            snapshots: vec![snapshot],
            installations: [(installation_id, installation)].into_iter().collect(),
        }))
    }
}
