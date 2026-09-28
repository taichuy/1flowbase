pub fn item_kind_for_event(event_type: &str) -> Option<domain::RuntimeItemKind> {
    match event_type {
        "tool_call_commit" => Some(domain::RuntimeItemKind::ToolCall),
        "mcp_call_commit" => Some(domain::RuntimeItemKind::McpCall),
        "context_compaction_recorded" => Some(domain::RuntimeItemKind::Compaction),
        "gateway_forward_started" | "gateway_forward_finished" => {
            Some(domain::RuntimeItemKind::GatewayForward)
        }
        _ => None,
    }
}
