//! HTTP business documents; protocol envelopes and credential references stay private.
use super::*;
use crate::openapi_interface::contract::{self as ep, ApiInput, ApiOperationContext, ApiOutput};
use serde_json::{json, Value};

pub(crate) fn request_document(request: &NativeRunRequest) -> Result<Value, Arc<str>> {
    let mut value = ep::to_value(request)?;
    value
        .as_object_mut()
        .expect("Native request is an object")
        .remove("client_protocol_envelope");
    if let Some(tools) = value.pointer_mut("/inputs/tools") {
        safe_mcp_tools(tools);
    }
    Ok(value)
}

pub(crate) fn result_schema() -> Value {
    let mut schema = ep::schema::<super::super::native::NativeRunResponse>();
    schema["properties"]
        .as_object_mut()
        .expect("Native result properties")
        .remove("api_key_id");
    if let Some(required) = schema["required"].as_array_mut() {
        required.retain(|field| field != "api_key_id");
    }
    schema["additionalProperties"] = json!(false);
    schema
}

pub(crate) fn response_document(
    response: &super::super::native::NativeRunResponse,
) -> Result<Value, Arc<str>> {
    let mut value = ep::to_value(response)?;
    value
        .as_object_mut()
        .expect("Native result is an object")
        .remove("api_key_id");
    // These exact owner slots contain frozen application secrets, protocol
    // locators, and recovery/routing grants. Business JSON below other keys is
    // preserved, including nested objects whose keys happen to be `env`.
    if let Some(payload) = value
        .get_mut("node_input_payload")
        .and_then(Value::as_object_mut)
    {
        payload.remove("env");
        payload.remove(plugin_framework::provider_contract::CLIENT_PROTOCOL_ENVELOPE_PAYLOAD_KEY);
        if let Some(tools) = payload
            .get_mut(plugin_framework::provider_contract::NATIVE_MODEL_TOOL_CONTEXT_PAYLOAD_KEY)
            .and_then(|context| context.get_mut("tools"))
        {
            safe_mcp_tools(tools);
        }
        if let Some(sys) = payload.get_mut("sys").and_then(Value::as_object_mut) {
            sys.remove("native_inference_recovery");
            sys.remove("provider_session_identity");
        }
    }
    Ok(value)
}

pub(crate) fn safe_mcp_tools(value: &mut Value) {
    if let Some(tools) = value.as_array_mut() {
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

pub(crate) fn result_document(run: &NativeRunResult) -> Result<Value, Arc<str>> {
    response_document(&super::super::native::to_native_run_response(run.clone()))
}

impl ApiInput for ApplicationNativeRunInput {
    fn project_document_input(&self, _: &ApiOperationContext) -> Result<Value, Arc<str>> {
        Ok(json!({"body": request_document(&self.request)?}))
    }
}

impl ApiOutput for ApplicationNativeRunOutput {
    fn document_output_schema(_: &ApiOperationContext) -> Option<Value> {
        Some(result_schema())
    }
    fn project_document_output(&self, _: &ApiOperationContext) -> Result<Value, Arc<str>> {
        result_document(&self.0)
    }
}

#[cfg_attr(test, api_test_selection::group(public_api))]
#[cfg(test)]
#[path = "_tests/document_projection.rs"]
mod tests;
