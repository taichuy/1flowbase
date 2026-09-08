//! Explicit host references cover frozen invocations and future Outbox transactions.
use super::*;
use std::sync::{Arc, Mutex};

#[derive(Default)]
pub(super) struct SnapshotLifetime(Mutex<LifetimeState>, tokio::sync::Notify);
#[derive(Default)]
struct LifetimeState {
    references: usize,
    closed: bool,
    retired: bool,
}
pub(crate) struct SnapshotReference(Arc<SnapshotLifetime>);
impl SnapshotLifetime {
    pub(super) fn close_for_shutdown(&self) {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).closed = true;
    }
    pub(super) async fn wait_references(&self) {
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
        if state.references >= 256 {
            bail!("managed snapshot reference capacity exhausted");
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
        self.0 .1.notify_waiters();
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

// Fixed per-host budgets. Refusal never evicts a live exact-target retirement gate.
pub(super) const MAX_CURRENT_WORKSPACES: usize = 256;
pub(super) const MAX_RETAINED_SNAPSHOTS: usize = 256;
pub(super) const MAX_RETIRED_TARGETS: usize = 4096;
impl ManagedSnapshots {
    fn ensure_candidate_capacity(&self, workspace: Uuid) -> Result<()> {
        if self.retired_targets.len() >= MAX_RETIRED_TARGETS {
            bail!("managed retired target capacity exhausted");
        }
        if !self.current.contains_key(&workspace) && self.current.len() >= MAX_CURRENT_WORKSPACES {
            bail!("managed current workspace capacity exhausted");
        }
        Ok(())
    }
    fn ensure_retirement_capacity(&self, target: &str) -> Result<()> {
        if !self.retired_targets.contains_key(target)
            && self.retired_targets.len() >= MAX_RETIRED_TARGETS
        {
            bail!("managed retired target capacity exhausted");
        }
        Ok(())
    }
    fn ensure_publication_capacity(
        &self,
        candidates: &BTreeMap<Uuid, Arc<ManagedWorkspaceSnapshot>>,
    ) -> Result<()> {
        let added_workspaces = candidates
            .keys()
            .filter(|w| !self.current.contains_key(w))
            .count();
        if self.current.len() + added_workspaces > MAX_CURRENT_WORKSPACES {
            bail!("managed current workspace capacity exhausted");
        }
        let mut added = Vec::<Arc<ManagedWorkspaceSnapshot>>::new();
        for (workspace, candidate) in candidates {
            if let Some(old) = self.current.get(workspace) {
                if !candidate.same_execution_snapshot(old)
                    && !self
                        .retained
                        .values()
                        .flatten()
                        .chain(added.iter())
                        .any(|s| s.same_execution_snapshot(old))
                {
                    added.push(old.clone());
                }
            }
        }
        if self.retained.values().map(Vec::len).sum::<usize>() + added.len()
            > MAX_RETAINED_SNAPSHOTS
        {
            bail!("managed retained snapshot capacity exhausted");
        }
        Ok(())
    }
}
impl ManagedExtensionComposition {
    pub(crate) fn close_owned_admission(&self) {
        self.operations.close();
    }
    pub(crate) async fn wait_owned_shutdown(&self, budget: std::time::Duration) -> Result<()> {
        self.operations.wait(budget).await
    }
    pub(crate) async fn wait_for_shutdown(&self, budget: std::time::Duration) -> Result<()> {
        self.operations.wait(budget).await?;
        // Assembly completes before graph gates close; admitted Create/E2 references may finish.
        let lifetimes = {
            let visible = self.snapshots.lock().await;
            visible
                .current
                .values()
                .chain(visible.retained.values().flatten())
                .map(|snapshot| snapshot.lifetime.clone())
                .collect::<Vec<_>>()
        };
        for lifetime in &lifetimes {
            lifetime.close_for_shutdown();
        }
        tokio::time::timeout(budget, async {
            for lifetime in &lifetimes {
                lifetime.wait_references().await;
            }
        })
        .await
        .map_err(|_| {
            anyhow::anyhow!(
                "managed frozen references unfinished: {}",
                lifetimes.iter().map(|l| l.reference_count()).sum::<usize>()
            )
        })?;
        Ok(())
    }
    /// Called only after every owned operation, frozen reference and runtime worker has completed.
    pub(crate) async fn cleanup_after_shutdown(&self) {
        *self.snapshots.lock().await = ManagedSnapshots::default();
    }
}

#[cfg(test)]
#[path = "_tests/lifetime.rs"]
mod tests;
