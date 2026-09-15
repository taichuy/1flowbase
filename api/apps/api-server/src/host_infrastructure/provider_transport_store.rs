use std::sync::Arc;

use async_trait::async_trait;
use control_plane::ports::{
    ProviderContinuation, ProviderContinuationSlotId, ProviderProtocolCapsuleStore,
    ProviderProtocolContextSlotId, ProviderProtocolContextValue, ProviderTransportPayload,
    ProviderTransportSlotId, ProviderTransportStore,
};

#[derive(Clone)]
pub struct LayeredProviderTransportStore {
    transient: Arc<dyn ProviderTransportStore>,
    capsules: Arc<dyn ProviderProtocolCapsuleStore>,
}

impl LayeredProviderTransportStore {
    pub fn new(
        transient: Arc<dyn ProviderTransportStore>,
        capsules: Arc<dyn ProviderProtocolCapsuleStore>,
    ) -> Self {
        Self {
            transient,
            capsules,
        }
    }
}

#[async_trait]
impl ProviderTransportStore for LayeredProviderTransportStore {
    async fn put(
        &self,
        slot_id: ProviderTransportSlotId,
        payload: ProviderTransportPayload,
    ) -> anyhow::Result<()> {
        self.transient.put(slot_id, payload).await
    }

    async fn get(
        &self,
        slot_id: ProviderTransportSlotId,
    ) -> anyhow::Result<Option<ProviderTransportPayload>> {
        self.transient.get(slot_id).await
    }

    async fn consume(
        &self,
        slot_id: ProviderTransportSlotId,
    ) -> anyhow::Result<ProviderTransportPayload> {
        self.transient.consume(slot_id).await
    }

    async fn delete(&self, slot_id: ProviderTransportSlotId) -> anyhow::Result<bool> {
        self.transient.delete(slot_id).await
    }

    async fn put_protocol_context(
        &self,
        slot_id: ProviderProtocolContextSlotId,
        value: ProviderProtocolContextValue,
    ) -> anyhow::Result<()> {
        self.capsules.put_protocol_context(slot_id, value).await
    }

    async fn get_protocol_context(
        &self,
        slot_id: ProviderProtocolContextSlotId,
    ) -> anyhow::Result<Option<ProviderProtocolContextValue>> {
        self.capsules.get_protocol_context(slot_id).await
    }

    async fn delete_flow_run_protocol_contexts(
        &self,
        flow_run_id: uuid::Uuid,
    ) -> anyhow::Result<usize> {
        self.capsules
            .delete_flow_run_protocol_contexts(flow_run_id)
            .await
    }

    async fn put_continuation(
        &self,
        slot_id: ProviderContinuationSlotId,
        continuation: ProviderContinuation,
    ) -> anyhow::Result<()> {
        self.capsules.put_continuation(slot_id, continuation).await
    }

    async fn get_continuation(
        &self,
        slot_id: ProviderContinuationSlotId,
    ) -> anyhow::Result<Option<ProviderContinuation>> {
        self.capsules.get_continuation(slot_id).await
    }

    async fn consume_continuation(
        &self,
        slot_id: ProviderContinuationSlotId,
    ) -> anyhow::Result<ProviderContinuation> {
        self.capsules.consume_continuation(slot_id).await
    }

    async fn delete_continuation(
        &self,
        slot_id: ProviderContinuationSlotId,
    ) -> anyhow::Result<bool> {
        self.capsules.delete_continuation(slot_id).await
    }

    async fn clear_expired(&self) -> anyhow::Result<usize> {
        let transient = self.transient.clear_expired().await?;
        let capsules = self.capsules.clear_expired().await?;
        Ok(transient.saturating_add(capsules))
    }
}
