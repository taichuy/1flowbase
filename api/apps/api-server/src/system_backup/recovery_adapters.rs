use std::sync::Arc;

use async_trait::async_trait;
use control_plane::{
    ports::{CacheStore, SessionStore, TaskQueue},
    system_recovery::{
        PostRestoreDependencyError, PostRestoreRecoveryContext, RecoveryAuditProjection,
        RecoveryAuditProjector, RecoveryEphemeralState,
    },
};
use storage_durable::MainDurableStore;

/// Resets the three excluded ephemeral domains through explicit host capabilities.
///
/// Providers which retain the default unsupported implementation fail closed; the recovery
/// service then rolls durable/object/artifact targets back instead of reopening with stale state.
pub struct ApiRecoveryEphemeralState {
    sessions: Arc<dyn SessionStore>,
    cache: Arc<dyn CacheStore>,
    queue: Arc<dyn TaskQueue>,
}

impl ApiRecoveryEphemeralState {
    pub fn new(
        sessions: Arc<dyn SessionStore>,
        cache: Arc<dyn CacheStore>,
        queue: Arc<dyn TaskQueue>,
    ) -> Self {
        Self {
            sessions,
            cache,
            queue,
        }
    }
}

#[async_trait]
impl RecoveryEphemeralState for ApiRecoveryEphemeralState {
    async fn invalidate_after_restore(
        &self,
        _context: &PostRestoreRecoveryContext,
    ) -> Result<(), PostRestoreDependencyError> {
        self.sessions
            .reset_for_system_recovery()
            .await
            .map_err(|_| PostRestoreDependencyError)?;
        self.cache
            .reset_for_system_recovery()
            .await
            .map_err(|_| PostRestoreDependencyError)?;
        self.queue
            .reset_for_system_recovery()
            .await
            .map_err(|_| PostRestoreDependencyError)
    }
}

#[derive(Clone)]
pub struct PostgreSqlRecoveryAuditProjector {
    store: MainDurableStore,
}

impl PostgreSqlRecoveryAuditProjector {
    pub fn new(store: MainDurableStore) -> Self {
        Self { store }
    }
}

#[async_trait]
impl RecoveryAuditProjector for PostgreSqlRecoveryAuditProjector {
    async fn project(
        &self,
        projection: &RecoveryAuditProjection,
    ) -> Result<(), PostRestoreDependencyError> {
        let payload = serde_json::json!({
            "source_event_id": projection.source_event_id,
            "recovery_job_id": projection.recovery_job_id,
            "backup_set_id": projection.backup_set_id,
            "safety_backup_set_id": projection.safety_backup_set_id,
            "outcome": "succeeded",
        });
        let projected = sqlx::query_scalar::<_, uuid::Uuid>(
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
                    'system.recovery.succeeded', $5, $3, $3, $6, $6)
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
        .bind(projection.audit_id)
        .bind(domain::SYSTEM_SCOPE_ID)
        .bind(projection.actor_user_id)
        .bind(projection.recovery_job_id.as_uuid())
        .bind(payload)
        .bind(projection.verified_at)
        .fetch_optional(self.store.pool())
        .await
        .map_err(|_| PostRestoreDependencyError)?;
        if projected == Some(projection.audit_id) {
            Ok(())
        } else {
            Err(PostRestoreDependencyError)
        }
    }
}
