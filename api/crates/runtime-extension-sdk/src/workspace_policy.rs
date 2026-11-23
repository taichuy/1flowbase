use crate::CapabilityStdioResponse;
use extension_contracts::{WorkspacePolicyRequest, WorkspacePolicyResponse};
use serde_json::Value;

/// Decode only Execute calls for the manifest's policy handler. Other handlers
/// (e.g. managed services) remain with the plugin's existing dispatcher.
pub fn workspace_policy_input(
    envelope: &Value,
    handler: &str,
) -> Result<Option<WorkspacePolicyRequest>, serde_json::Error> {
    let Some(input) = envelope
        .get("input")
        .filter(|input| input["handler"].as_str() == Some(handler))
    else {
        return Ok(None);
    };
    if envelope.get("method").and_then(Value::as_str) != Some("execute") {
        return Ok(None);
    }
    serde_json::from_value(input["input_payload"].clone()).map(Some)
}

/// Encode a typed decision in the outer capability response envelope.
pub fn workspace_policy_output(result: WorkspacePolicyResponse) -> Value {
    serde_json::to_value(CapabilityStdioResponse {
        ok: true,
        result: serde_json::to_value(result).expect("workspace policy response is JSON"),
        error: None,
    })
    .expect("capability response is JSON")
}

#[cfg(test)]
#[path = "_tests/host_services/workspace_policy_sdk_tests.rs"]
mod tests;
