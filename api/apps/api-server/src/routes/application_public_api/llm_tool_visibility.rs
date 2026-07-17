use serde_json::Value;

const VISIBLE_INTERNAL_LLM_TOOL_TYPE: &str = "visible_internal_llm_tool";

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

fn llm_tool_call_is_internal(call: &Value) -> bool {
    call.get("type").and_then(Value::as_str) == Some(VISIBLE_INTERNAL_LLM_TOOL_TYPE)
        || call.get("origin").and_then(Value::as_str) == Some(VISIBLE_INTERNAL_LLM_TOOL_TYPE)
        || call.get("source").and_then(Value::as_str) == Some(VISIBLE_INTERNAL_LLM_TOOL_TYPE)
        || call.get("visibility").and_then(Value::as_str) == Some("internal")
        || call
            .get("metadata")
            .is_some_and(metadata_marks_internal_llm_tool_call)
}

fn metadata_marks_internal_llm_tool_call(metadata: &Value) -> bool {
    metadata.get("type").and_then(Value::as_str) == Some(VISIBLE_INTERNAL_LLM_TOOL_TYPE)
        || metadata.get("origin").and_then(Value::as_str) == Some(VISIBLE_INTERNAL_LLM_TOOL_TYPE)
        || metadata.get("visibility").and_then(Value::as_str) == Some("internal")
}
