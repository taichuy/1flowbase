use crate::PgControlPlaneStore;
use anyhow::Result;
use async_trait::async_trait;
use control_plane_contracts::{
    ports::{
        ContributionAuthorityLease, GrantContributionAuthorizationInput,
        PluginContributionAuthorityRepository, RevokeContributionAuthorizationInput,
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
    transaction: Transaction<'static, Postgres>,
    snapshot: PluginContributionAuthoritySnapshot,
}
impl ContributionAuthorityLease for PgContributionAuthorityLease {
    fn snapshot(&self) -> &PluginContributionAuthoritySnapshot {
        &self.snapshot
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
    // Assignment and installation locks also prevent these ownership facts changing while
    // a host admission lease is held. Every authority writer takes locks in this order.
    let installation: Option<Uuid> = sqlx::query_scalar(
        "select id from extension_installations where id=$1 and plugin_id is not null for share",
    )
    .bind(installation_id)
    .fetch_optional(&mut *transaction)
    .await?;
    if installation.is_none() {
        return Err(Error::NotFound("plugin_installation").into());
    }
    let assignment: Option<Uuid> = sqlx::query_scalar(
        "select id from plugin_assignments where installation_id=$1 and workspace_id=$2 for share",
    )
    .bind(installation_id)
    .bind(workspace_id)
    .fetch_optional(&mut *transaction)
    .await?;
    if assignment.is_none() {
        return Err(Error::PermissionDenied("contribution_workspace_assignment_required").into());
    }
    sqlx::query("insert into plugin_contribution_authorization_revisions (installation_id,workspace_id) values ($1,$2) on conflict do nothing")
        .bind(installation_id).bind(workspace_id).execute(&mut *transaction).await?;
    sqlx::query("select revision from plugin_contribution_authorization_revisions where installation_id=$1 and workspace_id=$2 for update")
        .bind(installation_id).bind(workspace_id).fetch_one(&mut *transaction).await?;
    Ok(transaction)
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

#[async_trait]
impl PluginContributionAuthorityRepository for PgControlPlaneStore {
    async fn grant_contribution_authorization(
        &self,
        input: &GrantContributionAuthorizationInput,
    ) -> Result<PluginContributionAuthoritySnapshot> {
        let mut transaction = begin_locked(self, input.installation_id, input.workspace_id).await?;
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
        let mut transaction = begin_locked(self, input.installation_id, input.workspace_id).await?;
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
        let mut transaction = begin_locked(self, installation_id, workspace_id).await?;
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
        Ok(Box::new(PgContributionAuthorityLease {
            transaction,
            snapshot,
        }))
    }
}
