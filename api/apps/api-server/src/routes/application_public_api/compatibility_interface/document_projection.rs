//! Private safe views decoded from the already parsed protocol documents.
//! These carriers never read source_request, headers, or provider transport.
use super::*;
use crate::event_center::api_projection::{
    self as ep, ApiEventInput, ApiEventOperationContext, ApiEventOutput,
};
use crate::routes::application_public_api::native_interface::document_projection as native;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

// An explicit JSON null is a value; it must not collapse into an absent field.
#[derive(Clone, Default)]
enum Field {
    #[default]
    Absent,
    Present(Value),
}
impl Field {
    fn absent(&self) -> bool {
        matches!(self, Self::Absent)
    }
}
impl<'de> Deserialize<'de> for Field {
    fn deserialize<D: serde::Deserializer<'de>>(decoder: D) -> Result<Self, D::Error> {
        Value::deserialize(decoder).map(Self::Present)
    }
}
impl Serialize for Field {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Absent => serializer.serialize_unit(),
            Self::Present(value) => value.serialize(serializer),
        }
    }
}
macro_rules! document {
    ($name:ident, [$($field:ident),* $(,)?]) => {
        #[derive(Clone, Deserialize, Serialize)]
        pub(crate) struct $name {
            $(#[serde(default, skip_serializing_if="Field::absent")] $field: Field,)*
        }
        impl $name {
            const FIELDS: &'static [&'static str] = &[$(stringify!($field)),*];
            fn schema() -> Value {
                let properties = Self::FIELDS.iter().map(|name| (name.to_string(), Value::Bool(true))).collect::<serde_json::Map<_,_>>();
                json!({"type":"object","properties":properties,"additionalProperties":false})
            }
        }
    }
}
document!(
    ChatDocument,
    [
        model,
        messages,
        stream,
        user,
        metadata,
        max_completion_tokens,
        max_tokens,
        audio,
        modalities,
        tools,
        tool_choice,
        function_call,
        parallel_tool_calls,
        response_format,
        reasoning_effort,
        temperature,
        top_p,
        presence_penalty,
        frequency_penalty,
        seed,
        stop,
        stream_options
    ]
);
document!(
    ResponsesDocument,
    [
        model,
        input,
        instructions,
        stream,
        user,
        metadata,
        max_output_tokens,
        store,
        previous_response_id,
        tools,
        tool_choice,
        parallel_tool_calls,
        response_format,
        text,
        reasoning,
        background,
        include,
        prompt_cache_key,
        client_metadata,
        max_tool_calls,
        truncation,
        generate
    ]
);
document!(
    AnthropicDocument,
    [
        model,
        messages,
        max_tokens,
        system,
        stream,
        metadata,
        tools,
        tool_choice,
        container,
        mcp_servers,
        thinking,
        output_config,
        service_tier,
        temperature,
        top_k,
        top_p,
        stop_sequences,
        stream_options
    ]
);

#[derive(Clone, Serialize)]
#[serde(untagged)]
pub(crate) enum ProtocolRequestDocument {
    Chat(ChatDocument),
    Responses(ResponsesDocument),
    Anthropic(AnthropicDocument),
}
fn decode<T: for<'de> Deserialize<'de>>(value: &Value) -> Result<T, Arc<str>> {
    T::deserialize(value).map_err(|_| Arc::from("invalid_safe_protocol_document"))
}
fn safe_mcp_tools(field: &mut Field) {
    if let Field::Present(Value::Array(tools)) = field {
        for tool in tools {
            if tool.get("type").and_then(Value::as_str) == Some("mcp") {
                if let Some(fields) = tool.as_object_mut() {
                    fields.remove("authorization");
                    fields.remove("headers");
                }
            }
        }
    }
}
impl ProtocolRequestDocument {
    pub(crate) fn chat(value: &Value) -> Result<Self, Arc<str>> {
        let mut document: ChatDocument = decode(value)?;
        safe_mcp_tools(&mut document.tools);
        Ok(Self::Chat(document))
    }
    pub(crate) fn responses(value: &Value) -> Result<Self, Arc<str>> {
        let mut document: ResponsesDocument = decode(value)?;
        safe_mcp_tools(&mut document.tools);
        Ok(Self::Responses(document))
    }
    pub(crate) fn anthropic(value: &Value) -> Result<Self, Arc<str>> {
        let mut document: AnthropicDocument = decode(value)?;
        safe_mcp_tools(&mut document.tools);
        if let Field::Present(Value::Array(servers)) = &mut document.mcp_servers {
            for server in servers {
                if let Some(fields) = server.as_object_mut() {
                    fields.remove("authorization_token");
                    fields.remove("headers");
                }
            }
        }
        Ok(Self::Anthropic(document))
    }
}
impl ApiEventInput for CompatibilityBlockingInput {
    fn document_input_schema(operation: &ApiEventOperationContext) -> Option<Value> {
        if operation.method == "GET" {
            return Some(json!({"type":"object","properties":{},"additionalProperties":false}));
        }
        let body = if operation
            .interface_id
            .starts_with("compat.openai.chat-completions")
        {
            ChatDocument::schema()
        } else if operation
            .interface_id
            .starts_with("compat.openai.responses")
        {
            ResponsesDocument::schema()
        } else {
            AnthropicDocument::schema()
        };
        Some(
            json!({"type":"object","properties":{"body":body},"required":["body"],"additionalProperties":false}),
        )
    }
    fn project_document_input(
        &self,
        operation: &ApiEventOperationContext,
    ) -> Result<Value, Arc<str>> {
        // A WebSocket turn has no HTTP request body. Never mislabel a socket frame
        // or its transport context as a GET argument.
        if operation.method == "GET" {
            return Ok(json!({}));
        }
        Ok(json!({"body":ep::to_value(self.document.as_ref().map_err(Arc::clone)?)?}))
    }
}
impl ApiEventOutput for CompatibilityBlockingOutput {
    fn document_output_schema(_: &ApiEventOperationContext) -> Option<Value> {
        Some(native::result_schema())
    }
    fn project_document_output(&self, _: &ApiEventOperationContext) -> Result<Value, Arc<str>> {
        // The typed run completion owns this business terminal. It is independent
        // from downstream SSE frames and socket-close or delivery receipts.
        native::result_document(&self.0)
    }
}

#[cfg_attr(test, api_test_selection::group(public_api))]
#[cfg(test)]
#[path = "_tests/document_projection.rs"]
mod tests;
