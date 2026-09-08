//! Explicit host references cover frozen invocations and future Outbox transactions.
use super::*;
use std::sync::{Arc, Mutex};

#[derive(Default)]
pub(super) struct SnapshotLifetime(Mutex<LifetimeState>);
#[derive(Default)]
struct LifetimeState {
    references: usize,
    closed: bool,
    retired: bool,
}
pub(crate) struct SnapshotReference(Arc<SnapshotLifetime>);
impl SnapshotLifetime {
    pub(super) fn ensure_open(&self) -> Result<()> {
        let state = self.0.lock().unwrap_or_else(|error| error.into_inner());
        if state.closed || state.retired {
            return Err(control_plane::errors::ControlPlaneError::Conflict(
                "managed_execution_retired_or_closing",
            )
            .into());
        }
        Ok(())
    }

    pub(super) fn reference_count(&self) -> usize {
        self.0
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .references
    }

    pub(super) fn acquire(self: &Arc<Self>) -> Result<SnapshotReference> {
        let mut state = self.0.lock().unwrap_or_else(|e| e.into_inner());
        if state.closed || state.retired {
            bail!("managed snapshot is closed for retirement");
        }
        state.references += 1;
        Ok(SnapshotReference(self.clone()))
    }
    pub(super) fn close(self: &Arc<Self>) -> Result<SnapshotRetirement> {
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
    }
}
pub(super) struct SnapshotRetirement(Arc<SnapshotLifetime>);
impl SnapshotRetirement {
    pub(super) fn ensure_unreferenced(&self) -> Result<()> {
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
    pub(super) fn retire(&self) {
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
    pub(crate) fn freeze_reference(&self) -> Result<SnapshotReference> {
        self.lifetime.acquire()
    }
    pub(crate) fn freeze_publication(
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
