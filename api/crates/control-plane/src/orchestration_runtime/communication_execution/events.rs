//! The admitted communication producer owns one runtime-stream generation and its persister.
use super::super::{runtime_event_persister, OrchestrationRuntimeService};
use crate::ports::{
    OrchestrationRuntimeRepository, RuntimeEventCloseReason, RuntimeEventStreamPolicy,
    RuntimeEventTerminalWriter,
};
use anyhow::Result;
use std::sync::Arc;
use tokio::task::JoinHandle;
use uuid::Uuid;

pub(in crate::orchestration_runtime) struct CommunicationRuntimeEvents {
    writer: Arc<dyn RuntimeEventTerminalWriter>,
    persister: Option<JoinHandle<()>>,
    run_id: Uuid,
}

impl<R, H> OrchestrationRuntimeService<R, H>
where
    R: OrchestrationRuntimeRepository + Clone + Send + Sync + 'static,
{
    /// Call only after the initial dispatch or resume claim has selected its execution owner.
    pub(in crate::orchestration_runtime) async fn begin_communication_events(
        &self,
        run: &domain::FlowRunRecord,
    ) -> Result<Option<CommunicationRuntimeEvents>> {
        if run.compatibility_mode.as_deref() != Some("communication") {
            return Ok(None);
        }
        let Some(stream) = &self.runtime_event_stream else {
            return Ok(None);
        };
        stream
            .open_run(run.id, RuntimeEventStreamPolicy::debug_default())
            .await?;
        let mut writer = stream.terminal_writer(run.id).await?;
        if writer.durable_replay_boundary().is_some() || writer.closure().is_some() {
            // The waiting checkpoint is committed before its presentation stream closes.
            // An immediate resume must let that producer finish, then reopen; sharing its
            // still-open generation would let the old producer close the resumed stream.
            // Only observe closure: do not copy the prior generation's replay payloads.
            let mut closure = stream.subscribe(run.id, Some(i64::MAX)).await?.closure;
            tokio::time::timeout(
                runtime_event_persister::RUNTIME_EVENT_PERSISTER_JOIN_TIMEOUT,
                closure.wait_for(Option::is_some),
            )
            .await
            .map_err(|_| anyhow::anyhow!("communication_previous_event_generation_still_open"))??;
            stream
                .open_run(run.id, RuntimeEventStreamPolicy::debug_default())
                .await?;
            writer = stream.terminal_writer(run.id).await?;
        }
        let persister = runtime_event_persister::start_runtime_debug_event_persister(
            self.repository.clone(),
            stream.clone(),
            run.id,
        )
        .await;
        let owner = CommunicationRuntimeEvents {
            writer,
            persister: Some(persister),
            run_id: run.id,
        };
        // A generation with no committed anchor is not ready to accept a producer.
        if owner.writer.generation_id().is_some()
            && owner.writer.durable_replay_boundary().is_none()
        {
            owner.finish().await;
            anyhow::bail!("communication_runtime_event_anchor_unavailable");
        }
        Ok(Some(owner))
    }
}

impl CommunicationRuntimeEvents {
    pub(in crate::orchestration_runtime) async fn finish(mut self) {
        if let Some(task) = self.persister.take() {
            finish_generation(self.writer.clone(), task, self.run_id).await;
        }
    }
}
impl Drop for CommunicationRuntimeEvents {
    fn drop(&mut self) {
        let Some(task) = self.persister.take() else {
            return;
        };
        let writer = self.writer.clone();
        let run_id = self.run_id;
        // Cancellation must close the captured generation, never a replacement reopened
        // by a fresh human/callback resume. This owner performs no business execution.
        match tokio::runtime::Handle::try_current() {
            Ok(runtime) => {
                runtime.spawn(finish_generation(writer, task, run_id));
            }
            Err(_) => {
                writer.record_persistence_failure();
                task.abort();
            }
        }
    }
}

async fn finish_generation(
    writer: Arc<dyn RuntimeEventTerminalWriter>,
    mut task: JoinHandle<()>,
    run_id: Uuid,
) {
    if writer.closure().is_none() {
        // Normal terminal/wait persistence already closes with its precise reason. An
        // abandoned producer closes without manufacturing a successful/cancelled terminal.
        if let Err(error) = writer
            .close_generation(RuntimeEventCloseReason::Expired)
            .await
        {
            writer.record_persistence_failure();
            tracing::warn!(%run_id, %error, "communication runtime generation could not close");
        }
    }
    match tokio::time::timeout(
        runtime_event_persister::RUNTIME_EVENT_PERSISTER_JOIN_TIMEOUT,
        &mut task,
    )
    .await
    {
        Ok(Ok(())) => {}
        Ok(Err(error)) => {
            writer.record_persistence_failure();
            tracing::warn!(%run_id, %error, "communication runtime persister failed");
        }
        Err(_) => {
            writer.record_persistence_failure();
            task.abort();
            let _ = task.await;
            tracing::warn!(%run_id, "communication runtime persister join timed out");
        }
    }
}
