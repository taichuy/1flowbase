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
    provider_runs: Mutex<HashMap<Uuid, ProviderRunActivity>>,
    cancellation_failures: Mutex<HashMap<Uuid, String>>,
}

struct ProviderRunActivity {
    session: Arc<SessionActivity>,
    registered: bool,
}

pub(crate) struct SessionActivity {
    id: Uuid,
    epoch: i64,
    state: Mutex<SessionState>,
    changed: watch::Sender<usize>,
}
struct SessionState {
    cancelled: bool,
    leases: usize,
    cancellation_failure: Option<String>,
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
    pub(crate) fn cancel(&self) {
        let mut state = lock(&self.state);
        state.cancelled = true;
        self.changed.send_replace(state.leases);
    }
    fn fail_cancellation(&self, message: String) {
        let mut state = lock(&self.state);
        state.cancellation_failure.get_or_insert(message);
        self.changed.send_replace(state.leases);
    }
    async fn cancel_and_join(&self) -> Result<()> {
        let mut changed = self.changed.subscribe();
        self.cancel();
        loop {
            {
                let state = lock(&self.state);
                if let Some(failure) = &state.cancellation_failure {
                    anyhow::bail!("communication_provider_cancel_join_failed: {failure}");
                }
                if state.leases == 0 {
                    return Ok(());
                }
            }
            changed
                .changed()
                .await
                .map_err(|_| anyhow::anyhow!("communication_join_owner_lost"))?;
        }
    }
}
impl CommunicationActivity {
    pub(crate) fn session(&self, id: Uuid, epoch: i64) -> Result<Arc<SessionActivity>> {
        ensure!(
            !lock(&self.cancellation_failures).contains_key(&id),
            "communication_provider_cancel_join_failed"
        );
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
            id,
            epoch,
            state: Mutex::new(SessionState {
                cancelled: false,
                leases: 0,
                cancellation_failure: None,
            }),
            changed,
        });
        sessions.insert(id, Arc::downgrade(&session));
        Ok(session)
    }
    pub(crate) async fn cancel_session(&self, id: Uuid) -> Result<()> {
        let session = lock(&self.sessions).get(&id).and_then(Weak::upgrade);
        if let Some(session) = session {
            session.cancel_and_join().await?;
        }
        if let Some(failure) = lock(&self.cancellation_failures).get(&id) {
            anyhow::bail!("communication_provider_cancel_join_failed: {failure}");
        }
        lock(&self.provider_runs).retain(|_, run| run.session.id != id || run.registered);
        Ok(())
    }

    pub(crate) fn register_provider_run(
        self: &Arc<Self>,
        run_id: Uuid,
        session: Arc<SessionActivity>,
    ) -> Result<ProviderRunRegistration> {
        let _admission = session.enter()?;
        let mut runs = lock(&self.provider_runs);
        ensure!(
            !runs.contains_key(&run_id),
            "communication_provider_run_already_owned"
        );
        runs.insert(
            run_id,
            ProviderRunActivity {
                session: session.clone(),
                registered: true,
            },
        );
        Ok(ProviderRunRegistration {
            activity: self.clone(),
            run_id,
            session,
        })
    }

    pub(crate) async fn execute_provider(
        self: &Arc<Self>,
        run_id: Uuid,
        backend: &orchestration_runtime::runtime_backend::OrchestrationRuntimeBackend,
        request: runtime_core::runtime_backend::RuntimeExecutionRequest,
        sinks: runtime_core::runtime_backend::RuntimeStreamSinks,
    ) -> std::result::Result<
        runtime_core::runtime_backend::RuntimeExecutionOutcome,
        runtime_core::runtime_backend::RuntimeBackendError,
    > {
        use runtime_core::runtime_backend::RuntimeBackendError;
        let session = lock(&self.provider_runs)
            .get(&run_id)
            .map(|run| run.session.clone());
        let Some(session) = session else {
            return backend.execute_stream(request, sinks).await;
        };
        let request_id = request.request_id.clone();
        let lease = session
            .enter()
            .map_err(|_| RuntimeBackendError::Cancelled(request_id.clone()))?;
        let backend = backend.clone();
        let activity = self.clone();
        let task = tokio::spawn(async move {
            let mut owner = ProviderRequestOwner {
                activity,
                session: session.clone(),
                _lease: lease,
                completed: false,
            };
            let execution = backend.execute_stream(request, sinks);
            tokio::pin!(execution);
            let result = tokio::select! {
                biased;
                _ = session.cancelled() => {
                    match backend.cancel(&request_id).await {
                        Ok(_) => Err(RuntimeBackendError::Cancelled(request_id)),
                        Err(error) => {
                            owner.failed(error.to_string());
                            Err(error)
                        }
                    }
                }
                result = &mut execution => result,
            };
            owner.completed = true;
            result
        });
        task.await.map_err(|error| RuntimeBackendError::Execution {
            target_id: run_id.to_string(),
            message: format!("communication provider owner failed: {error}"),
        })?
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
pub(crate) struct ProviderRunRegistration {
    activity: Arc<CommunicationActivity>,
    run_id: Uuid,
    session: Arc<SessionActivity>,
}
impl Drop for ProviderRunRegistration {
    fn drop(&mut self) {
        // Keep a cancelled run fenced until every callback/provider lease has joined.
        let state = lock(&self.session.state);
        let mut runs = lock(&self.activity.provider_runs);
        if !state.cancelled || (state.leases == 0 && state.cancellation_failure.is_none()) {
            runs.remove(&self.run_id);
        } else if let Some(run) = runs.get_mut(&self.run_id) {
            run.registered = false;
        }
    }
}

struct ProviderRequestOwner {
    activity: Arc<CommunicationActivity>,
    session: Arc<SessionActivity>,
    _lease: SessionLease,
    completed: bool,
}
impl ProviderRequestOwner {
    fn failed(&self, message: String) {
        lock(&self.activity.cancellation_failures)
            .entry(self.session.id)
            .or_insert_with(|| message.clone());
        self.session.fail_cancellation(message);
    }
}
impl Drop for ProviderRequestOwner {
    fn drop(&mut self) {
        if !self.completed {
            self.failed("communication provider execution owner unwound before join".into());
        }
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
pub(crate) mod tests;
