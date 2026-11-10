use serde_json::Value;

pub(crate) fn external_llm_tool_calls(tool_calls: Option<&Value>) -> Option<Vec<&Value>> {
    let calls = tool_calls?.as_array()?;
    external_llm_tool_call_values(calls)
}

pub(crate) fn external_llm_tool_call_values(calls: &[Value]) -> Option<Vec<&Value>> {
    let calls = calls
        .iter()
        .filter(|call| !llm_tool_call_is_internal(call))
        .collect::<Vec<_>>();

    (!calls.is_empty()).then_some(calls)
}

use control_plane::application_public_api::compat::openai::projection::llm_tool_call_is_internal;
