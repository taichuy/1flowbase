//! ChatGPT file inputs for the four-meta-tool MCP protocol.
//! The gateway owns file argument delivery; target interfaces still own upload/storage.

use serde_json::{json, Value};

use super::input_schema;

/// A file input must be a top-level argument of the tool ChatGPT actually sees.
/// It remains optional so existing mcp_call clients need no changes.
pub(super) fn file_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "download_url": {"type": "string"},
            "file_id": {"type": "string"},
            "mime_type": {"type": "string"},
            "file_name": {"type": "string"}
        },
        "required": ["download_url", "file_id"],
        "additionalProperties": false
    })
}

/// Merge the optional outer mcp_call.file into the nested business-tool arguments.
/// No URL fetching, file bytes, or storage decisions occur at the MCP adapter.
pub(super) fn merge_call_file(
    tool: &domain::McpToolRecord,
    call: &Value,
) -> Result<Option<Value>, &'static str> {
    let Some(file) = call.get("file") else {
        return Ok(None);
    };
    if !valid_file_value(file) {
        return Err("Invalid file parameter");
    }
    let target_field = target_file_field(tool).ok_or("Target tool does not accept file input")?;
    let mut merged = call.clone();
    let arguments = merged
        .get_mut("arguments")
        .and_then(Value::as_object_mut)
        .ok_or("Invalid arguments")?;
    match arguments.get(target_field) {
        Some(existing) if existing == file => {}
        Some(_) => return Err("Conflicting file arguments"),
        None => {
            arguments.insert(target_field.to_owned(), file.clone());
        }
    }
    merged
        .as_object_mut()
        .expect("call is an object")
        .remove("file");
    Ok(Some(merged))
}

fn valid_file_value(file: &Value) -> bool {
    let Some(fields) = file.as_object() else {
        return false;
    };
    ["download_url", "file_id"].iter().all(|key| {
        fields
            .get(*key)
            .and_then(Value::as_str)
            .is_some_and(|s| !s.is_empty())
    }) && fields.iter().all(|(key, value)| {
        ["download_url", "file_id", "mime_type", "file_name"].contains(&key.as_str())
            && value.is_string()
    })
}

/// Only an explicitly opted-in wrapper with one file-shaped mapped input may
/// receive the file. Existing non-file tools continue using their old arguments.
fn target_file_field(tool: &domain::McpToolRecord) -> Option<&str> {
    if !matches!(
        tool.execution_target,
        domain::McpToolExecutionTarget::InterfaceWrapper { .. }
    ) {
        return None;
    }
    let file_names = tool.input_mapping.get("openai_file_params")?.as_array()?;
    let [name] = file_names.as_slice() else {
        return None;
    };
    let name = name.as_str()?;
    if name.is_empty() || name.contains('.') {
        return None;
    }
    let mapped = input_schema::mapped_schema(&tool.parameter_schema, &tool.input_mapping);
    let field = mapped.get("properties")?.get(name)?;
    if field.get("type")?.as_str() != Some("object") {
        return None;
    }
    let fields = field.get("properties")?.as_object()?;
    let required = field.get("required")?.as_array()?;
    if required.len() != 2
        || !["download_url", "file_id"].iter().all(|key| {
            required
                .iter()
                .any(|required| required.as_str() == Some(key))
                && fields
                    .get(*key)
                    .and_then(|field| field.get("type"))
                    .and_then(Value::as_str)
                    == Some("string")
        })
        || !["mime_type", "file_name"]
            .iter()
            .all(|key| fields.get(*key).is_some_and(optional_string))
    {
        return None;
    }
    Some(name)
}

fn optional_string(field: &Value) -> bool {
    if field.get("type").and_then(Value::as_str) == Some("string") {
        return true;
    }
    let variants = field
        .get("anyOf")
        .and_then(Value::as_array)
        .or_else(|| field.get("type").and_then(Value::as_array));
    let Some(variants) = variants else {
        return false;
    };
    variants.len() == 2
        && ["string", "null"].iter().all(|kind| {
            variants.iter().any(|value| {
                value.get("type").and_then(Value::as_str) == Some(kind)
                    || value.as_str() == Some(kind)
            })
        })
}
