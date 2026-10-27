use async_trait::async_trait;
use std::sync::Arc;

/// Host-only ownership shared by the adapter, admitted calls, callbacks and reapers.
/// Dropping the last owner requests release; dropping an HTTP future does not prove
/// that external work stopped. The signal is observational and does not retain ownership.
#[derive(Clone)]
pub struct ManagedServiceMutationOwnership {
    pub owner: Arc<dyn Send + Sync>,
    /// True (or unexpected sender closure) revokes local work, never authorizes replay.
    pub lost: tokio::sync::watch::Receiver<bool>,
}

/// Exclusive mutation ownership for a system managed service across API nodes.
#[async_trait]
pub trait ManagedServiceMutationRepository: Send + Sync {
    async fn lock_managed_service_mutation(
        &self,
        installation_id: uuid::Uuid,
        scope_id: uuid::Uuid,
        deadline_unix_ms: i64,
    ) -> anyhow::Result<ManagedServiceMutationOwnership>;
}
