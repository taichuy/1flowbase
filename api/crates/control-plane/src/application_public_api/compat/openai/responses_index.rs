use std::collections::BTreeMap;

use serde_json::Value;

use super::OpenAiCompatError;
use crate::application_public_api::protocol_translation::{TranslationProtocol, TranslationReport};
use crate::ports::ProviderTransportPayload;

pub(crate) const MAX_NATIVE_RESPONSES_INPUT_ITEMS: usize = 4_096;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ResponsesInputIndexError {
    RequestKind,
    InputKind,
    PreviousResponseId,
    TooManyItems,
    ItemKind { index: usize },
    ItemType { index: usize },
}

impl ResponsesInputIndexError {
    pub(crate) const fn param(self) -> &'static str {
        match self {
            Self::RequestKind => "request",
            Self::PreviousResponseId => "previous_response_id",
            Self::InputKind
            | Self::TooManyItems
            | Self::ItemKind { .. }
            | Self::ItemType { .. } => "input",
        }
    }

    pub(crate) const fn message(self) -> &'static str {
        match self {
            Self::RequestKind => "Responses request must be an object",
            Self::InputKind => "input must be text or an array",
            Self::PreviousResponseId => "previous_response_id must be non-empty text",
            Self::TooManyItems => "Responses input has too many items",
            Self::ItemKind { .. } => "input items must be objects",
            Self::ItemType { .. } => "input item type must be text",
        }
    }

    pub(crate) fn receipt_source_path(self) -> Option<String> {
        match self {
            Self::InputKind | Self::TooManyItems => Some("$.input".to_string()),
            Self::ItemKind { index } => Some(format!("$.input[{index}]")),
            Self::ItemType { index } => Some(format!("$.input[{index}].type")),
            Self::RequestKind | Self::PreviousResponseId => None,
        }
    }
}

/// Request-level protocol index shared by ingress correlation and translation.
/// The opaque body itself remains in the request/`ProviderTransportPayload`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ResponsesRequestIndex {
    previous_response_id: Option<String>,
    input: ResponsesInputIndex,
}

impl ResponsesRequestIndex {
    pub(crate) fn build(request: &Value) -> Result<Self, ResponsesInputIndexError> {
        let object = request
            .as_object()
            .ok_or(ResponsesInputIndexError::RequestKind)?;
        let previous_response_id = match object.get("previous_response_id") {
            Some(Value::String(value)) if !value.is_empty() => Some(value.clone()),
            Some(_) => return Err(ResponsesInputIndexError::PreviousResponseId),
            None => None,
        };
        let input = object
            .get("input")
            .ok_or(ResponsesInputIndexError::InputKind)
            .and_then(ResponsesInputIndex::build)?;
        Ok(Self {
            previous_response_id,
            input,
        })
    }

    pub(crate) fn previous_response_id(&self) -> Option<&str> {
        self.previous_response_id.as_deref()
    }

    pub(crate) const fn input(&self) -> &ResponsesInputIndex {
        &self.input
    }
}

/// Ephemeral protocol envelope shared by ingress, correlation, translation,
/// and native Provider staging. Its debug view never renders the opaque body.
#[derive(Clone)]
pub struct OpenAiResponsesEnvelope {
    transport: ProviderTransportPayload,
    index: ResponsesRequestIndex,
}

impl std::fmt::Debug for OpenAiResponsesEnvelope {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("OpenAiResponsesEnvelope")
            .field("transport_digest", &self.transport.digest())
            .field("transport_size_bytes", &self.transport.size_bytes())
            .field("index", &self.index)
            .finish()
    }
}

impl OpenAiResponsesEnvelope {
    pub fn capture(raw_body: Value) -> Result<Self, OpenAiCompatError> {
        let index = ResponsesRequestIndex::build(&raw_body).map_err(|error| {
            OpenAiCompatError::invalid(error.param(), error.message())
                .with_report(TranslationReport::new(TranslationProtocol::OpenAiResponses))
        })?;
        let transport = ProviderTransportPayload::openai_responses(raw_body).map_err(|_| {
            OpenAiCompatError::invalid("request", "Responses request must be an object")
                .with_report(TranslationReport::new(TranslationProtocol::OpenAiResponses))
        })?;
        Ok(Self { transport, index })
    }

    pub fn previous_response_id(&self) -> Option<&str> {
        self.index.previous_response_id()
    }

    pub fn raw_body(&self) -> &Value {
        self.transport.wire_body()
    }

    pub fn provider_transport_payload(&self) -> ProviderTransportPayload {
        self.transport.clone()
    }

    pub(crate) const fn index(&self) -> &ResponsesRequestIndex {
        &self.index
    }

    pub(crate) fn into_parts(self) -> (Value, ResponsesRequestIndex) {
        (self.transport.into_wire_body(), self.index)
    }
}

/// Bounded, content-free index over an opaque Responses input.
///
/// The raw body remains owned by `ProviderTransportPayload`. This index stores
/// only positions and identity fields required by the protocol adapter, so AI
/// Native does not need an exhaustive Responses item enum.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ResponsesInputIndex {
    item_count: usize,
    item_types: Vec<Option<String>>,
    user_message_positions: Vec<usize>,
    tool_output_positions: Vec<usize>,
    outputs_by_call_id: BTreeMap<String, Vec<usize>>,
    last_tool_call_position: Option<usize>,
}

impl ResponsesInputIndex {
    pub(crate) fn build(input: &Value) -> Result<Self, ResponsesInputIndexError> {
        if input.is_string() {
            return Ok(Self {
                item_count: 0,
                item_types: Vec::new(),
                user_message_positions: Vec::new(),
                tool_output_positions: Vec::new(),
                outputs_by_call_id: BTreeMap::new(),
                last_tool_call_position: None,
            });
        }
        let items = input
            .as_array()
            .ok_or(ResponsesInputIndexError::InputKind)?;
        if items.len() > MAX_NATIVE_RESPONSES_INPUT_ITEMS {
            return Err(ResponsesInputIndexError::TooManyItems);
        }

        let mut index = Self {
            item_count: items.len(),
            item_types: Vec::with_capacity(items.len()),
            user_message_positions: Vec::new(),
            tool_output_positions: Vec::new(),
            outputs_by_call_id: BTreeMap::new(),
            last_tool_call_position: None,
        };
        for (position, item) in items.iter().enumerate() {
            let object = item
                .as_object()
                .ok_or(ResponsesInputIndexError::ItemKind { index: position })?;
            let item_type = match object.get("type") {
                Some(Value::String(value)) => Some(value.clone()),
                Some(_) => return Err(ResponsesInputIndexError::ItemType { index: position }),
                None => None,
            };
            if object.get("role").and_then(Value::as_str) == Some("user") {
                index.user_message_positions.push(position);
            }
            match item_type.as_deref() {
                Some("function_call" | "custom_tool_call") => {
                    index.last_tool_call_position = Some(position);
                }
                Some("function_call_output" | "custom_tool_call_output") => {
                    index.tool_output_positions.push(position);
                    if let Some(call_id) = object
                        .get("call_id")
                        .and_then(Value::as_str)
                        .filter(|value| !value.is_empty())
                    {
                        index
                            .outputs_by_call_id
                            .entry(call_id.to_owned())
                            .or_default()
                            .push(position);
                    }
                }
                _ => {}
            }
            index.item_types.push(item_type);
        }
        Ok(index)
    }

    pub(crate) const fn item_count(&self) -> usize {
        self.item_count
    }

    pub(crate) fn item_types(&self) -> &[Option<String>] {
        &self.item_types
    }

    pub(crate) fn outputs_by_call_id(&self) -> &BTreeMap<String, Vec<usize>> {
        &self.outputs_by_call_id
    }

    /// Selects the latest tool-call segment while allowing opaque context items
    /// after its outputs. A later user message starts a new turn and suppresses
    /// callback admission.
    pub(crate) fn current_tool_output_positions(&self) -> Vec<usize> {
        let start = self
            .last_tool_call_position
            .map_or(0, |position| position + 1);
        let positions = self
            .tool_output_positions
            .iter()
            .copied()
            .filter(|position| *position >= start)
            .collect::<Vec<_>>();
        let Some(first_output) = positions.first().copied() else {
            return positions;
        };
        if self
            .user_message_positions
            .iter()
            .any(|position| *position > first_output)
        {
            return Vec::new();
        }
        positions
    }
}
