use serde::{de, Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{Map, Value};

use crate::ports::ProviderTransportPayload;

use crate::application_public_api::protocol_translation::{
    anonymous_unknown_source_paths, TranslationSafeRepresentation,
};

const METADATA_PATH: &str = "$.metadata";

/// Public and durable Native request metadata has one owner: the external trace id.
/// The closed type also carries non-durable adapter admission state, which is
/// intentionally absent from its wire and durable representations.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NativeRequestMetadata {
    trace_id: Option<String>,
    responses_input_history: Option<Value>,
    responses_configuration_digest: Option<String>,
    recovery: Option<crate::application_public_api::callback_resume::NativeInferenceRecoveryGrant>,
    transport_connection_scope: Option<String>,
    observation_context: Option<control_plane_contracts::ports::WorkflowObservationContext>,
    application_run_log_context: Option<control_plane_contracts::ports::ApplicationRunLogContext>,
    responses_transport_requirement: ResponsesTransportRequirement,
    provider_transport_payload: Option<ProviderTransportPayload>,
    provider_transport_summary: Option<ProviderTransportSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProviderTransportSummary {
    protocol: &'static str,
    digest: String,
    size_bytes: usize,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum ResponsesTransportRequirement {
    #[default]
    SemanticCompatible,
    NativePassthrough,
}

impl NativeRequestMetadata {
    pub(super) fn from_object(
        object: &Map<String, Value>,
    ) -> Result<Self, NativeRequestMetadataParseError> {
        let unknown_fields = object
            .keys()
            .filter(|field| field.as_str() != "trace_id")
            .collect::<Vec<_>>();
        if !unknown_fields.is_empty() {
            return Err(NativeRequestMetadataParseError::unknown_fields(
                unknown_fields,
            ));
        }
        let trace_id = match object.get("trace_id") {
            Some(value) => Some(
                value
                    .as_str()
                    .ok_or_else(NativeRequestMetadataParseError::trace_id)?
                    .to_owned(),
            ),
            None => None,
        };
        Ok(Self {
            trace_id,
            responses_input_history: None,
            responses_configuration_digest: None,
            recovery: None,
            transport_connection_scope: None,
            observation_context: None,
            responses_transport_requirement: ResponsesTransportRequirement::default(),
            provider_transport_payload: None,
            provider_transport_summary: None,
            application_run_log_context: None,
        })
    }

    pub fn with_trace_id(trace_id: Option<String>) -> Self {
        Self {
            trace_id,
            responses_input_history: None,
            responses_configuration_digest: None,
            recovery: None,
            transport_connection_scope: None,
            observation_context: None,
            responses_transport_requirement: ResponsesTransportRequirement::default(),
            provider_transport_payload: None,
            provider_transport_summary: None,
            application_run_log_context: None,
        }
    }

    /// Only a Native admission can mint this non-wire recovery grant.
    pub fn set_inference_recovery(
        &mut self,
        grant: crate::application_public_api::callback_resume::NativeInferenceRecoveryGrant,
    ) {
        self.recovery = Some(grant);
    }

    pub(crate) fn inference_recovery(
        &self,
    ) -> Option<&crate::application_public_api::callback_resume::NativeInferenceRecoveryGrant> {
        self.recovery.as_ref()
    }

    pub fn application_run_log_context(
        &self,
    ) -> Option<&control_plane_contracts::ports::ApplicationRunLogContext> {
        self.application_run_log_context.as_ref()
    }

    /// The Native layer owns the call kind: it is the classified AI Native
    /// operation, so protocol mappers cannot declare a kind the runtime will not execute.
    pub(crate) fn set_application_run_log_context(
        &mut self,
        mut context: control_plane_contracts::ports::ApplicationRunLogContext,
        execution_operation: domain::AiNativeOperation,
    ) {
        context.call_kind = Some(execution_operation.call_kind().to_owned());
        self.application_run_log_context = Some(context);
    }

    /// Host-only execution attachment. It is neither public metadata nor protocol context.
    pub fn set_transport_connection_scope(&mut self, scope: Option<String>) {
        self.transport_connection_scope = scope;
    }

    pub fn take_transport_connection_scope(&mut self) -> Option<String> {
        self.transport_connection_scope.take()
    }

    /// This attachment is minted by the host for this request and consumed before persistence.
    pub fn set_observation_context(
        &mut self,
        context: Option<control_plane_contracts::ports::WorkflowObservationContext>,
    ) {
        self.observation_context = context;
    }

    pub fn take_observation_context(
        &mut self,
    ) -> Option<control_plane_contracts::ports::WorkflowObservationContext> {
        self.observation_context.take()
    }

    pub(crate) fn set_responses_input_history(&mut self, history: Option<Value>) {
        self.responses_input_history = history;
    }
    pub(crate) fn set_responses_configuration_digest(&mut self, digest: String) {
        self.responses_configuration_digest = Some(digest);
    }
    pub(crate) fn responses_configuration_digest(&self) -> Option<&str> {
        self.responses_configuration_digest.as_deref()
    }
    pub(crate) fn responses_input_history(&self) -> Option<&Value> {
        self.responses_input_history.as_ref()
    }

    pub fn trace_id(&self) -> Option<&str> {
        self.trace_id.as_deref()
    }

    pub(crate) fn set_responses_transport_requirement(
        &mut self,
        requirement: ResponsesTransportRequirement,
    ) {
        self.responses_transport_requirement = requirement;
    }

    #[cfg(test)]
    pub(crate) fn responses_transport_requirement(&self) -> ResponsesTransportRequirement {
        self.responses_transport_requirement
    }

    /// Admits an adapter-owned wire payload long enough to create its durable-safe summary.
    /// The adapter must take the payload before the request crosses into runtime execution.
    pub fn set_provider_transport_payload(&mut self, payload: ProviderTransportPayload) {
        self.provider_transport_summary = Some(ProviderTransportSummary {
            protocol: match payload.protocol() {
                crate::ports::ProviderTransportProtocol::OpenAiResponses => "openai_responses",
            },
            digest: payload.digest().to_string(),
            size_bytes: payload.size_bytes(),
        });
        self.provider_transport_payload = Some(payload);
    }

    pub fn take_provider_transport_payload(&mut self) -> Option<ProviderTransportPayload> {
        self.provider_transport_payload.take()
    }

    pub(crate) fn provider_transport_summary_value(&self) -> Option<Value> {
        self.provider_transport_summary.as_ref().map(|summary| {
            serde_json::json!({
                "protocol": summary.protocol,
                "digest": summary.digest,
                "size_bytes": summary.size_bytes,
                "storage": "ephemeral",
            })
        })
    }

    pub fn as_value(&self) -> Value {
        let mut metadata = Map::new();
        if let Some(trace_id) = &self.trace_id {
            metadata.insert("trace_id".to_string(), Value::String(trace_id.clone()));
        }
        Value::Object(metadata)
    }
}

impl Serialize for NativeRequestMetadata {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.as_value().serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for NativeRequestMetadata {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;
        let object = value
            .as_object()
            .ok_or_else(|| de::Error::custom("expected object"))?;
        Self::from_object(object).map_err(de::Error::custom)
    }
}

#[derive(Debug)]
pub(super) struct NativeRequestMetadataParseError {
    source_paths: Vec<String>,
    pub(super) message: &'static str,
    pub(super) reason: &'static str,
    pub(super) effective_value: TranslationSafeRepresentation,
}

impl NativeRequestMetadataParseError {
    fn trace_id() -> Self {
        Self {
            source_paths: vec![format!("{METADATA_PATH}.trace_id")],
            message: "metadata trace_id must be a string",
            reason: "Native metadata trace_id must be a string",
            effective_value: TranslationSafeRepresentation::Present,
        }
    }

    fn unknown_fields<'a>(fields: impl IntoIterator<Item = &'a String>) -> Self {
        Self {
            source_paths: anonymous_unknown_source_paths(METADATA_PATH, fields),
            message: "unknown Native metadata field",
            reason: "Native metadata field has no canonical owner",
            effective_value: TranslationSafeRepresentation::Present,
        }
    }

    pub(super) fn source_paths(&self) -> &[String] {
        &self.source_paths
    }
}

impl std::fmt::Display for NativeRequestMetadataParseError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.message)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn workflow_observation_is_host_only_consumed_once_and_not_inherited_from_wire() {
        let previous = uuid::Uuid::now_v7();
        let current = uuid::Uuid::now_v7();
        let mut metadata = NativeRequestMetadata::default();
        metadata.set_observation_context(Some(
            control_plane_contracts::ports::WorkflowObservationContext {
                client_request_id: current,
                context_flow_run_id: Some(previous),
                context_response_id: Some("resp-A".into()),
                is_resume: true,
            },
        ));
        let wire = serde_json::to_value(&metadata).unwrap();
        assert_eq!(wire, json!({}));
        assert_eq!(metadata.as_value(), json!({}));
        let mut replay: NativeRequestMetadata = serde_json::from_value(wire).unwrap();
        assert!(replay.take_observation_context().is_none());
        let context = metadata.take_observation_context().unwrap();
        assert_eq!(context.client_request_id, current);
        assert_eq!(context.context_flow_run_id, Some(previous));
        assert!(metadata.take_observation_context().is_none());
        assert!(serde_json::from_value::<NativeRequestMetadata>(json!({
            "observation_context": {"client_request_id": current, "is_resume": true}
        }))
        .is_err());
    }

    #[test]
    fn qf6_connection_scope_is_host_only_and_never_serialized_or_deserialized() {
        let mut metadata = NativeRequestMetadata::with_trace_id(Some("trace".into()));
        metadata.set_transport_connection_scope(Some("host-socket".into()));
        assert_eq!(
            serde_json::to_value(&metadata).unwrap(),
            json!({"trace_id":"trace"})
        );
        assert_eq!(
            metadata.take_transport_connection_scope().as_deref(),
            Some("host-socket")
        );
        assert!(metadata.take_transport_connection_scope().is_none());
        assert!(serde_json::from_value::<NativeRequestMetadata>(json!({
            "transport_connection_scope": "client-forged"
        }))
        .is_err());
        assert!(NativeRequestMetadata::default()
            .take_transport_connection_scope()
            .is_none());
    }

    #[test]
    fn d4_ac_007_transport_requirement_is_transient_and_not_user_settable() {
        let mut metadata = NativeRequestMetadata::with_trace_id(Some("trace-1".to_string()));
        metadata
            .set_responses_transport_requirement(ResponsesTransportRequirement::NativePassthrough);

        assert_eq!(
            serde_json::to_value(&metadata).unwrap(),
            json!({"trace_id": "trace-1"})
        );
        assert_eq!(
            serde_json::from_value::<NativeRequestMetadata>(json!({
                "trace_id": "trace-1",
                "responses_transport_requirement": "native_passthrough"
            }))
            .unwrap_err()
            .to_string(),
            "unknown Native metadata field"
        );

        let decoded: NativeRequestMetadata = serde_json::from_value(json!({"trace_id": "trace-1"}))
            .expect("public metadata should retain its closed trace-only shape");
        assert_eq!(
            decoded.responses_transport_requirement(),
            ResponsesTransportRequirement::SemanticCompatible
        );
    }

    #[test]
    fn d3_p1_provider_transport_metadata_exposes_only_the_exact_ephemeral_summary() {
        const CANARY: &str = "D3-P1-RAW-PROVIDER-CANARY";
        let mut metadata = NativeRequestMetadata::with_trace_id(Some("trace-1".to_string()));
        let payload = ProviderTransportPayload::openai_responses(json!({
            "model": "gpt-test",
            "provider_target": "must-not-be-durable",
            "input": CANARY,
        }))
        .expect("fixture provider payload should be valid");
        let expected_digest = payload.digest().to_string();
        let expected_size = payload.size_bytes();

        metadata.set_provider_transport_payload(payload);

        assert_eq!(
            metadata.provider_transport_summary_value(),
            Some(json!({
                "protocol": "openai_responses",
                "digest": expected_digest,
                "size_bytes": expected_size,
                "storage": "ephemeral",
            }))
        );
        assert_eq!(
            serde_json::to_value(&metadata).unwrap(),
            json!({"trace_id": "trace-1"})
        );
        assert!(!format!("{metadata:?}").contains(CANARY));
        assert!(!format!("{metadata:?}").contains("must-not-be-durable"));
    }
}
