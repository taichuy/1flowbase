//! Direct exposure of opted-in file-accepting interface wrappers.
//! Ordinary tools keep the four meta-tool discovery contract.
use serde_json::{json, Value};

use super::{
    input_schema, meta_tools, visible_tool, VirtualMcpScope, MCP_CALL, MCP_GET, MCP_LIST,
    MCP_RESULT,
};

/// The ordinary catalog stays behind mcp_list/get/call. Only explicitly opted-in
/// file-consuming wrappers are projected as direct tools because ChatGPT file
/// parameters must be top-level MCP tool inputs.
pub(crate) fn advertised_tools(
    catalog: &domain::McpCatalogSnapshot,
    scope: &VirtualMcpScope,
    path_regex_enabled: bool,
) -> Vec<Value> {
    let mut tools = meta_tools(path_regex_enabled).to_vec();
    tools.extend(catalog.tools.iter().filter_map(|candidate| {
        let (_, tool) = visible_tool(catalog, scope, &candidate.tool_id, false)?;
        if tool.id != candidate.id
            || !matches!(
                tool.execution_target,
                domain::McpToolExecutionTarget::InterfaceWrapper { .. }
            )
        {
            return None;
        }
        let (file_inputs, schema) = openai_file_inputs(tool)?;
        Some(json!({
            "name": tool.tool_id,
            "title": tool.name,
            "description": tool.full_description,
            "inputSchema": schema,
            "_meta": {"openai/fileParams": file_inputs}
        }))
    }));
    tools
}

pub(crate) fn is_direct_file_tool(
    catalog: &domain::McpCatalogSnapshot,
    scope: &VirtualMcpScope,
    name: &str,
) -> bool {
    if matches!(name, MCP_LIST | MCP_GET | MCP_RESULT | MCP_CALL) {
        return false;
    }
    visible_tool(catalog, scope, name, false).is_some_and(|(_, tool)| {
        matches!(
            tool.execution_target,
            domain::McpToolExecutionTarget::InterfaceWrapper { .. }
        ) && openai_file_inputs(tool).is_some()
    })
}

fn openai_file_inputs(tool: &domain::McpToolRecord) -> Option<(Vec<String>, Value)> {
    let names = tool.input_mapping.get("openai_file_params")?.as_array()?;
    if names.is_empty() {
        return None;
    }
    let mut schema = input_schema::mapped_schema(&tool.parameter_schema, &tool.input_mapping);
    let properties = schema.get_mut("properties")?.as_object_mut()?;
    let mut file_inputs = Vec::with_capacity(names.len());
    for name in names {
        let name = name.as_str()?;
        if name.is_empty() || name.contains('.') || file_inputs.iter().any(|n| n == name) {
            return None;
        }
        let field = properties.get_mut(name)?;
        let file_object = if field.get("type").and_then(Value::as_str) == Some("array") {
            field.get_mut("items")?
        } else {
            field
        };
        let required = file_object.get("required")?.as_array()?;
        if required.len() != 2
            || !["download_url", "file_id"].iter().all(|required_name| {
                required
                    .iter()
                    .any(|value| value.as_str() == Some(*required_name))
            })
        {
            return None;
        }
        let object_properties = file_object.get_mut("properties")?.as_object_mut()?;
        for key in ["download_url", "file_id"] {
            if object_properties.get(key)?.get("type")?.as_str() != Some("string") {
                return None;
            }
        }
        // An optional Rust Option<String> may compile to a nullable JSON Schema.
        // The ChatGPT file schema requires optional *non-null* string properties.
        // Normalize only this explicitly opted-in provider projection: a missing
        // optional field remains valid for the canonical Rust DTO.
        for key in ["mime_type", "file_name"] {
            let field = object_properties.get_mut(key)?;
            if field.get("type").and_then(Value::as_str) != Some("string") {
                if !is_nullable_string(field) {
                    return None;
                }
                *field = json!({"type":"string"});
            }
        }
        file_inputs.push(name.to_owned());
    }
    let valid_name = tool.tool_id.len() <= 64
        && !tool.tool_id.is_empty()
        && tool
            .tool_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-');
    valid_name.then_some((file_inputs, schema))
}

fn is_nullable_string(field: &Value) -> bool {
    let variants = field
        .get("anyOf")
        .and_then(Value::as_array)
        .or_else(|| field.get("type").and_then(Value::as_array));
    let Some(variants) = variants else {
        return false;
    };
    variants.len() == 2
        && variants.iter().any(|value| {
            value.get("type").and_then(Value::as_str) == Some("string")
                || value.as_str() == Some("string")
        })
        && variants.iter().any(|value| {
            value.get("type").and_then(Value::as_str) == Some("null")
                || value.as_str() == Some("null")
        })
}
