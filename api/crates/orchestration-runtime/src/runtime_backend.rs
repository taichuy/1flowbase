use std::sync::Arc;

use runtime_core::runtime_backend::{
    RuntimeBackendError, RuntimeCancelOutcome, RuntimeExecutionOutcome, RuntimeExecutionPort,
    RuntimeExecutionRequest, RuntimeRequestId, RuntimeStreamSinks,
};

/// Orchestration-owned Runtime Backend seam.
///
/// Provider routing selects the target before this executor is called. The executor knows only
/// the stable Runtime Backend Port, so replacing the composition binding does not change the
/// orchestration or API business path.
#[derive(Clone)]
pub struct OrchestrationRuntimeBackend {
    execution: Arc<dyn RuntimeExecutionPort>,
}

impl OrchestrationRuntimeBackend {
    pub fn new(execution: Arc<dyn RuntimeExecutionPort>) -> Self {
        Self { execution }
    }

    pub async fn execute(
        &self,
        request: RuntimeExecutionRequest,
    ) -> Result<RuntimeExecutionOutcome, RuntimeBackendError> {
        self.execution.execute(request).await
    }

    pub async fn execute_stream(
        &self,
        request: RuntimeExecutionRequest,
        sinks: RuntimeStreamSinks,
    ) -> Result<RuntimeExecutionOutcome, RuntimeBackendError> {
        self.execution.execute_stream(request, sinks).await
    }

    pub async fn cancel(
        &self,
        request_id: &RuntimeRequestId,
    ) -> Result<RuntimeCancelOutcome, RuntimeBackendError> {
        self.execution.cancel(request_id).await
    }
}
