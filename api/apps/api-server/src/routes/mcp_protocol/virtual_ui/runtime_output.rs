use super::*;

pub(super) fn runtime_internal_tool_output(
    node: &CompiledNode,
    registration: &RuntimeInternalToolRegistration,
    outcome: VirtualToolOutcome,
) -> anyhow::Result<RuntimeInternalToolOutput> {
    let (content, is_error) = match outcome {
        VirtualToolOutcome::Success(value) => (value, false),
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
