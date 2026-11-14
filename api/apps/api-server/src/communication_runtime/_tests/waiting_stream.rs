//! Controls only the interval between a real waiting commit and stream closure.
use anyhow::Result;
use control_plane::ports::{
    AppendTerminalIfMissingAndCloseOutcome, RuntimeEventCloseReason, RuntimeEventEnvelope,
    RuntimeEventPayload, RuntimeEventStream, RuntimeEventStreamPolicy, RuntimeEventSubscription,
    RuntimeEventTerminalWriter, RuntimeEventTrimPolicy,
};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tokio::sync::Notify;
use uuid::Uuid;

#[derive(Default)]
pub(super) struct WaitingStream {
    inner: crate::host_infrastructure::LocalRuntimeEventStream,
    paused: AtomicBool,
    pause_taken: AtomicBool,
    pub waiting_entered: Notify,
    pub resume_entered: Notify,
    pub release_waiting: Notify,
}

#[async_trait::async_trait]
impl RuntimeEventStream for WaitingStream {
    async fn terminal_writer(&self, run: Uuid) -> Result<Arc<dyn RuntimeEventTerminalWriter>> {
        self.inner.terminal_writer(run).await
    }

    async fn open_run(&self, run: Uuid, policy: RuntimeEventStreamPolicy) -> Result<()> {
        let result = self.inner.open_run(run, policy).await;
        if self.paused.load(Ordering::SeqCst) {
            self.resume_entered.notify_one();
        }
        result
    }

    async fn append(&self, run: Uuid, event: RuntimeEventPayload) -> Result<RuntimeEventEnvelope> {
        self.inner.append(run, event).await
    }

    async fn append_terminal_if_missing_and_close(
        &self,
        run: Uuid,
        event: RuntimeEventPayload,
    ) -> Result<AppendTerminalIfMissingAndCloseOutcome> {
        self.inner
            .append_terminal_if_missing_and_close(run, event)
            .await
    }

    async fn subscribe(&self, run: Uuid, from: Option<i64>) -> Result<RuntimeEventSubscription> {
        self.inner.subscribe(run, from).await
    }

    async fn replay(
        &self,
        run: Uuid,
        from: Option<i64>,
        limit: usize,
    ) -> Result<Vec<RuntimeEventEnvelope>> {
        self.inner.replay(run, from, limit).await
    }

    async fn close_run(&self, run: Uuid, reason: RuntimeEventCloseReason) -> Result<()> {
        if matches!(
            reason,
            RuntimeEventCloseReason::WaitingHuman | RuntimeEventCloseReason::WaitingCallback
        ) && !self.pause_taken.swap(true, Ordering::SeqCst)
        {
            self.paused.store(true, Ordering::SeqCst);
            self.waiting_entered.notify_one();
            self.release_waiting.notified().await;
            self.paused.store(false, Ordering::SeqCst);
        }
        self.inner.close_run(run, reason).await
    }

    async fn trim(&self, run: Uuid, policy: RuntimeEventTrimPolicy) -> Result<()> {
        self.inner.trim(run, policy).await
    }
}
