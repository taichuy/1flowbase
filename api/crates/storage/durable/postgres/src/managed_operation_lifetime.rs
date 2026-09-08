//! Finite in-process lifetime owner, constructed with the canonical PostgreSQL store.
use control_plane_contracts::ports::{
    ManagedOperationLifetime, ManagedOperationPermit, ManagedOwnedOperation,
    ManagedOwnedOperationSnapshot,
};
use std::sync::{Arc, Mutex};
use tokio::sync::Notify;
#[derive(Default)]
struct State {
    snapshot: ManagedOwnedOperationSnapshot,
}
struct Inner {
    state: Mutex<State>,
    changed: Notify,
    capacity: usize,
}
#[derive(Clone)]
pub(crate) struct ManagedOperationOwner(Arc<Inner>);
struct OwnedPermit {
    owner: ManagedOperationOwner,
    index: usize,
}
impl Default for ManagedOperationOwner {
    fn default() -> Self {
        Self::new(128)
    }
}
impl ManagedOperationOwner {
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0);
        Self(Arc::new(Inner {
            state: Mutex::new(State::default()),
            changed: Notify::new(),
            capacity,
        }))
    }
}

#[async_trait::async_trait]
impl ManagedOperationLifetime for ManagedOperationOwner {
    fn admit(
        &self,
        operation: ManagedOwnedOperation,
    ) -> anyhow::Result<Box<dyn ManagedOperationPermit>> {
        let mut state = self.0.state.lock().unwrap_or_else(|e| e.into_inner());
        if state.snapshot.closed {
            anyhow::bail!("managed owned operations closed");
        }
        if state.snapshot.active.iter().sum::<usize>() >= self.0.capacity {
            anyhow::bail!("managed owned operation capacity exhausted");
        }
        let index = operation as usize;
        state.snapshot.active[index] += 1;
        Ok(Box::new(OwnedPermit {
            owner: self.clone(),
            index,
        }))
    }
    fn close(&self) {
        self.0
            .state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .snapshot
            .closed = true;
        self.0.changed.notify_waiters();
    }
    fn snapshot(&self) -> ManagedOwnedOperationSnapshot {
        self.0
            .state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .snapshot
    }
    /// Timeout drops only the waiter. Owners retain permits until real completion.
    async fn wait(&self, budget: std::time::Duration) -> anyhow::Result<()> {
        tokio::time::timeout(budget, async {
            loop {
                let notified = self.0.changed.notified();
                tokio::pin!(notified);
                notified.as_mut().enable();
                let snapshot = self.snapshot();
                if !snapshot.closed {
                    anyhow::bail!("managed owned operations must close before wait");
                }
                if snapshot.active.iter().all(|n| *n == 0) {
                    return Ok(());
                }
                notified.await;
            }
        })
        .await
        .map_err(|_| {
            anyhow::anyhow!("managed owned operations unfinished: {:?}", self.snapshot())
        })?
    }
}
impl ManagedOperationPermit for OwnedPermit {}
impl Drop for OwnedPermit {
    fn drop(&mut self) {
        self.owner
            .0
            .state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .snapshot
            .active[self.index] -= 1;
        self.owner.0.changed.notify_waiters();
    }
}

#[cfg(test)]
#[path = "_tests/managed_operation_lifetime.rs"]
mod tests;
