//! Explicit host references cover frozen invocations and future Outbox transactions.
use super::*;
use std::sync::{Arc, Mutex};

#[derive(Default)]
pub struct SnapshotLifetime(
    Mutex<LifetimeState>,
    tokio::sync::Notify,
    Option<Arc<tokio::sync::Notify>>,
);
#[derive(Default)]
struct LifetimeState {
    references: usize,
    closed: bool,
    retired: bool,
}
pub struct SnapshotReference(Arc<SnapshotLifetime>);
impl SnapshotLifetime {
    #[doc(hidden)]
    #[cfg(any(test, feature = "test-support"))]
    pub fn set_reference_count_for_test_support(&self, references: usize) {
        self.0.lock().unwrap().references = references;
    }

    pub fn with_reclamation_notify(notify: Arc<tokio::sync::Notify>) -> Self {
        Self(Mutex::default(), tokio::sync::Notify::new(), Some(notify))
    }
    pub fn close_for_shutdown(&self) {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).closed = true;
    }
    pub async fn wait_references(&self) {
        loop {
            let notified = self.1.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            if self.reference_count() == 0 {
                return;
            }
            notified.await;
        }
    }
    pub fn ensure_open(&self) -> Result<()> {
        let state = self.0.lock().unwrap_or_else(|error| error.into_inner());
        if state.closed || state.retired {
            return Err(control_plane::errors::ControlPlaneError::Conflict(
                "managed_execution_retired_or_closing",
            )
            .into());
        }
        Ok(())
    }

    pub fn reference_count(&self) -> usize {
        self.0
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .references
    }

    pub fn acquire(self: &Arc<Self>) -> Result<SnapshotReference> {
        let mut state = self.0.lock().unwrap_or_else(|e| e.into_inner());
        if state.closed || state.retired {
            bail!("managed snapshot is closed for retirement");
        }
        state.references = state
            .references
            .checked_add(1)
            .context("managed snapshot reference count is not representable")?;
        Ok(SnapshotReference(self.clone()))
    }
    pub fn close(self: &Arc<Self>) -> Result<SnapshotRetirement> {
        let mut state = self.0.lock().unwrap_or_else(|e| e.into_inner());
        if state.closed || state.retired {
            bail!("managed snapshot retirement already started");
        }
        state.closed = true;
        Ok(SnapshotRetirement(self.clone()))
    }
}
impl Drop for SnapshotReference {
    fn drop(&mut self) {
        self.0
             .0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .references -= 1;
        self.0 .1.notify_waiters();
        if let Some(notify) = &self.0 .2 {
            notify.notify_one();
        }
    }
}
pub struct SnapshotRetirement(Arc<SnapshotLifetime>);
impl SnapshotRetirement {
    pub fn ensure_unreferenced(&self) -> Result<()> {
        if self
            .0
             .0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .references
            != 0
        {
            return Err(control_plane::errors::ControlPlaneError::Conflict(
                "managed_snapshot_has_frozen_references",
            )
            .into());
        }
        Ok(())
    }
    pub fn retire(&self) {
        self.0 .0.lock().unwrap_or_else(|e| e.into_inner()).retired = true;
    }
}
impl Drop for SnapshotRetirement {
    fn drop(&mut self) {
        let mut state = self.0 .0.lock().unwrap_or_else(|e| e.into_inner());
        if !state.retired {
            state.closed = false;
        }
    }
}
impl ManagedWorkspaceSnapshot {
    pub fn freeze_reference(&self) -> Result<SnapshotReference> {
        self.lifetime.acquire()
    }
    pub fn freeze_publication(
        &self,
        contract: &str,
        version: &str,
    ) -> Result<Option<control_plane_contracts::ports::FrozenLifecyclePublication>> {
        let reference = self.freeze_reference()?;
        Ok(self.publication_plan(contract, version).map(|plan| {
            control_plane_contracts::ports::FrozenLifecyclePublication::managed(
                plan,
                Box::new(reference),
            )
        }))
    }
}
