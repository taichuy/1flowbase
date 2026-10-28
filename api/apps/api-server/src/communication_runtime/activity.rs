//! Process-local cancellation/join owner; durable claims and session epochs remain authoritative.
use anyhow::{ensure, Result};
use control_plane::orchestration_runtime::{
    CommunicationExecutionBinding, CommunicationExecutionResolver,
};
use orchestration_runtime::execution_engine::communication::{
    CommunicationExecutionActivity, CommunicationExecutionLease,
};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex, Weak},
};
use tokio::sync::watch;
use uuid::Uuid;

// Worker unwinding must not prevent remaining leases from releasing or cancellation
// from joining. These locks protect only counters and owned registrations; recover their
// existing state rather than panic again in a lease's Drop implementation.
fn lock<T>(state: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    match state.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

#[derive(Default)]
pub struct CommunicationActivity {
    sessions: Mutex<HashMap<Uuid, Weak<SessionActivity>>>,
    executions: Mutex<HashMap<Uuid, CommunicationExecutionBinding>>,
}

pub(crate) struct SessionActivity {
    epoch: i64,
    state: Mutex<SessionState>,
    changed: watch::Sender<usize>,
}
struct SessionState {
    cancelled: bool,
    leases: usize,
}
pub(crate) struct SessionLease(Arc<SessionActivity>);
impl Drop for SessionLease {
    fn drop(&mut self) {
        let mut state = lock(&self.0.state);
        state.leases -= 1;
        self.0.changed.send_replace(state.leases);
    }
}
impl SessionActivity {
    pub(crate) fn enter(self: &Arc<Self>) -> Result<SessionLease> {
        let mut state = lock(&self.state);
        ensure!(!state.cancelled, "communication_session_cancelled");
        state.leases += 1;
        Ok(SessionLease(self.clone()))
    }
    pub(crate) async fn cancelled(&self) {
        let mut changed = self.changed.subscribe();
        loop {
            if lock(&self.state).cancelled {
                return;
            }
            if changed.changed().await.is_err() {
                return;
            }
        }
    }
    async fn cancel_and_join(&self) {
        let mut changed = self.changed.subscribe();
        {
            let mut state = lock(&self.state);
            state.cancelled = true;
            self.changed.send_replace(state.leases);
        }
        loop {
            if lock(&self.state).leases == 0 {
                return;
            }
            if changed.changed().await.is_err() {
                return;
            }
        }
    }
}
impl CommunicationActivity {
    pub(crate) fn session(&self, id: Uuid, epoch: i64) -> Result<Arc<SessionActivity>> {
        let mut sessions = lock(&self.sessions);
        sessions.retain(|_, entry| entry.strong_count() > 0);
        if let Some(session) = sessions.get(&id).and_then(Weak::upgrade) {
            ensure!(
                session.epoch == epoch,
                "communication_session_epoch_changed"
            );
            ensure!(
                !lock(&session.state).cancelled,
                "communication_session_cancelled"
            );
            return Ok(session);
        }
        let (changed, _) = watch::channel(0);
        let session = Arc::new(SessionActivity {
            epoch,
            state: Mutex::new(SessionState {
                cancelled: false,
                leases: 0,
            }),
            changed,
        });
        sessions.insert(id, Arc::downgrade(&session));
        Ok(session)
    }
    pub(crate) async fn cancel_session(&self, id: Uuid) {
        let session = lock(&self.sessions).get(&id).and_then(Weak::upgrade);
        if let Some(session) = session {
            session.cancel_and_join().await;
        }
    }
    pub(crate) fn register_run(
        self: &Arc<Self>,
        run_id: Uuid,
        binding: CommunicationExecutionBinding,
    ) -> Result<RunRegistration> {
        let mut executions = lock(&self.executions);
        ensure!(
            !executions.contains_key(&run_id),
            "communication_run_already_owned"
        );
        executions.insert(run_id, binding);
        Ok(RunRegistration {
            activity: self.clone(),
            run_id,
        })
    }
}
pub(crate) struct RunRegistration {
    activity: Arc<CommunicationActivity>,
    run_id: Uuid,
}
impl Drop for RunRegistration {
    fn drop(&mut self) {
        lock(&self.activity.executions).remove(&self.run_id);
    }
}

#[async_trait::async_trait]
impl CommunicationExecutionActivity for CommunicationActivity {
    async fn enter(
        &self,
        session_id: Uuid,
        session_epoch: i64,
    ) -> Result<Box<dyn CommunicationExecutionLease>> {
        let session = lock(&self.sessions)
            .get(&session_id)
            .and_then(Weak::upgrade)
            .ok_or_else(|| anyhow::anyhow!("communication_session_owner_missing"))?;
        ensure!(
            session.epoch == session_epoch,
            "communication_session_epoch_changed"
        );
        Ok(Box::new(session.enter()?))
    }
    async fn cancelled(&self, session_id: Uuid, session_epoch: i64) {
        let session = lock(&self.sessions)
            .get(&session_id)
            .and_then(Weak::upgrade);
        if let Some(session) = session.filter(|s| s.epoch == session_epoch) {
            session.cancelled().await;
        }
    }
}
#[async_trait::async_trait]
impl CommunicationExecutionResolver for CommunicationActivity {
    async fn resolve(
        &self,
        run: &domain::FlowRunRecord,
    ) -> Result<Option<CommunicationExecutionBinding>> {
        if run.compatibility_mode.as_deref() != Some("communication") {
            return Ok(None);
        }
        if let Some(binding) = lock(&self.executions).get(&run.id).cloned() {
            return Ok(Some(binding));
        }
        // Pre-upgrade runs had no host-sealed AI session. Keep their existing observation path.
        ensure!(
            run.input_payload
                .pointer("/sys/provider_session_identity")
                .is_none(),
            "communication_turn_owner_unavailable"
        );
        Ok(None)
    }
}

#[cfg(test)]
#[path = "_tests/activity.rs"]
mod tests;
