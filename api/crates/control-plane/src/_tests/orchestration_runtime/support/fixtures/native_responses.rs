use super::*;

impl OrchestrationRuntimeService<InMemoryOrchestrationRuntimeRepository, InMemoryProviderRuntime> {
    pub fn native_recovery_page_reads_fixture(&self) -> usize {
        self.repository.native_recovery_page_reads()
    }

    pub fn alter_native_recovery_source_fixture(
        &self,
        run_id: Uuid,
        mode: &str,
        predecessor: Option<crate::ports::NativeContinuationRecoverySource>,
    ) {
        self.repository
            .alter_native_recovery_source(run_id, mode, predecessor)
    }

    pub fn for_tests_with_captured_provider_outputs(
        outputs: Vec<crate::ports::ProviderRuntimeInvocationOutput>,
    ) -> (
        Self,
        std::sync::Arc<std::sync::Mutex<Vec<ProviderInvocationInput>>>,
    ) {
        let repository = InMemoryOrchestrationRuntimeRepository::with_permissions(vec![
            "application.view.all",
            "application.create.all",
        ]);
        let (runtime, captured) =
            InMemoryProviderRuntime::with_provider_outputs(outputs).capturing_invocations();
        (Self::with_test_runtime(repository, runtime), captured)
    }

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

    pub fn gate_llm_selection_fixture(
        &self,
        fail: bool,
    ) -> (
        std::sync::Arc<tokio::sync::Notify>,
        std::sync::Arc<tokio::sync::Notify>,
    ) {
        self.repository.gate_llm_selection_append(fail)
    }

    pub async fn llm_selection_events_fixture(
        &self,
        run_id: Uuid,
    ) -> Vec<domain::RuntimeEventRecord> {
        self.repository
            .list_runtime_events(run_id, 0)
            .await
            .unwrap()
    }

    /// Claim the same durable client-tool batch consumed by HTTP SSE and Responses WS.
    pub async fn claim_native_tool_deliveries_fixture(
        &self,
        flow_run_id: Uuid,
    ) -> anyhow::Result<crate::ports::RuntimeEventDeliveryBatch> {
        self.repository
            .claim_runtime_event_deliveries(&crate::ports::ClaimRuntimeEventDeliveriesInput {
                flow_run_id,
                scope: crate::ports::RuntimeEventDeliveryScope::Start,
                include_membership: true,
                limit: 8,
                lease_seconds: 30,
            })
            .await
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
