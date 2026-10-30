use crate::ports::*;
use crate::runtime_observability::RuntimeObservationRepository;
use async_trait::async_trait;
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, sync::Mutex};
use uuid::Uuid;

pub(super) struct ScopedObservationWriter<'a, R> {
    pub repository: &'a R,
    pub observation_id: Uuid,
    counters: Mutex<BTreeMap<&'static str, u64>>,
}
impl<'a, R> ScopedObservationWriter<'a, R> {
    pub fn new(repository: &'a R, observation_id: Uuid) -> Self {
        Self {
            repository,
            observation_id,
            counters: Mutex::new(BTreeMap::new()),
        }
    }
    fn record_id(&self, kind: &'static str, key: Option<String>) -> anyhow::Result<Uuid> {
        let key = match key {
            Some(key) => key,
            None => {
                let mut counters = self
                    .counters
                    .lock()
                    .map_err(|_| anyhow::anyhow!("observation counter poisoned"))?;
                let counter = counters.entry(kind).or_default();
                let key = counter.to_string();
                *counter += 1;
                key
            }
        };
        let mut hash = Sha256::new();
        hash.update(self.observation_id.as_bytes());
        hash.update(kind);
        hash.update([0]);
        hash.update(key);
        let digest = hash.finalize();
        let mut bytes = [0; 16];
        bytes.copy_from_slice(&digest[..16]);
        bytes[6] = (bytes[6] & 15) | 128;
        bytes[8] = (bytes[8] & 63) | 128;
        Ok(Uuid::from_bytes(bytes))
    }
}
#[async_trait]
impl<R: OrchestrationRuntimeRepository> RuntimeObservationRepository
    for ScopedObservationWriter<'_, R>
{
    async fn append_runtime_span(
        &self,
        input: &AppendRuntimeSpanInput,
    ) -> anyhow::Result<domain::RuntimeSpanRecord> {
        let id = self.record_id("Span", None)?;
        match self
            .repository
            .apply_container_observation(
                self.observation_id,
                id,
                &ContainerObservationOperation::Span(input.clone()),
            )
            .await?
        {
            ContainerObservationResult::Span(record) => Ok(record),
            _ => anyhow::bail!("container observation result kind mismatch"),
        }
    }
    async fn append_context_projection(
        &self,
        input: &AppendContextProjectionInput,
    ) -> anyhow::Result<domain::ContextProjectionRecord> {
        let id = self.record_id("ContextProjection", None)?;
        match self
            .repository
            .apply_container_observation(
                self.observation_id,
                id,
                &ContainerObservationOperation::ContextProjection(input.clone()),
            )
            .await?
        {
            ContainerObservationResult::ContextProjection(record) => Ok(record),
            _ => anyhow::bail!("container observation result kind mismatch"),
        }
    }
    async fn append_model_failover_attempt_ledger(
        &self,
        input: &AppendModelFailoverAttemptLedgerInput,
    ) -> anyhow::Result<domain::ModelFailoverAttemptLedgerRecord> {
        let id = self.record_id("Attempt", None)?;
        match self
            .repository
            .apply_container_observation(
                self.observation_id,
                id,
                &ContainerObservationOperation::Attempt(input.clone()),
            )
            .await?
        {
            ContainerObservationResult::Attempt(record) => Ok(record),
            _ => anyhow::bail!("container observation result kind mismatch"),
        }
    }
    async fn append_usage_ledger(
        &self,
        input: &AppendUsageLedgerInput,
    ) -> anyhow::Result<domain::UsageLedgerRecord> {
        let id = self.record_id("Usage", Some(format!("{:?}", input.failover_attempt_id)))?;
        match self
            .repository
            .apply_container_observation(
                self.observation_id,
                id,
                &ContainerObservationOperation::Usage(input.clone()),
            )
            .await?
        {
            ContainerObservationResult::Usage(record) => Ok(record),
            _ => anyhow::bail!("container observation result kind mismatch"),
        }
    }
    async fn append_run_event(
        &self,
        input: &AppendRunEventInput,
    ) -> anyhow::Result<domain::RunEventRecord> {
        let id = self.record_id("RunEvent", None)?;
        match self
            .repository
            .apply_container_observation(
                self.observation_id,
                id,
                &ContainerObservationOperation::RunEvent(input.clone()),
            )
            .await?
        {
            ContainerObservationResult::RunEvent(record) => Ok(record),
            _ => anyhow::bail!("container observation result kind mismatch"),
        }
    }
    async fn append_runtime_event(
        &self,
        input: &AppendRuntimeEventInput,
    ) -> anyhow::Result<domain::RuntimeEventRecord> {
        let id = self.record_id("RuntimeEvent", None)?;
        match self
            .repository
            .apply_container_observation(
                self.observation_id,
                id,
                &ContainerObservationOperation::RuntimeEvent(input.clone()),
            )
            .await?
        {
            ContainerObservationResult::RuntimeEvent(record) => Ok(record),
            _ => anyhow::bail!("container observation result kind mismatch"),
        }
    }
    async fn append_runtime_item(
        &self,
        input: &AppendRuntimeItemInput,
    ) -> anyhow::Result<domain::RuntimeItemRecord> {
        let id = self.record_id("Item", None)?;
        match self
            .repository
            .apply_container_observation(
                self.observation_id,
                id,
                &ContainerObservationOperation::Item(input.clone()),
            )
            .await?
        {
            ContainerObservationResult::Item(record) => Ok(record),
            _ => anyhow::bail!("container observation result kind mismatch"),
        }
    }
    async fn append_capability_invocation(
        &self,
        input: &AppendCapabilityInvocationInput,
    ) -> anyhow::Result<domain::CapabilityInvocationRecord> {
        let id = self.record_id("Capability", None)?;
        match self
            .repository
            .apply_container_observation(
                self.observation_id,
                id,
                &ContainerObservationOperation::Capability(input.clone()),
            )
            .await?
        {
            ContainerObservationResult::Capability(record) => Ok(record),
            _ => anyhow::bail!("container observation result kind mismatch"),
        }
    }
    async fn append_provider_invocation_context(
        &self,
        input: &AppendProviderInvocationContextInput,
    ) -> anyhow::Result<domain::ContextVersionRecord> {
        self.repository
            .append_provider_invocation_context(input)
            .await
    }
    async fn link_usage_ledger_to_model_failover_attempt(
        &self,
        input: &LinkUsageLedgerToModelFailoverAttemptInput,
    ) -> anyhow::Result<domain::ModelFailoverAttemptLedgerRecord> {
        self.repository
            .link_usage_ledger_to_model_failover_attempt(input)
            .await
    }
}

#[cfg(test)]
#[path = "_tests/scoped_observation_writer.rs"]
mod tests;
