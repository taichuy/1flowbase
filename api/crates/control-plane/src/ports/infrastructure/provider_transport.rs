use std::fmt;

use async_trait::async_trait;
use plugin_framework::provider_contract::ProtocolContextEnvelope;
use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use uuid::Uuid;

const EPHEMERAL_PROTOCOL_CONTEXT_LOCATOR_KEY: &str = "__1flowbase_ephemeral_protocol_context";

/// Opaque runtime-only handle for an AI Native operation's ephemeral provider payload.
///
/// The handle intentionally has no serde or `Debug` representation: it may cross the
/// route-to-runtime call boundary, but must not enter Native input, durable state, or logs.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct ProviderTransportSlotId(Uuid);

impl ProviderTransportSlotId {
    pub const fn for_flow_run(flow_run_id: Uuid) -> Self {
        Self(flow_run_id)
    }

    pub(crate) const fn as_uuid(self) -> Uuid {
        self.0
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct ProviderContinuationSlotId(Uuid);

impl ProviderContinuationSlotId {
    pub const fn for_flow_run(flow_run_id: Uuid) -> Self {
        Self(flow_run_id)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum ProviderProtocolContextSlot {
    Original,
    Derived(Uuid),
}

/// Flow-owned key for one raw protocol-context value in ephemeral storage.
///
/// The flow owner has no `Debug` or serde representation. The durable representation is
/// [`ProviderProtocolContextLocator`], which contains only a locator, digest, byte count, and safe
/// source projection.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct ProviderProtocolContextSlotId {
    flow_run_id: Uuid,
    slot: ProviderProtocolContextSlot,
}

impl ProviderProtocolContextSlotId {
    pub const fn for_original_flow_run(flow_run_id: Uuid) -> Self {
        Self {
            flow_run_id,
            slot: ProviderProtocolContextSlot::Original,
        }
    }

    pub fn for_locator(flow_run_id: Uuid, locator: &ProviderProtocolContextLocator) -> Self {
        Self {
            flow_run_id,
            slot: locator.slot,
        }
    }

    pub fn belongs_to(self, flow_run_id: Uuid) -> bool {
        self.flow_run_id == flow_run_id
    }

    pub const fn flow_run_id(self) -> Uuid {
        self.flow_run_id
    }
}

/// Raw protocol-context value sealed for storage. Its `Debug` output intentionally exposes only
/// the durable-safe descriptor.
#[derive(Clone, PartialEq)]
pub struct ProviderProtocolContextValue {
    value: Value,
    digest: String,
    size_bytes: usize,
    source_protocol: Option<String>,
}

impl ProviderProtocolContextValue {
    pub fn new(value: Value) -> anyhow::Result<Self> {
        let encoded = serde_json::to_vec(&value)?;
        let mut canonical = Vec::new();
        write_canonical_json(&value, &mut canonical)?;
        let source_protocol = value
            .get("source_protocol")
            .and_then(Value::as_str)
            .filter(|source| {
                !source.is_empty()
                    && source.len() <= 64
                    && source.bytes().all(|byte| {
                        byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.')
                    })
            })
            .map(str::to_string);
        Ok(Self {
            value,
            digest: format!("sha256:{:x}", Sha256::digest(&canonical)),
            size_bytes: encoded.len(),
            source_protocol,
        })
    }

    pub fn from_envelope(envelope: ProtocolContextEnvelope) -> anyhow::Result<Self> {
        Self::new(serde_json::to_value(envelope)?)
    }

    pub fn original_locator(&self) -> ProviderProtocolContextLocator {
        ProviderProtocolContextLocator::new(ProviderProtocolContextSlot::Original, self)
    }

    pub fn derived_locator(&self) -> ProviderProtocolContextLocator {
        ProviderProtocolContextLocator::new(
            ProviderProtocolContextSlot::Derived(Uuid::now_v7()),
            self,
        )
    }

    pub fn into_value(self) -> Value {
        self.value
    }

    pub const fn size_bytes(&self) -> usize {
        self.size_bytes
    }

    pub fn matches_locator(&self, locator: &ProviderProtocolContextLocator) -> bool {
        self.digest == locator.digest
            && self.size_bytes == locator.size_bytes
            && self.source_protocol == locator.source_protocol
    }
}

impl fmt::Debug for ProviderProtocolContextValue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProviderProtocolContextValue")
            .field("digest", &self.digest)
            .field("size_bytes", &self.size_bytes)
            .field("source_protocol", &self.source_protocol)
            .finish_non_exhaustive()
    }
}

/// Durable-safe pointer to one protocol-context value owned by a flow run.
#[derive(Clone, PartialEq, Eq)]
pub struct ProviderProtocolContextLocator {
    slot: ProviderProtocolContextSlot,
    digest: String,
    size_bytes: usize,
    source_protocol: Option<String>,
}

impl fmt::Debug for ProviderProtocolContextLocator {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let slot = match self.slot {
            ProviderProtocolContextSlot::Original => "original".to_string(),
            ProviderProtocolContextSlot::Derived(slot_id) => slot_id.to_string(),
        };
        formatter
            .debug_struct("ProviderProtocolContextLocator")
            .field("slot", &slot)
            .field("digest", &self.digest)
            .field("size_bytes", &self.size_bytes)
            .field("source_protocol", &self.source_protocol)
            .finish()
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ProviderProtocolContextLocatorPayload {
    storage: String,
    slot: String,
    digest: String,
    size_bytes: usize,
    #[serde(default)]
    source_protocol: Option<String>,
}

impl ProviderProtocolContextLocator {
    fn new(slot: ProviderProtocolContextSlot, value: &ProviderProtocolContextValue) -> Self {
        Self {
            slot,
            digest: value.digest.clone(),
            size_bytes: value.size_bytes,
            source_protocol: value.source_protocol.clone(),
        }
    }

    pub fn parse(value: &Value) -> anyhow::Result<Option<Self>> {
        let Some(object) = value.as_object() else {
            return Ok(None);
        };
        let Some(payload) = object.get(EPHEMERAL_PROTOCOL_CONTEXT_LOCATOR_KEY) else {
            return Ok(None);
        };
        anyhow::ensure!(
            object.len() == 1,
            "ephemeral_protocol_context_locator_invalid"
        );
        let payload: ProviderProtocolContextLocatorPayload =
            serde_json::from_value(payload.clone())
                .map_err(|_| anyhow::anyhow!("ephemeral_protocol_context_locator_invalid"))?;
        anyhow::ensure!(
            payload.storage == "ephemeral"
                && payload.digest.starts_with("sha256:")
                && payload.digest.len() == 71
                && payload.size_bytes > 0
                && payload.source_protocol.as_ref().is_none_or(|source| {
                    !source.is_empty()
                        && source.len() <= 64
                        && source.bytes().all(|byte| {
                            byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.')
                        })
                }),
            "ephemeral_protocol_context_locator_invalid"
        );
        let slot = if payload.slot == "original" {
            ProviderProtocolContextSlot::Original
        } else {
            ProviderProtocolContextSlot::Derived(
                Uuid::parse_str(&payload.slot)
                    .map_err(|_| anyhow::anyhow!("ephemeral_protocol_context_locator_invalid"))?,
            )
        };
        Ok(Some(Self {
            slot,
            digest: payload.digest,
            size_bytes: payload.size_bytes,
            source_protocol: payload.source_protocol,
        }))
    }

    pub fn as_value(&self) -> Value {
        let slot = match self.slot {
            ProviderProtocolContextSlot::Original => "original".to_string(),
            ProviderProtocolContextSlot::Derived(slot_id) => slot_id.to_string(),
        };
        serde_json::json!({
            (EPHEMERAL_PROTOCOL_CONTEXT_LOCATOR_KEY): {
                "storage": "ephemeral",
                "slot": slot,
                "digest": self.digest,
                "size_bytes": self.size_bytes,
                "source_protocol": self.source_protocol,
            }
        })
    }

    pub fn digest(&self) -> &str {
        &self.digest
    }

    pub const fn size_bytes(&self) -> usize {
        self.size_bytes
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderTransportProtocol {
    OpenAiResponses,
}

/// Runtime-only identity of the Provider route that owns opaque continuation state.
///
/// This value is derived from an actual LLM invocation. It is deliberately not serializable
/// because workflow variables and durable request bodies must not become a second routing owner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderTransportAffinity {
    provider_instance_id: String,
    provider_code: String,
    protocol: String,
    model: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderContinuation {
    response_id: String,
    affinity: ProviderTransportAffinity,
}

impl ProviderContinuation {
    pub fn new(
        response_id: impl Into<String>,
        affinity: ProviderTransportAffinity,
    ) -> anyhow::Result<Self> {
        let response_id = response_id.into();
        anyhow::ensure!(
            !response_id.trim().is_empty() && response_id.len() <= 4096,
            "provider_continuation_id_invalid"
        );
        Ok(Self {
            response_id,
            affinity,
        })
    }

    pub(crate) fn response_id(&self) -> &str {
        &self.response_id
    }

    pub(crate) fn affinity(&self) -> &ProviderTransportAffinity {
        &self.affinity
    }
}

impl ProviderTransportAffinity {
    pub fn new(
        provider_instance_id: impl Into<String>,
        provider_code: impl Into<String>,
        protocol: impl Into<String>,
        model: impl Into<String>,
    ) -> Self {
        Self {
            provider_instance_id: provider_instance_id.into(),
            provider_code: provider_code.into(),
            protocol: protocol.into(),
            model: model.into(),
        }
    }

    pub fn matches(
        &self,
        provider_instance_id: &str,
        provider_code: &str,
        protocol: &str,
        model: &str,
    ) -> bool {
        self.provider_instance_id == provider_instance_id
            && self.provider_code == provider_code
            && self.protocol == protocol
            && self.model == model
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct ProviderTransportPayload {
    protocol: ProviderTransportProtocol,
    wire_body: Value,
    digest: String,
    size_bytes: usize,
    affinity: Option<ProviderTransportAffinity>,
}

impl ProviderTransportPayload {
    pub fn openai_responses(wire_body: Value) -> anyhow::Result<Self> {
        anyhow::ensure!(
            wire_body.is_object(),
            "provider_transport_payload_must_be_object"
        );
        let encoded = serde_json::to_vec(&wire_body)?;
        let mut canonical = Vec::new();
        write_canonical_json(&wire_body, &mut canonical)?;
        let digest = format!("sha256:{:x}", Sha256::digest(&canonical));
        Ok(Self {
            protocol: ProviderTransportProtocol::OpenAiResponses,
            wire_body,
            digest,
            size_bytes: encoded.len(),
            affinity: None,
        })
    }

    pub fn with_affinity(mut self, affinity: ProviderTransportAffinity) -> Self {
        self.affinity = Some(affinity);
        self
    }

    pub fn bind_openai_continuation(
        mut self,
        continuation: ProviderContinuation,
    ) -> anyhow::Result<Self> {
        anyhow::ensure!(
            self.protocol == ProviderTransportProtocol::OpenAiResponses,
            "provider_continuation_protocol_mismatch"
        );
        let body = self
            .wire_body
            .as_object_mut()
            .ok_or_else(|| anyhow::anyhow!("provider_transport_payload_must_be_object"))?;
        body.insert(
            "previous_response_id".to_string(),
            Value::String(continuation.response_id),
        );
        self.affinity = Some(continuation.affinity);
        let encoded = serde_json::to_vec(&self.wire_body)?;
        let mut canonical = Vec::new();
        write_canonical_json(&self.wire_body, &mut canonical)?;
        self.digest = format!("sha256:{:x}", Sha256::digest(&canonical));
        self.size_bytes = encoded.len();
        Ok(self)
    }

    pub const fn protocol(&self) -> ProviderTransportProtocol {
        self.protocol
    }

    pub fn wire_body(&self) -> &Value {
        &self.wire_body
    }

    pub fn into_wire_body(self) -> Value {
        self.wire_body
    }

    pub fn digest(&self) -> &str {
        &self.digest
    }

    pub const fn size_bytes(&self) -> usize {
        self.size_bytes
    }

    pub fn affinity(&self) -> Option<&ProviderTransportAffinity> {
        self.affinity.as_ref()
    }
}

fn write_canonical_json(value: &Value, out: &mut Vec<u8>) -> serde_json::Result<()> {
    match value {
        Value::Object(object) => {
            out.push(b'{');
            let mut keys = object.keys().collect::<Vec<_>>();
            keys.sort_unstable();
            for (index, key) in keys.into_iter().enumerate() {
                if index > 0 {
                    out.push(b',');
                }
                serde_json::to_writer(&mut *out, key)?;
                out.push(b':');
                write_canonical_json(&object[key], out)?;
            }
            out.push(b'}');
            Ok(())
        }
        Value::Array(values) => {
            out.push(b'[');
            for (index, item) in values.iter().enumerate() {
                if index > 0 {
                    out.push(b',');
                }
                write_canonical_json(item, out)?;
            }
            out.push(b']');
            Ok(())
        }
        _ => serde_json::to_writer(out, value),
    }
}

impl fmt::Debug for ProviderTransportPayload {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProviderTransportPayload")
            .field("protocol", &self.protocol)
            .field("digest", &self.digest)
            .field("size_bytes", &self.size_bytes)
            .field("has_affinity", &self.affinity.is_some())
            .finish_non_exhaustive()
    }
}

#[async_trait]
pub trait ProviderTransportStore: Send + Sync {
    async fn put(
        &self,
        slot_id: ProviderTransportSlotId,
        payload: ProviderTransportPayload,
    ) -> anyhow::Result<()>;

    async fn get(
        &self,
        slot_id: ProviderTransportSlotId,
    ) -> anyhow::Result<Option<ProviderTransportPayload>>;

    /// Atomically transfers a sealed request payload into one execution segment.
    ///
    /// The returned value is owned by that segment and may be reused for Provider retries. The
    /// slot itself must no longer be observable after this call succeeds. Concurrent production
    /// adapters must override the compatibility implementation with one atomic storage action.
    async fn consume(
        &self,
        slot_id: ProviderTransportSlotId,
    ) -> anyhow::Result<ProviderTransportPayload> {
        let payload = self
            .get(slot_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("ephemeral_transport_missing"))?;
        anyhow::ensure!(self.delete(slot_id).await?, "ephemeral_transport_missing");
        Ok(payload)
    }

    async fn delete(&self, slot_id: ProviderTransportSlotId) -> anyhow::Result<bool>;

    async fn put_protocol_context(
        &self,
        slot_id: ProviderProtocolContextSlotId,
        value: ProviderProtocolContextValue,
    ) -> anyhow::Result<()>;

    async fn get_protocol_context(
        &self,
        slot_id: ProviderProtocolContextSlotId,
    ) -> anyhow::Result<Option<ProviderProtocolContextValue>>;

    async fn delete_flow_run_protocol_contexts(&self, flow_run_id: Uuid) -> anyhow::Result<usize>;

    async fn put_continuation(
        &self,
        slot_id: ProviderContinuationSlotId,
        continuation: ProviderContinuation,
    ) -> anyhow::Result<()>;

    async fn get_continuation(
        &self,
        slot_id: ProviderContinuationSlotId,
    ) -> anyhow::Result<Option<ProviderContinuation>>;

    /// Atomically transfers a sealed continuation into one resumed execution segment.
    /// Concurrent production adapters must override the compatibility implementation with one
    /// atomic storage action.
    async fn consume_continuation(
        &self,
        slot_id: ProviderContinuationSlotId,
    ) -> anyhow::Result<ProviderContinuation> {
        let continuation = self
            .get_continuation(slot_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("ephemeral_continuation_missing"))?;
        anyhow::ensure!(
            self.delete_continuation(slot_id).await?,
            "ephemeral_continuation_missing"
        );
        Ok(continuation)
    }

    async fn delete_continuation(
        &self,
        slot_id: ProviderContinuationSlotId,
    ) -> anyhow::Result<bool>;

    /// Clears every sealed Provider value owned by a terminal flow run, or by an execution
    /// segment that has confirmed it will not retry.
    async fn clear_flow_run(&self, flow_run_id: Uuid) -> anyhow::Result<()> {
        self.delete(ProviderTransportSlotId::for_flow_run(flow_run_id))
            .await?;
        self.delete_continuation(ProviderContinuationSlotId::for_flow_run(flow_run_id))
            .await?;
        self.delete_flow_run_protocol_contexts(flow_run_id).await?;
        Ok(())
    }

    /// Eagerly removes expired sealed values. Implementations may also clean them lazily on read.
    async fn clear_expired(&self) -> anyhow::Result<usize> {
        Ok(0)
    }
}
