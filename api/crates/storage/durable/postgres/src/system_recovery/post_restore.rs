use anyhow::Result;
use domain::{RecoveryJobId, SYSTEM_SCOPE_ID};
use serde_json::Value;
use sqlx::postgres::PgPoolOptions;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::PgControlPlaneStore;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PostRestoreCatalogReadinessCounts {
    pub permission_definitions: i64,
    pub unhealthy_node_artifacts: i64,
    pub missing_active_installations: i64,
}

#[derive(Debug, Clone)]
pub struct SystemRecoveryAuditLogRow<'a> {
    pub audit_id: Uuid,
    pub actor_user_id: Uuid,
    pub recovery_job_id: RecoveryJobId,
    pub event_code: &'a str,
    pub payload: Value,
    pub occurred_at: OffsetDateTime,
}

pub async fn bump_all_user_session_versions_at_url(database_url: &str) -> Result<u64> {
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(database_url)
        .await?;
    let result = sqlx::query(
        r#"
        update users
           set session_version = session_version + 1,
               updated_at = now()
        "#,
    )
    .execute(&pool)
    .await;
    pool.close().await;
    let receipt = result?;
    Ok(receipt.rows_affected())
}

pub async fn upsert_system_recovery_audit_log_at_url(
    database_url: &str,
    row: SystemRecoveryAuditLogRow<'_>,
) -> Result<bool> {
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(database_url)
        .await?;
    let expected_audit_id = row.audit_id;
    let projected = sqlx::query_scalar::<_, Uuid>(
        r#"
        insert into audit_logs (
            id,
            workspace_id,
            scope_id,
            actor_user_id,
            target_type,
            target_id,
            event_code,
            payload,
            created_by,
            updated_by,
            created_at,
            updated_at
        )
        values ($1, null, $2, $3, 'system_recovery', $4,
                $5, $6, $3, $3, $7, $7)
        on conflict (id) do update set id = excluded.id
         where audit_logs.workspace_id is not distinct from excluded.workspace_id
           and audit_logs.scope_id = excluded.scope_id
           and audit_logs.actor_user_id is not distinct from excluded.actor_user_id
           and audit_logs.target_type = excluded.target_type
           and audit_logs.target_id is not distinct from excluded.target_id
           and audit_logs.event_code = excluded.event_code
           and audit_logs.payload = excluded.payload
           and audit_logs.created_at = excluded.created_at
        returning id
        "#,
    )
    .bind(row.audit_id)
    .bind(SYSTEM_SCOPE_ID)
    .bind(row.actor_user_id)
    .bind(row.recovery_job_id.as_uuid())
    .bind(row.event_code)
    .bind(row.payload)
    .bind(row.occurred_at)
    .fetch_optional(&pool)
    .await;
    pool.close().await;
    Ok(projected? == Some(expected_audit_id))
}

impl PgControlPlaneStore {
    pub async fn connect_for_recovery(database_url: &str) -> Result<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(1)
            .connect(database_url)
            .await?;
        Ok(Self::new(pool))
    }

    pub async fn post_restore_catalog_readiness_counts(
        &self,
        node_id: &str,
    ) -> Result<PostRestoreCatalogReadinessCounts> {
        let pool = self.pool();
        let permission_definitions =
            sqlx::query_scalar::<_, i64>("select count(*) from permission_definitions")
                .fetch_one(pool)
                .await?;
        let unhealthy_node_artifacts = sqlx::query_scalar::<_, i64>(
            r#"
            select count(*)
              from extension_artifact_instances
             where node_id = $1 and artifact_status <> 'ready'
            "#,
        )
        .bind(node_id)
        .fetch_one(pool)
        .await?;
        let missing_active_installations = sqlx::query_scalar::<_, i64>(
            r#"
            select count(*)
              from extension_installations installation
             where installation.desired_state = 'active_requested'
               and not exists (
                   select 1
                     from extension_artifact_instances artifact
                    where artifact.node_id = $1
                      and artifact.installation_id = installation.id
                      and artifact.artifact_status = 'ready'
               )
            "#,
        )
        .bind(node_id)
        .fetch_one(pool)
        .await?;
        Ok(PostRestoreCatalogReadinessCounts {
            permission_definitions,
            unhealthy_node_artifacts,
            missing_active_installations,
        })
    }
}
