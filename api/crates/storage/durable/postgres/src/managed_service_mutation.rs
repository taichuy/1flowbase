use crate::{repositories::PgControlPlaneStore, PgAdvisoryLockKey, PgCoordinationLease};
use async_trait::async_trait;
use control_plane_contracts::ports::{
    ManagedServiceMutationOwnership, ManagedServiceMutationRepository,
};
use std::{sync::Arc, time::Duration};

struct MutationOwner {
    _lease: PgCoordinationLease,
    loss_watch: tokio::task::JoinHandle<()>,
}
impl Drop for MutationOwner {
    fn drop(&mut self) {
        // The watcher only holds a status receiver, never a lease clone. Stop it
        // synchronously when the final Host owner finishes, including healthy calls.
        self.loss_watch.abort();
    }
}

#[async_trait]
impl ManagedServiceMutationRepository for PgControlPlaneStore {
    async fn lock_managed_service_mutation(
        &self,
        installation_id: uuid::Uuid,
        scope_id: uuid::Uuid,
        deadline_unix_ms: i64,
    ) -> anyhow::Result<ManagedServiceMutationOwnership> {
        let remaining = i128::from(deadline_unix_ms)
            - time::OffsetDateTime::now_utc().unix_timestamp_nanos() / 1_000_000;
        anyhow::ensure!(remaining > 0, "managed service deadline exceeded");
        let deadline = tokio::time::Instant::now()
            .checked_add(Duration::from_millis(u64::try_from(remaining)?))
            .ok_or_else(|| anyhow::anyhow!("managed service deadline is not representable"))?;
        let lease = self
            .connections()
            .acquire(
                PgAdvisoryLockKey::managed_mutation(installation_id, scope_id),
                deadline,
            )
            .await?;
        let mut status = lease.subscribe();
        let (lost, receiver) = tokio::sync::watch::channel(lease.status().lost);
        let loss_watch = tokio::spawn(async move {
            loop {
                let current = *status.borrow_and_update();
                if current.lost {
                    lost.send_replace(true);
                    return;
                }
                if current.closed {
                    return;
                }
                if status.changed().await.is_err() {
                    lost.send_replace(true);
                    return;
                }
            }
        });
        Ok(ManagedServiceMutationOwnership {
            owner: Arc::new(MutationOwner {
                _lease: lease,
                loss_watch,
            }),
            lost: receiver,
        })
    }
}
