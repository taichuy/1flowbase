use super::*;

impl OrchestrationRuntimeService<InMemoryOrchestrationRuntimeRepository, InMemoryProviderRuntime> {
    /// Use the real service/invoker assembly while keeping the upstream deterministically scripted.
    pub async fn enable_native_responses_fixture(&self) {
        self.repository.enable_native_responses_fixture().await;
    }

    pub async fn continue_native_responses_fixture(
        &self,
        command: ContinueFlowDebugRunCommand,
        payload: crate::ports::ProviderTransportPayload,
    ) -> anyhow::Result<domain::ApplicationRunDetail> {
        crate::orchestration_runtime::live_debug_run::continue_flow_debug_run_with_provider_transport(
            self, command, Some(payload), None, None,
        ).await
    }

    pub async fn native_continuation_fixture(
        &self,
        flow_run_id: Uuid,
    ) -> crate::ports::ProviderContinuation {
        self.optional_native_continuation_fixture(flow_run_id)
            .await
            .expect("successful native response should publish its continuation")
    }

    pub async fn optional_native_continuation_fixture(
        &self,
        flow_run_id: Uuid,
    ) -> Option<crate::ports::ProviderContinuation> {
        self.provider_transport_store
            .get_continuation(crate::ports::ProviderContinuationSlotId::for_flow_run(
                flow_run_id,
            ))
            .await
            .unwrap()
    }
}
