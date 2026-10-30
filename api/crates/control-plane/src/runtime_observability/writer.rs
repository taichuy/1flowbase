//! The existing observation functions need these writes, not the entire
//! orchestration repository. Ordinary repositories delegate without changing
//! behavior; scoped recovery supplies stable write identities.
use crate::ports::*;
use async_trait::async_trait;

#[async_trait]
pub trait RuntimeObservationRepository: Send + Sync {
    async fn append_runtime_span(
        &self,
        input: &AppendRuntimeSpanInput,
    ) -> anyhow::Result<domain::RuntimeSpanRecord>;
    async fn append_context_projection(
        &self,
        input: &AppendContextProjectionInput,
    ) -> anyhow::Result<domain::ContextProjectionRecord>;
    async fn append_model_failover_attempt_ledger(
        &self,
        input: &AppendModelFailoverAttemptLedgerInput,
    ) -> anyhow::Result<domain::ModelFailoverAttemptLedgerRecord>;
    async fn append_usage_ledger(
        &self,
        input: &AppendUsageLedgerInput,
    ) -> anyhow::Result<domain::UsageLedgerRecord>;
    async fn append_run_event(
        &self,
        input: &AppendRunEventInput,
    ) -> anyhow::Result<domain::RunEventRecord>;
    async fn append_runtime_event(
        &self,
        input: &AppendRuntimeEventInput,
    ) -> anyhow::Result<domain::RuntimeEventRecord>;
    async fn append_runtime_item(
        &self,
        input: &AppendRuntimeItemInput,
    ) -> anyhow::Result<domain::RuntimeItemRecord>;
    async fn append_capability_invocation(
        &self,
        input: &AppendCapabilityInvocationInput,
    ) -> anyhow::Result<domain::CapabilityInvocationRecord>;
    async fn append_provider_invocation_context(
        &self,
        input: &AppendProviderInvocationContextInput,
    ) -> anyhow::Result<domain::ContextVersionRecord>;
    async fn link_usage_ledger_to_model_failover_attempt(
        &self,
        input: &LinkUsageLedgerToModelFailoverAttemptInput,
    ) -> anyhow::Result<domain::ModelFailoverAttemptLedgerRecord>;
    async fn append_run_events(
        &self,
        inputs: &[AppendRunEventInput],
    ) -> anyhow::Result<Vec<domain::RunEventRecord>> {
        let mut records = Vec::with_capacity(inputs.len());
        for input in inputs {
            records.push(self.append_run_event(input).await?);
        }
        Ok(records)
    }
    async fn append_runtime_events(
        &self,
        inputs: &[AppendRuntimeEventInput],
    ) -> anyhow::Result<Vec<domain::RuntimeEventRecord>> {
        let mut records = Vec::with_capacity(inputs.len());
        for input in inputs {
            records.push(self.append_runtime_event(input).await?);
        }
        Ok(records)
    }
}
#[async_trait]
impl<R: OrchestrationRuntimeRepository + ?Sized> RuntimeObservationRepository for R {
    async fn append_runtime_span(
        &self,
        input: &AppendRuntimeSpanInput,
    ) -> anyhow::Result<domain::RuntimeSpanRecord> {
        OrchestrationRuntimeRepository::append_runtime_span(self, input).await
    }
    async fn append_context_projection(
        &self,
        input: &AppendContextProjectionInput,
    ) -> anyhow::Result<domain::ContextProjectionRecord> {
        OrchestrationRuntimeRepository::append_context_projection(self, input).await
    }
    async fn append_model_failover_attempt_ledger(
        &self,
        input: &AppendModelFailoverAttemptLedgerInput,
    ) -> anyhow::Result<domain::ModelFailoverAttemptLedgerRecord> {
        OrchestrationRuntimeRepository::append_model_failover_attempt_ledger(self, input).await
    }
    async fn append_usage_ledger(
        &self,
        input: &AppendUsageLedgerInput,
    ) -> anyhow::Result<domain::UsageLedgerRecord> {
        OrchestrationRuntimeRepository::append_usage_ledger(self, input).await
    }
    async fn append_run_event(
        &self,
        input: &AppendRunEventInput,
    ) -> anyhow::Result<domain::RunEventRecord> {
        OrchestrationRuntimeRepository::append_run_event(self, input).await
    }
    async fn append_runtime_event(
        &self,
        input: &AppendRuntimeEventInput,
    ) -> anyhow::Result<domain::RuntimeEventRecord> {
        OrchestrationRuntimeRepository::append_runtime_event(self, input).await
    }
    async fn append_runtime_item(
        &self,
        input: &AppendRuntimeItemInput,
    ) -> anyhow::Result<domain::RuntimeItemRecord> {
        OrchestrationRuntimeRepository::append_runtime_item(self, input).await
    }
    async fn append_capability_invocation(
        &self,
        input: &AppendCapabilityInvocationInput,
    ) -> anyhow::Result<domain::CapabilityInvocationRecord> {
        OrchestrationRuntimeRepository::append_capability_invocation(self, input).await
    }
    async fn append_provider_invocation_context(
        &self,
        input: &AppendProviderInvocationContextInput,
    ) -> anyhow::Result<domain::ContextVersionRecord> {
        OrchestrationRuntimeRepository::append_provider_invocation_context(self, input).await
    }
    async fn link_usage_ledger_to_model_failover_attempt(
        &self,
        input: &LinkUsageLedgerToModelFailoverAttemptInput,
    ) -> anyhow::Result<domain::ModelFailoverAttemptLedgerRecord> {
        OrchestrationRuntimeRepository::link_usage_ledger_to_model_failover_attempt(self, input)
            .await
    }
    async fn append_run_events(
        &self,
        inputs: &[AppendRunEventInput],
    ) -> anyhow::Result<Vec<domain::RunEventRecord>> {
        OrchestrationRuntimeRepository::append_run_events(self, inputs).await
    }
    async fn append_runtime_events(
        &self,
        inputs: &[AppendRuntimeEventInput],
    ) -> anyhow::Result<Vec<domain::RuntimeEventRecord>> {
        OrchestrationRuntimeRepository::append_runtime_events(self, inputs).await
    }
}
