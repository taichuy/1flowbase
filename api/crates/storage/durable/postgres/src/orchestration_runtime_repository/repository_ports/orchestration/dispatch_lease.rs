//! Initial execution owns one session lock, without holding a transaction over provider I/O.
use crate::repositories::PgControlPlaneStore;
use anyhow::Result;
use control_plane_contracts::ports::FlowRunDispatchLease;
use sqlx::{pool::PoolConnection, Postgres};
use uuid::Uuid;

struct PgFlowRunDispatchLease {
    connection: Option<PoolConnection<Postgres>>,
}
impl FlowRunDispatchLease for PgFlowRunDispatchLease {}
impl Drop for PgFlowRunDispatchLease {
    fn drop(&mut self) {
        // Includes cancellation while acquiring: the server may have granted a lock even
        // when its result did not reach this future. Never leak session locks into the pool.
        if let Some(connection) = self.connection.as_mut() {
            connection.close_on_drop();
        }
    }
}

pub(super) async fn acquire(
    store: &PgControlPlaneStore,
    application_id: Uuid,
    run_id: Uuid,
) -> Result<Option<Box<dyn FlowRunDispatchLease>>> {
    let mut guard = PgFlowRunDispatchLease {
        connection: Some(store.pool().acquire().await?),
    };
    let connection = guard.connection.as_mut().expect("new dispatch connection");
    let acquired: bool =
        sqlx::query_scalar("select pg_try_advisory_lock(hashtextextended($1,2012))")
            .bind(format!("flow-dispatch:{application_id}:{run_id}"))
            .fetch_one(&mut **connection)
            .await?;
    if !acquired {
        // A confirmed non-owner holds no session lock, and can safely return its connection.
        guard.connection.take();
        return Ok(None);
    }
    Ok(Some(Box::new(guard)))
}
