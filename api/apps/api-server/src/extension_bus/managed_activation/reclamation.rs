//! Event-driven reclamation owns its task until orderly shutdown; no idle operation permit.
use super::*;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::{sync::Notify, task::JoinHandle};

#[derive(Default)]
pub(super) struct ReclamationWorker {
    pub(super) notify: Arc<Notify>,
    stopped: Arc<AtomicBool>,
    task: Mutex<Option<JoinHandle<()>>>,
}

impl ReclamationWorker {
    pub(super) fn stop(&self) {
        self.stopped.store(true, Ordering::Release);
        self.notify.notify_one();
    }

    pub(super) async fn wait_stopped(&self, budget: std::time::Duration) -> Result<()> {
        let mut slot = self.task.lock().await;
        if let Some(task) = slot.as_mut() {
            // Borrow in place: cancelling or timing out a shutdown keeps the join owner.
            tokio::time::timeout(budget, task)
                .await
                .context("managed reclamation owner unfinished")?
                .context("managed reclamation owner terminated")?;
            *slot = None;
        }
        Ok(())
    }
}

impl Drop for ReclamationWorker {
    fn drop(&mut self) {
        // The task keeps only the signal and a Weak composition while idle. A dropped host
        // wakes it for termination; the normal shutdown path explicitly joins it.
        self.stop();
    }
}

impl ManagedExtensionComposition {
    pub(crate) fn start_reclamation(self: &Arc<Self>) {
        let worker = self.reclamation.clone();
        let Ok(mut task) = worker.task.try_lock() else {
            return;
        };
        if task.is_some() || worker.stopped.load(Ordering::Acquire) {
            return;
        }
        let owner = Arc::downgrade(self);
        let notify = worker.notify.clone();
        let stopped = worker.stopped.clone();
        *task = Some(tokio::spawn(async move {
            loop {
                // notify_one retains a permit while a sweep is running. Register before the
                // stop check so shutdown and reference drops cannot be lost at the wait edge.
                let notified = notify.notified();
                tokio::pin!(notified);
                notified.as_mut().enable();
                if stopped.load(Ordering::Acquire) {
                    break;
                }
                notified.await;
                if stopped.load(Ordering::Acquire) {
                    break;
                }
                let Some(owner) = owner.upgrade() else { break };
                owner.reclaim_retained_snapshots().await;
            }
        }));
        // Also sweep snapshots retained before the worker was attached.
        worker.notify.notify_one();
    }

    pub(crate) fn notify_reclamation(&self) {
        self.reclamation.notify.notify_one();
    }

    pub(super) async fn reclaim_retained_snapshots(&self) {
        let assembly = loop {
            let notified = self.reclamation.notify.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            if self.reclamation.stopped.load(Ordering::Acquire) {
                return;
            }
            tokio::select! {
                guard = self.assembly.lock() => break guard,
                _ = notified => {}
            }
        };
        if self.reclamation.stopped.load(Ordering::Acquire) {
            return;
        }
        let Ok(_operation) = self
            .operations
            .admit(control_plane_contracts::ports::ManagedOwnedOperation::Retirement)
        else {
            return;
        };
        let candidates = self
            .snapshots
            .lock()
            .await
            .retained
            .values()
            .flatten()
            .cloned()
            .collect::<Vec<_>>();
        for snapshot in candidates {
            if self.reclamation.stopped.load(Ordering::Acquire) {
                break;
            }
            if snapshot.lifetime.reference_count() != 0 {
                continue;
            }
            // Each full snapshot is independent: one durable backlog must not pin unrelated
            // graphs. The shared retirement path rechecks references and current/shared handles.
            if let Err(error) = self.retire_snapshots_locked(vec![snapshot], &[]).await {
                tracing::debug!(%error, "managed retained snapshot remains protected");
            }
        }
        drop(assembly);
    }
}

#[cfg(test)]
#[path = "_tests/reclamation.rs"]
mod tests;
