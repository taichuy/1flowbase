use std::fmt;

use async_trait::async_trait;
use serde_json::Value;
use sha2::{Digest, Sha256};
use uuid::Uuid;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderTransportProtocol {
    OpenAiResponses,
}

/// Runtime-only identity of the Provider route that owns opaque continuation state.
///
/// This value is derived from an actual LLM invocation. It is deliberately not serializable
/// because workflow variables and durable request bodies must not become a second routing owner.
#[derive(Clone, PartialEq, Eq)]
pub struct ProviderTransportAffinity {
    provider_instance_id: String,
    provider_code: String,
    protocol: String,
    model: String,
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

    async fn delete(&self, slot_id: ProviderTransportSlotId) -> anyhow::Result<bool>;
}
