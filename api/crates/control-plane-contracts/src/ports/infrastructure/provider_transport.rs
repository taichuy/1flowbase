use std::{
    fmt,
    io::{self, Write},
    sync::Arc,
};

use async_trait::async_trait;
use extension_contracts::provider_contract::ProtocolContextEnvelope;
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

    pub const fn as_uuid(self) -> Uuid {
        self.0
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct ProviderContinuationSlotId {
    flow_run_id: Uuid,
    response_round_id: Option<Uuid>,
    resume_claim_id: Option<Uuid>,
}

impl ProviderContinuationSlotId {
    pub const fn for_flow_run(flow_run_id: Uuid) -> Self {
        Self {
            flow_run_id,
            response_round_id: None,
            resume_claim_id: None,
        }
    }

    /// Immutable public response identity, separate from the mutable waiting-node slot.
    pub const fn for_response_round(flow_run_id: Uuid, response_round_id: Uuid) -> Self {
        Self {
            flow_run_id,
            response_round_id: Some(response_round_id),
            resume_claim_id: None,
        }
    }

    /// Immutable input captured for one durable resume claim, across lease generations.
    pub const fn for_resume_claim(flow_run_id: Uuid, resume_claim_id: Uuid) -> Self {
        Self {
            flow_run_id,
            response_round_id: None,
            resume_claim_id: Some(resume_claim_id),
        }
    }

    pub const fn belongs_to(self, flow_run_id: Uuid) -> bool {
        self.flow_run_id.as_u128() == flow_run_id.as_u128()
    }

    pub const fn flow_run_id(self) -> Uuid {
        self.flow_run_id
    }

    pub fn storage_key(self) -> String {
        if let Some(id) = self.resume_claim_id {
            format!("claim:{id}")
        } else {
            self.response_round_id
                .map_or_else(|| "current".to_string(), |id| id.to_string())
        }
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

    pub fn storage_key(self) -> String {
        match self.slot {
            ProviderProtocolContextSlot::Original => "original".to_string(),
            ProviderProtocolContextSlot::Derived(id) => id.to_string(),
        }
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
        let (digest, size_bytes) = canonical_digest_and_size(&value)?;
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
            digest,
            size_bytes,
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

    pub fn digest(&self) -> &str {
        &self.digest
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

#[derive(Clone, PartialEq, Eq)]
pub struct ProviderContinuation {
    response_id: String,
    affinity: ProviderTransportAffinity,
    // Host-produced evidence; never read from the client wire body. Slot + affinity own it.
    native_history: Option<Value>,
    // Host-sealed Responses session identity for headerless previous_response_id turns.
    session_identity: Option<String>,
}

impl fmt::Debug for ProviderContinuation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProviderContinuation")
            .field("affinity", &self.affinity)
            .finish_non_exhaustive()
    }
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
            native_history: None,
            session_identity: None,
        })
    }

    pub fn with_session_identity(mut self, identity: Option<&str>) -> anyhow::Result<Self> {
        if let Some(identity) = identity {
            anyhow::ensure!(
                identity.len() == 64 && identity.bytes().all(|byte| byte.is_ascii_hexdigit()),
                "provider_continuation_session_identity_invalid"
            );
            self.session_identity = Some(identity.to_owned());
        }
        Ok(self)
    }

    pub fn session_identity(&self) -> Option<&str> {
        self.session_identity.as_deref()
    }

    /// Attached only by the host after successful completion of the response round.
    pub fn with_native_history(mut self, history: Option<Value>) -> Self {
        self.native_history = history;
        self
    }

    pub fn native_history(&self) -> Option<&Value> {
        self.native_history.as_ref()
    }

    pub fn response_id(&self) -> &str {
        &self.response_id
    }

    pub fn affinity(&self) -> &ProviderTransportAffinity {
        &self.affinity
    }

    pub fn matches_affinity(&self, expected: &ProviderTransportAffinity) -> bool {
        &self.affinity == expected
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

    pub fn provider_instance_id(&self) -> &str {
        &self.provider_instance_id
    }

    pub fn provider_code(&self) -> &str {
        &self.provider_code
    }

    pub fn protocol(&self) -> &str {
        &self.protocol
    }

    pub fn model(&self) -> &str {
        &self.model
    }
}

/// Durable encrypted owner for protocol context and opaque Provider continuations.
///
/// AI Native receives only the typed opaque handles above. Implementations must encrypt values
/// before persistence and enforce a bounded hard retention independently of socket lifetimes.
#[async_trait]
pub trait ProviderProtocolCapsuleStore: Send + Sync {
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

    async fn consume_continuation(
        &self,
        slot_id: ProviderContinuationSlotId,
    ) -> anyhow::Result<ProviderContinuation>;

    /// Atomically moves the mutable current continuation into an immutable claim-owned slot.
    /// Reacquiring the same claim returns its original value without touching a newer current.
    async fn claim_continuation(
        &self,
        flow_run_id: Uuid,
        resume_claim_id: Uuid,
    ) -> anyhow::Result<ProviderContinuation>;

    async fn delete_continuation(
        &self,
        slot_id: ProviderContinuationSlotId,
    ) -> anyhow::Result<bool>;

    async fn delete_flow_run_continuations(&self, flow_run_id: Uuid) -> anyhow::Result<usize>;

    async fn clear_expired(&self) -> anyhow::Result<usize>;
}

#[derive(Clone, PartialEq, Eq)]
pub struct ProviderTransportPayload {
    protocol: ProviderTransportProtocol,
    wire_body: Arc<Value>,
    digest: String,
    size_bytes: usize,
    affinity: Option<ProviderTransportAffinity>,
    native_history: Option<Value>,
}

impl ProviderTransportPayload {
    pub fn openai_responses(wire_body: Value) -> anyhow::Result<Self> {
        anyhow::ensure!(
            wire_body.is_object(),
            "provider_transport_payload_must_be_object"
        );
        let (digest, size_bytes) = canonical_digest_and_size(&wire_body)?;
        Ok(Self {
            protocol: ProviderTransportProtocol::OpenAiResponses,
            wire_body: Arc::new(wire_body),
            digest,
            size_bytes,
            affinity: None,
            native_history: None,
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
        let body = Arc::make_mut(&mut self.wire_body)
            .as_object_mut()
            .ok_or_else(|| anyhow::anyhow!("provider_transport_payload_must_be_object"))?;
        body.insert(
            "previous_response_id".to_string(),
            Value::String(continuation.response_id),
        );
        self.affinity = Some(continuation.affinity);
        self.native_history = continuation.native_history;
        let (digest, size_bytes) = canonical_digest_and_size(&self.wire_body)?;
        self.digest = digest;
        self.size_bytes = size_bytes;
        Ok(self)
    }

    /// Trusted predecessor proof carried outside the client-controlled wire body.
    pub fn native_history(&self) -> Option<&Value> {
        self.native_history.as_ref()
    }

    pub const fn protocol(&self) -> ProviderTransportProtocol {
        self.protocol
    }

    pub fn wire_body(&self) -> &Value {
        &self.wire_body
    }

    pub fn into_wire_body(self) -> Value {
        Arc::try_unwrap(self.wire_body).unwrap_or_else(|shared| (*shared).clone())
    }

    pub fn digest(&self) -> &str {
        &self.digest
    }

    /// Identifies the generation configuration independently of history and delivery metadata.
    pub fn user_messages_digest(&self) -> anyhow::Result<Option<String>> {
        let Some(input) = self.wire_body.get("input").and_then(Value::as_array) else {
            return Ok(None);
        };
        let mut messages = input
            .iter()
            .filter(|item| item.get("role").and_then(Value::as_str) == Some("user"))
            .peekable();
        if messages.peek().is_none() {
            return Ok(None);
        }
        let mut writer = DigestWriter::new();
        writer.write_all(b"[")?;
        for (index, message) in messages.enumerate() {
            if index > 0 {
                writer.write_all(b",")?;
            }
            write_canonical_json(message, &mut writer)?;
        }
        writer.write_all(b"]")?;
        Ok(Some(writer.finish().0))
    }

    pub fn configuration_digest(&self) -> anyhow::Result<String> {
        Self::openai_responses_configuration_digest(&self.wire_body)
    }

    pub fn openai_responses_configuration_digest(wire_body: &Value) -> anyhow::Result<String> {
        let mut writer = DigestWriter::new();
        if let Some(body) = wire_body.as_object() {
            write_canonical_object(body, CONFIGURATION_OMITTED_FIELDS, None, &mut writer)?;
        } else {
            write_canonical_json(wire_body, &mut writer)?;
        }
        Ok(writer.finish().0)
    }

    pub fn openai_responses_configuration_digest_with_reasoning_default(
        wire_body: &Value,
        default: &str,
    ) -> anyhow::Result<String> {
        let body = wire_body
            .as_object()
            .ok_or_else(|| anyhow::anyhow!("provider_transport_payload_must_be_object"))?;
        let mut reasoning = match body.get("reasoning") {
            Some(Value::Object(reasoning)) if reasoning.contains_key("effort") => {
                return Self::openai_responses_configuration_digest(wire_body);
            }
            Some(Value::Object(reasoning)) => reasoning.clone(),
            Some(Value::Null) | None => serde_json::Map::new(),
            _ => anyhow::bail!("provider_transport_reasoning_must_be_object"),
        };
        reasoning.insert("effort".to_owned(), Value::String(default.to_owned()));
        let reasoning = Value::Object(reasoning);
        let mut writer = DigestWriter::new();
        write_canonical_object(
            body,
            CONFIGURATION_OMITTED_FIELDS,
            Some(("reasoning", &reasoning)),
            &mut writer,
        )?;
        Ok(writer.finish().0)
    }

    pub const fn size_bytes(&self) -> usize {
        self.size_bytes
    }

    pub fn affinity(&self) -> Option<&ProviderTransportAffinity> {
        self.affinity.as_ref()
    }
}

const CONFIGURATION_OMITTED_FIELDS: &[&str] = &[
    "input",
    "previous_response_id",
    "stream",
    "stream_options",
    "client_metadata",
    "metadata",
];

struct DigestWriter {
    hash: Sha256,
    bytes: usize,
}

impl DigestWriter {
    fn new() -> Self {
        Self {
            hash: Sha256::new(),
            bytes: 0,
        }
    }

    fn finish(self) -> (String, usize) {
        (format!("sha256:{:x}", self.hash.finalize()), self.bytes)
    }
}

impl Write for DigestWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.bytes = self
            .bytes
            .checked_add(bytes.len())
            .ok_or_else(|| io::Error::other("provider_transport_payload_size_overflow"))?;
        self.hash.update(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn canonical_digest_and_size(value: &Value) -> anyhow::Result<(String, usize)> {
    let mut writer = DigestWriter::new();
    write_canonical_json(value, &mut writer)?;
    // Sorting object members changes their order, not their serialized byte count.
    Ok(writer.finish())
}

fn write_canonical_json(value: &Value, out: &mut impl Write) -> anyhow::Result<()> {
    match value {
        Value::Object(object) => write_canonical_object(object, &[], None, out),
        Value::Array(values) => {
            out.write_all(b"[")?;
            for (index, item) in values.iter().enumerate() {
                if index > 0 {
                    out.write_all(b",")?;
                }
                write_canonical_json(item, out)?;
            }
            out.write_all(b"]")?;
            Ok(())
        }
        _ => Ok(serde_json::to_writer(out, value)?),
    }
}

fn write_canonical_object(
    object: &serde_json::Map<String, Value>,
    omitted: &[&str],
    override_field: Option<(&str, &Value)>,
    out: &mut impl Write,
) -> anyhow::Result<()> {
    out.write_all(b"{")?;
    let mut keys = object
        .keys()
        .map(String::as_str)
        .filter(|key| !omitted.contains(key))
        .collect::<Vec<_>>();
    if let Some((key, _)) = override_field {
        if !object.contains_key(key) {
            keys.push(key);
        }
    }
    keys.sort_unstable();
    for (index, key) in keys.into_iter().enumerate() {
        if index > 0 {
            out.write_all(b",")?;
        }
        serde_json::to_writer(&mut *out, key)?;
        out.write_all(b":")?;
        let value = override_field
            .filter(|(overridden_key, _)| *overridden_key == key)
            .map(|(_, value)| value)
            .unwrap_or_else(|| &object[key]);
        write_canonical_json(value, out)?;
    }
    out.write_all(b"}")?;
    Ok(())
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

    /// Production adapters must override this with one atomic storage action.
    async fn claim_continuation(
        &self,
        flow_run_id: Uuid,
        resume_claim_id: Uuid,
    ) -> anyhow::Result<ProviderContinuation> {
        let claimed = ProviderContinuationSlotId::for_resume_claim(flow_run_id, resume_claim_id);
        if let Some(continuation) = self.get_continuation(claimed).await? {
            return Ok(continuation);
        }
        let continuation = self
            .consume_continuation(ProviderContinuationSlotId::for_flow_run(flow_run_id))
            .await?;
        self.put_continuation(claimed, continuation.clone()).await?;
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
