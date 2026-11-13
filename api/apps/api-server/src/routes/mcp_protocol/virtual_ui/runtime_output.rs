use super::*;

pub(super) fn runtime_internal_tool_output(
    node: &CompiledNode,
    registration: &RuntimeInternalToolRegistration,
    outcome: VirtualToolOutcome,
) -> anyhow::Result<RuntimeInternalToolOutput> {
    let (content, is_error) = match outcome {
        VirtualToolOutcome::Success(value) => (model_tool_content(value), false),
        VirtualToolOutcome::Error {
            code,
            message,
            data,
        } => (
            json!({"code": code, "message": message, "data": data}),
            true,
        ),
    };
    Ok(RuntimeInternalToolOutput {
        content,
        is_error,
        event: json!({
            "event_type": "mcp_runtime_tool_call_completed",
            "node_id": node.node_id,
            "provider_name": registration.provider_name,
            "owner": registration.owner,
            "is_error": is_error,
        }),
    })
}

/// MCP CallToolResult is an envelope. The LLM callback consumes content blocks,
/// not a JSON string containing that envelope. This runs only after an explicit tool call.
fn model_tool_content(value: Value) -> Value {
    let Some(blocks) = value.get("content").and_then(Value::as_array) else {
        return value;
    };
    if !blocks.iter().any(|b| b["type"] == "image")
        || !blocks
            .iter()
            .all(|b| matches!(b["type"].as_str(), Some("text" | "image")))
    {
        return value;
    }
    // The MCP media projector already validates MIME and standard base64.
    Value::Array(blocks.iter().map(|block| {
        if block["type"]=="image" {
            json!({"type":"image","source":{"type":"base64","media_type":block["mimeType"],"data":block["data"]}})
        } else {block.clone()}
    }).collect())
}

#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
#[path = "../_tests/runtime_output.rs"]
mod tests;
