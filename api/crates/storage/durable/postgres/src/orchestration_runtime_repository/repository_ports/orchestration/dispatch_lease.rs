//! Initial execution owns one session lock, without holding a transaction over provider I/O.
use crate::repositories::PgControlPlaneStore;
use anyhow::Result;
use control_plane_contracts::ports::FlowRunDispatchLease;
use sqlx::PgConnection;
use uuid::Uuid;

// This owns a raw session, not a pool checkout. PgConnection owns its socket, so
// dropping this guard (including cancellation) closes the session and its locks.
struct PgFlowRunDispatchLease {
    connection: PgConnection,
}
impl FlowRunDispatchLease for PgFlowRunDispatchLease {}

pub(super) async fn acquire(
    store: &PgControlPlaneStore,
    application_id: Uuid,
    run_id: Uuid,
) -> Result<Option<Box<dyn FlowRunDispatchLease>>> {
    let mut guard = PgFlowRunDispatchLease {
        // Detach before any lock query: release the business-pool permit so even a
        // max_connections=1 pool can execute the run's subsequent SQL. Each active
        // initial dispatch uses one extra database session until this guard drops.
        connection: store.pool().acquire().await?.detach(),
    };
    let acquired: bool =
        sqlx::query_scalar("select pg_try_advisory_lock(hashtextextended($1,2012))")
            .bind(format!("flow-dispatch:{application_id}:{run_id}"))
            .fetch_one(&mut guard.connection)
            .await?;
    if !acquired {
        // Non-owners also close their detached session rather than returning it to the pool.
        return Ok(None);
    }
    Ok(Some(Box::new(guard)))
}
