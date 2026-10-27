//! Initial execution owns one session lock, without holding a transaction over provider I/O.
use crate::repositories::PgControlPlaneStore;
use crate::{PgAdvisoryLockKey, PgCoordinationLease};
use anyhow::Result;
use control_plane_contracts::ports::FlowRunDispatchLease;
use tokio::time::Instant;
use uuid::Uuid;

struct PgFlowRunDispatchLease {
    lease: PgCoordinationLease,
}
#[async_trait::async_trait]
impl FlowRunDispatchLease for PgFlowRunDispatchLease {
    async fn wait_for_loss(&mut self) {
        self.lease.wait_for_loss().await;
    }
}

pub(super) async fn acquire(
    store: &PgControlPlaneStore,
    application_id: Uuid,
    run_id: Uuid,
) -> Result<Option<Box<dyn FlowRunDispatchLease>>> {
    // D is independent of business P and managed M. Only an acknowledged false
    // from PG is a non-owner; a configured D budget timeout must remain an error.
    let deadline = Instant::now() + store.connections().settings().business.acquire_timeout;
    Ok(store
        .connections()
        .try_acquire(
            PgAdvisoryLockKey::dispatch(application_id, run_id),
            deadline,
        )
        .await?
        .map(|lease| Box::new(PgFlowRunDispatchLease { lease }) as Box<dyn FlowRunDispatchLease>))
}
