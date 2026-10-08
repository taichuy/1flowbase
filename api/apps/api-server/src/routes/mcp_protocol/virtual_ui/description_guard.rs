use super::{visible_tool, VirtualMcpScope};
use serde_json::Value;

/// Resolve only the referenced tool's current description; its own dependency is not traversed.
pub(super) fn description_tool<'a>(
    catalog: &'a domain::McpCatalogSnapshot,
    scope: &VirtualMcpScope,
    tool: &'a domain::McpToolRecord,
    allow_assistant_client: bool,
) -> Option<&'a domain::McpToolRecord> {
    match domain::mcp_management::mcp_description_tool_id(&tool.input_mapping) {
        Some(tool_id) => {
            visible_tool(catalog, scope, tool_id, allow_assistant_client).map(|(_, tool)| tool)
        }
        None => Some(tool),
    }
}

pub(super) fn validate_tool_call_controls(
    arguments: &Value,
    tool: &domain::McpToolRecord,
    description: Option<&domain::McpToolRecord>,
) -> Result<(), &'static str> {
    for name in ["des_id", "max_inline_chars", "response_fields"] {
        if arguments.get(name).is_some()
            && !domain::mcp_management::mcp_call_parameter_is_open(&tool.input_mapping, name)
        {
            let hidden = tool
                .input_mapping
                .get("mappings")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .any(|entry| {
                    entry.pointer("/source/kind").and_then(Value::as_str) == Some("mcp_call")
                        && entry.get("interface_param").and_then(Value::as_str) == Some(name)
                        && domain::mcp_management::input_defaults::mapping_is_hidden(entry)
                });
            if !hidden {
                return Err("Call parameter not open for this tool");
            }
        }
    }
    let has_reference = tool
        .input_mapping
        .get("mappings")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .any(|mapping| mapping.pointer("/source/tool_id").is_some());
    let required = domain::mcp_management::mcp_des_id_required(&tool.input_mapping);
    if required || has_reference {
        domain::mcp_management::validate_mcp_call_parameters(&tool.input_mapping)
            .map_err(|_| "Invalid description configuration")?;
    }
    if required {
        let description = description.ok_or("Description tool not visible")?;
        let des_id = arguments
            .get("des_id")
            .and_then(Value::as_str)
            .ok_or("Missing des_id")?;
        if des_id != description.des_id.as_str() {
            return Err("Invalid des_id");
        }
    } else {
        let resolved = domain::mcp_management::input_defaults::resolve_call_defaults(
            arguments,
            &tool.input_mapping,
        );
        if resolved
            .get("des_id")
            .is_some_and(|value| !value.is_string())
        {
            return Err("Invalid des_id");
        }
    }
    Ok(())
}
