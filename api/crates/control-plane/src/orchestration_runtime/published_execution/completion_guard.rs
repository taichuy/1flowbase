//! The admitted business root owns interruption cleanup, independently of subscribers.
use std::{future::Future, pin::Pin, sync::Arc};

use anyhow::Result;
use tokio::runtime::Handle;
use uuid::Uuid;

use super::super::stream_terminal_recovery::{
    finalize_published_run_missing_stream_terminal_owned,
    FinalizePublishedRunMissingStreamTerminalCommand,
};
use crate::ports::{OrchestrationRuntimeRepository, RuntimeEventStream};

type Completion = Pin<Box<dyn Future<Output = Result<domain::FlowRunRecord>> + Send>>;

pub(super) struct PublishedExecutionCompletionGuard {
    runtime: Handle,
    flow_run_id: Uuid,
    completion: Option<Completion>,
}

impl PublishedExecutionCompletionGuard {
    pub(super) fn new<R>(
        runtime: Handle,
        repository: R,
        stream: Option<Arc<dyn RuntimeEventStream>>,
        application_id: Uuid,
        flow_run_id: Uuid,
    ) -> Self
    where
        R: OrchestrationRuntimeRepository + Send + Sync + 'static,
    {
        Self {
            runtime,
            flow_run_id,
            completion: Some(Box::pin(async move {
                finalize_published_run_missing_stream_terminal_owned(
                    &repository,
                    stream.as_ref(),
                    FinalizePublishedRunMissingStreamTerminalCommand {
                        application_id,
                        flow_run_id,
                    },
                    // Checkpoint/claim handoff permanently revokes this initial root.
                    true,
                )
                .await
            })),
        }
    }

    pub(super) fn disarm(&mut self) {
        self.completion = None;
    }
}

impl Drop for PublishedExecutionCompletionGuard {
    fn drop(&mut self) {
        let Some(completion) = self.completion.take() else {
            return;
        };
        let flow_run_id = self.flow_run_id;
        // Capture the already installed runtime at admission, never infer a
        // cross-process recovery owner or turn observer closure into cancellation.
        self.runtime.spawn(async move {
            if let Err(error) = completion.await {
                tracing::warn!(%flow_run_id, %error, "interrupted published execution finalization failed");
            }
        });
    }
}

#[cfg(test)]
#[path = "../../_tests/orchestration_runtime/published_execution/completion.rs"]
mod tests;
