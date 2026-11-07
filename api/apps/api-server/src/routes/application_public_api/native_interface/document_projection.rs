//! HTTP business documents; protocol envelopes and credential references stay private.
use super::*;
use crate::event_center::api_projection::{
    self as ep, ApiEventInput, ApiEventOperationContext, ApiEventOutput,
};
use serde_json::{json, Value};

pub(crate) fn request_document(request: &NativeRunRequest) -> Result<Value, Arc<str>> {
    let mut value = ep::to_value(request)?;
    value
        .as_object_mut()
        .expect("Native request is an object")
        .remove("client_protocol_envelope");
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
        if let Some(sys) = payload.get_mut("sys").and_then(Value::as_object_mut) {
            sys.remove("native_inference_recovery");
            sys.remove("provider_session_identity");
        }
    }
    Ok(value)
}

pub(crate) fn result_document(run: &NativeRunResult) -> Result<Value, Arc<str>> {
    response_document(&super::super::native::to_native_run_response(run.clone()))
}

impl ApiEventInput for ApplicationNativeRunInput {
    fn project_document_input(&self, _: &ApiEventOperationContext) -> Result<Value, Arc<str>> {
        Ok(json!({"body": request_document(&self.request)?}))
    }
}

impl ApiEventOutput for ApplicationNativeRunOutput {
    fn document_output_schema(_: &ApiEventOperationContext) -> Option<Value> {
        Some(result_schema())
    }
    fn project_document_output(&self, _: &ApiEventOperationContext) -> Result<Value, Arc<str>> {
        result_document(&self.0)
    }
}

#[cfg_attr(test, api_test_selection::group(public_api))]
#[cfg(test)]
#[path = "_tests/document_projection.rs"]
mod tests;
