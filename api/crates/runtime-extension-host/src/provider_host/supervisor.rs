use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;

use extension_package_runtime::{
    error::{FrameworkResult, PluginFrameworkError},
    provider_contract::{ProviderStdioRequest, ProviderStreamEvent},
    PluginRuntimeLimits,
};
use serde::Serialize;
use serde_json::Value;
use tokio::sync::{Mutex, Notify};

use crate::stdio_runtime::{
    ProviderWorker, ProviderWorkerCleanupReason, ProviderWorkerCleanupReceipt,
    ProviderWorkerLifecycleState, ProviderWorkerProcessControl, StreamingProviderOutput,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProviderWorkerLifecycleEvent {
    Activated,
    ActivationFailed,
    BeginQuiesce,
    Quiesced,
    RuntimeFailed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProviderWorkerLifecycle {
    state: ProviderWorkerLifecycleState,
    generation: u64,
}

impl ProviderWorkerLifecycle {
    pub(crate) fn activating(generation: u64) -> Self {
        Self {
            state: ProviderWorkerLifecycleState::Activating,
            generation,
        }
    }

    pub(crate) fn state(&self) -> ProviderWorkerLifecycleState {
        self.state
    }

    pub(crate) fn generation(&self) -> u64 {
        self.generation
    }

    pub(crate) fn transition(
        &mut self,
        event: ProviderWorkerLifecycleEvent,
    ) -> FrameworkResult<()> {
        let next = match (self.state, event) {
            (ProviderWorkerLifecycleState::Activating, ProviderWorkerLifecycleEvent::Activated) => {
                ProviderWorkerLifecycleState::Active
            }
            (
                ProviderWorkerLifecycleState::Activating,
                ProviderWorkerLifecycleEvent::ActivationFailed,
            )
            | (ProviderWorkerLifecycleState::Active, ProviderWorkerLifecycleEvent::RuntimeFailed) => {
                ProviderWorkerLifecycleState::Failed
            }
            (
                ProviderWorkerLifecycleState::Activating
                | ProviderWorkerLifecycleState::Active
                | ProviderWorkerLifecycleState::Failed,
                ProviderWorkerLifecycleEvent::BeginQuiesce,
            ) => ProviderWorkerLifecycleState::Quiescing,
            (ProviderWorkerLifecycleState::Quiescing, ProviderWorkerLifecycleEvent::Quiesced) => {
                ProviderWorkerLifecycleState::Inactive
            }
            (state, event) => {
                return Err(PluginFrameworkError::invalid_provider_package(format!(
                    "illegal provider worker lifecycle transition: {state:?} + {event:?}"
                )))
            }
        };
        self.state = next;
        Ok(())
    }
}

#[derive(Debug)]
struct AdmissionState {
    lifecycle: ProviderWorkerLifecycle,
    in_flight: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProviderWorkerSupervisorSnapshot {
    pub state: ProviderWorkerLifecycleState,
    pub generation: u64,
    pub pid: Option<u32>,
    pub in_flight: usize,
}

#[derive(Debug)]
pub(crate) struct ProviderWorkerSupervisor {
    admission: StdMutex<AdmissionState>,
    drained: Notify,
    worker: Mutex<ProviderWorker>,
    process_control: ProviderWorkerProcessControl,
    last_cleanup: StdMutex<Option<ProviderWorkerCleanupReceipt>>,
}

#[derive(Debug)]
struct ProviderWorkerInvocationLease {
    supervisor: Arc<ProviderWorkerSupervisor>,
}

impl Drop for ProviderWorkerInvocationLease {
    fn drop(&mut self) {
        if let Ok(mut admission) = self.supervisor.admission.lock() {
            admission.in_flight = admission.in_flight.saturating_sub(1);
            if admission.in_flight == 0 {
                self.supervisor.drained.notify_waiters();
            }
        }
    }
}

impl ProviderWorkerSupervisor {
    pub(crate) fn activate(
        executable_path: std::path::PathBuf,
        limits: PluginRuntimeLimits,
        generation: u64,
    ) -> FrameworkResult<Arc<Self>> {
        let mut lifecycle = ProviderWorkerLifecycle::activating(generation);
        let mut worker = ProviderWorker::new(executable_path, limits);
        match worker.activate() {
            Ok(_) => {
                lifecycle.transition(ProviderWorkerLifecycleEvent::Activated)?;
            }
            Err(error) => {
                lifecycle.transition(ProviderWorkerLifecycleEvent::ActivationFailed)?;
                return Err(error);
            }
        }
        let process_control = worker.process_control().ok_or_else(|| {
            PluginFrameworkError::invalid_provider_package(
                "activated provider worker has no process control",
            )
        })?;
        Ok(Arc::new(Self {
            admission: StdMutex::new(AdmissionState {
                lifecycle,
                in_flight: 0,
            }),
            drained: Notify::new(),
            worker: Mutex::new(worker),
            process_control,
            last_cleanup: StdMutex::new(None),
        }))
    }

    pub(crate) fn snapshot(&self) -> FrameworkResult<ProviderWorkerSupervisorSnapshot> {
        let admission = self.lock_admission()?;
        Ok(ProviderWorkerSupervisorSnapshot {
            state: admission.lifecycle.state(),
            generation: admission.lifecycle.generation(),
            pid: self.process_control.pid(),
            in_flight: admission.in_flight,
        })
    }

    pub(crate) fn last_cleanup_receipt(
        &self,
    ) -> FrameworkResult<Option<ProviderWorkerCleanupReceipt>> {
        self.last_cleanup
            .lock()
            .map(|receipt| receipt.clone())
            .map_err(|_| lifecycle_lock_error())
    }

    pub(crate) fn begin_quiesce(&self) -> FrameworkResult<()> {
        let mut admission = self.lock_admission()?;
        admission
            .lifecycle
            .transition(ProviderWorkerLifecycleEvent::BeginQuiesce)
    }

    pub(crate) async fn call(
        self: &Arc<Self>,
        request: &ProviderStdioRequest,
    ) -> FrameworkResult<Value> {
        let lease = self.admit()?;
        let mut worker = self.worker.lock().await;
        self.ensure_lease_can_dispatch(&worker)?;
        let result = worker.call(request).await;
        if result.is_err() {
            self.fail_active_worker(&mut worker).await?;
        }
        drop(worker);
        drop(lease);
        result
    }

    pub(crate) async fn call_streaming_with_limits(
        self: &Arc<Self>,
        request: &ProviderStdioRequest,
        timeout_limits: &PluginRuntimeLimits,
        required_live_events: Option<tokio::sync::mpsc::Sender<ProviderStreamEvent>>,
        diagnostic_live_events: Option<tokio::sync::mpsc::Sender<ProviderStreamEvent>>,
        event_observer: Option<tokio::sync::mpsc::UnboundedSender<()>>,
    ) -> FrameworkResult<StreamingProviderOutput> {
        let lease = self.admit()?;
        let mut worker = self.worker.lock().await;
        self.ensure_lease_can_dispatch(&worker)?;
        let result = worker
            .call_streaming_with_limits(
                request,
                timeout_limits,
                required_live_events,
                diagnostic_live_events,
                event_observer,
            )
            .await;
        if result.is_err() {
            self.fail_active_worker(&mut worker).await?;
        }
        drop(worker);
        drop(lease);
        result
    }

    pub(crate) async fn finish_quiesce(
        &self,
        deadline: Duration,
        drained_reason: ProviderWorkerCleanupReason,
    ) -> FrameworkResult<ProviderWorkerCleanupReceipt> {
        let drained = tokio::time::timeout(deadline, self.wait_until_drained())
            .await
            .is_ok();
        let evidence = if drained {
            None
        } else {
            Some(self.process_control.terminate().await)
        };
        let generation = self.lock_admission()?.lifecycle.generation();
        let reason = if drained {
            drained_reason
        } else {
            ProviderWorkerCleanupReason::DeadlineExceeded
        };
        let mut worker = self.worker.lock().await;
        let prior_cleanup = self.last_cleanup_receipt()?;
        let receipt = if evidence.is_none() && worker.process_control().is_none() {
            match prior_cleanup {
                Some(mut receipt) => {
                    receipt.generation = generation;
                    receipt.reason = reason;
                    receipt.final_state = ProviderWorkerLifecycleState::Inactive;
                    receipt
                }
                None => {
                    worker
                        .stop_with_reason(
                            generation,
                            reason,
                            ProviderWorkerLifecycleState::Inactive,
                        )
                        .await
                }
            }
        } else {
            worker
                .stop_with_evidence(
                    generation,
                    reason,
                    ProviderWorkerLifecycleState::Inactive,
                    evidence,
                )
                .await
        };
        {
            let mut admission = self.lock_admission()?;
            admission
                .lifecycle
                .transition(ProviderWorkerLifecycleEvent::Quiesced)?;
        }
        *self
            .last_cleanup
            .lock()
            .map_err(|_| lifecycle_lock_error())? = Some(receipt.clone());
        Ok(receipt)
    }

    fn admit(self: &Arc<Self>) -> FrameworkResult<ProviderWorkerInvocationLease> {
        let mut admission = self.lock_admission()?;
        if admission.lifecycle.state() != ProviderWorkerLifecycleState::Active {
            return Err(PluginFrameworkError::invalid_provider_package(format!(
                "provider worker generation {} is not accepting calls in state {:?}",
                admission.lifecycle.generation(),
                admission.lifecycle.state()
            )));
        }
        admission.in_flight = admission.in_flight.saturating_add(1);
        Ok(ProviderWorkerInvocationLease {
            supervisor: Arc::clone(self),
        })
    }

    async fn wait_until_drained(&self) {
        loop {
            let notified = self.drained.notified();
            if self
                .admission
                .lock()
                .map(|admission| admission.in_flight == 0)
                .unwrap_or(true)
            {
                return;
            }
            notified.await;
        }
    }

    async fn fail_active_worker(&self, worker: &mut ProviderWorker) -> FrameworkResult<()> {
        let generation = {
            let mut admission = self.lock_admission()?;
            if admission.lifecycle.state() == ProviderWorkerLifecycleState::Active {
                admission
                    .lifecycle
                    .transition(ProviderWorkerLifecycleEvent::RuntimeFailed)?;
            }
            admission.lifecycle.generation()
        };
        let receipt = match worker.take_last_cleanup_receipt() {
            Some(mut receipt) => {
                receipt.generation = generation;
                receipt
            }
            None => {
                worker
                    .stop_with_reason(
                        generation,
                        ProviderWorkerCleanupReason::RuntimeFailure,
                        ProviderWorkerLifecycleState::Failed,
                    )
                    .await
            }
        };
        *self
            .last_cleanup
            .lock()
            .map_err(|_| lifecycle_lock_error())? = Some(receipt);
        Ok(())
    }

    fn ensure_lease_can_dispatch(&self, worker: &ProviderWorker) -> FrameworkResult<()> {
        let admission = self.lock_admission()?;
        let dispatchable = match admission.lifecycle.state() {
            ProviderWorkerLifecycleState::Active => true,
            ProviderWorkerLifecycleState::Quiescing => worker.process_control().is_some(),
            ProviderWorkerLifecycleState::Activating
            | ProviderWorkerLifecycleState::Inactive
            | ProviderWorkerLifecycleState::Failed => false,
        };
        if dispatchable {
            return Ok(());
        }
        Err(PluginFrameworkError::invalid_provider_package(format!(
            "provider worker generation {} cannot dispatch an admitted call in state {:?}",
            admission.lifecycle.generation(),
            admission.lifecycle.state()
        )))
    }

    fn lock_admission(&self) -> FrameworkResult<std::sync::MutexGuard<'_, AdmissionState>> {
        self.admission.lock().map_err(|_| lifecycle_lock_error())
    }
}

fn lifecycle_lock_error() -> PluginFrameworkError {
    PluginFrameworkError::invalid_provider_package("provider worker lifecycle is unavailable")
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use extension_package_runtime::provider_contract::ProviderStdioMethod;
    use serde_json::json;

    use super::*;

    #[test]
    fn lifecycle_accepts_only_declared_transitions() {
        let mut lifecycle = ProviderWorkerLifecycle::activating(7);
        lifecycle
            .transition(ProviderWorkerLifecycleEvent::Activated)
            .unwrap();
        lifecycle
            .transition(ProviderWorkerLifecycleEvent::BeginQuiesce)
            .unwrap();
        lifecycle
            .transition(ProviderWorkerLifecycleEvent::Quiesced)
            .unwrap();
        assert_eq!(lifecycle.state(), ProviderWorkerLifecycleState::Inactive);
        assert_eq!(lifecycle.generation(), 7);
        assert!(lifecycle
            .transition(ProviderWorkerLifecycleEvent::Quiesced)
            .unwrap_err()
            .to_string()
            .contains("illegal provider worker lifecycle transition"));
    }

    #[tokio::test]
    async fn quiescing_rejects_new_calls_and_drains_existing_lease() {
        let supervisor = supervisor(1);
        let running = Arc::clone(&supervisor);
        let call = tokio::spawn(async move { running.call(&request("slow")).await });
        wait_for_in_flight(&supervisor).await;

        supervisor.begin_quiesce().unwrap();
        let rejected = supervisor.call(&request("normal")).await.unwrap_err();
        assert!(rejected.to_string().contains("not accepting calls"));
        let receipt = supervisor
            .finish_quiesce(
                Duration::from_secs(1),
                ProviderWorkerCleanupReason::Restarted,
            )
            .await
            .unwrap();

        assert!(call.await.unwrap().is_ok());
        assert_eq!(receipt.reason, ProviderWorkerCleanupReason::Restarted);
        assert!(receipt.exited);
        assert_eq!(
            supervisor.snapshot().unwrap().state,
            ProviderWorkerLifecycleState::Inactive
        );
    }

    #[tokio::test]
    async fn quiesce_deadline_kills_child_and_records_prior_pid() {
        let supervisor = supervisor(1);
        let prior_pid = supervisor.snapshot().unwrap().pid;
        let running = Arc::clone(&supervisor);
        let call = tokio::spawn(async move { running.call(&request("slow")).await });
        wait_for_in_flight(&supervisor).await;

        supervisor.begin_quiesce().unwrap();
        let receipt = supervisor
            .finish_quiesce(
                Duration::from_millis(10),
                ProviderWorkerCleanupReason::Restarted,
            )
            .await
            .unwrap();

        assert!(call.await.unwrap().is_err());
        assert_eq!(receipt.prior_pid, prior_pid);
        assert_eq!(
            receipt.reason,
            ProviderWorkerCleanupReason::DeadlineExceeded
        );
        assert!(receipt.kill_sent);
        assert!(receipt.exited);
    }

    #[tokio::test]
    async fn child_crash_marks_generation_failed_with_cleanup_receipt() {
        let supervisor = supervisor(4);
        let pid = supervisor.snapshot().unwrap().pid;

        assert!(supervisor.call(&request("crash")).await.is_err());

        assert_eq!(
            supervisor.snapshot().unwrap().state,
            ProviderWorkerLifecycleState::Failed
        );
        let receipt = supervisor.last_cleanup_receipt().unwrap().unwrap();
        assert_eq!(receipt.generation, 4);
        assert_eq!(receipt.prior_pid, pid);
        assert_eq!(receipt.final_state, ProviderWorkerLifecycleState::Failed);
        assert!(receipt.exited);
    }

    #[tokio::test]
    async fn restart_uses_new_generation_and_pid_after_old_cleanup() {
        let old = supervisor(9);
        let old_pid = old.snapshot().unwrap().pid.unwrap();
        old.begin_quiesce().unwrap();
        let receipt = old
            .finish_quiesce(
                Duration::from_secs(1),
                ProviderWorkerCleanupReason::Restarted,
            )
            .await
            .unwrap();
        assert!(receipt.exited);
        assert!(!pid_is_alive(old_pid));

        let replacement = supervisor(10);
        let replacement_snapshot = replacement.snapshot().unwrap();
        assert_eq!(replacement_snapshot.generation, 10);
        assert_ne!(replacement_snapshot.pid, Some(old_pid));
        replacement.begin_quiesce().unwrap();
        replacement
            .finish_quiesce(Duration::from_secs(1), ProviderWorkerCleanupReason::Drained)
            .await
            .unwrap();
    }

    fn supervisor(generation: u64) -> Arc<ProviderWorkerSupervisor> {
        ProviderWorkerSupervisor::activate(fixture_script(), limits(), generation).unwrap()
    }

    fn fixture_script() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/_fixtures/provider_stdio/lifecycle_worker.sh")
    }

    fn limits() -> PluginRuntimeLimits {
        PluginRuntimeLimits {
            timeout_ms: Some(2_000),
            invoke_timeout_ms: None,
            first_token_timeout_ms: None,
            stream_idle_timeout_ms: None,
            memory_bytes: None,
        }
    }

    fn request(mode: &str) -> ProviderStdioRequest {
        ProviderStdioRequest {
            method: ProviderStdioMethod::Validate,
            input: json!({ "mode": mode }),
        }
    }

    async fn wait_for_in_flight(supervisor: &ProviderWorkerSupervisor) {
        for _ in 0..20 {
            if supervisor.snapshot().unwrap().in_flight == 1 {
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        panic!("provider worker call did not acquire an admission lease");
    }

    #[cfg(target_os = "linux")]
    fn pid_is_alive(pid: u32) -> bool {
        PathBuf::from(format!("/proc/{pid}")).exists()
    }

    #[cfg(not(target_os = "linux"))]
    fn pid_is_alive(_pid: u32) -> bool {
        false
    }
}
