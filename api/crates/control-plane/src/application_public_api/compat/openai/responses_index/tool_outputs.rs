use std::collections::BTreeMap;

use serde_json::{json, Value};

use crate::errors::ControlPlaneError;

/// Project Responses outputs into the inbox's one receipt per call. Codex exec
/// notifications are additional custom outputs, not additional callback claims.
/// The provider still receives the untouched wire body; this projection binds all
/// parts (including their order and item boundaries) to the durable fingerprint.
pub(crate) fn native_tool_results<'a>(
    outputs: impl IntoIterator<Item = &'a Value>,
) -> anyhow::Result<Vec<Value>> {
    let mut positions = BTreeMap::new();
    let mut groups: Vec<Vec<(usize, &Value)>> = Vec::new();
    for (position, output) in outputs.into_iter().enumerate() {
        let id = output["call_id"]
            .as_str()
            .filter(|id| !id.is_empty())
            .ok_or(ControlPlaneError::InvalidInput(
                "native_tool_output_call_id",
            ))?;
        output.get("output").ok_or(ControlPlaneError::InvalidInput(
            "native_tool_output_content",
        ))?;
        let group = *positions.entry(id).or_insert_with(|| {
            groups.push(Vec::new());
            groups.len() - 1
        });
        if let Some((_, first)) = groups[group].first() {
            if first["type"] != "custom_tool_call_output"
                || output["type"] != "custom_tool_call_output"
            {
                return Err(ControlPlaneError::Conflict("native_tool_output_duplicate").into());
            }
        }
        groups[group].push((position, output));
    }
    Ok(groups
        .into_iter()
        .map(|parts| {
            let first = parts[0].1;
            if parts.len() == 1 {
                // Keep existing single-output receipt identities unchanged.
                return json!({"tool_call_id":first["call_id"],"content":first["output"]});
            }
            json!({
                "tool_call_id":first["call_id"],
                "content":parts.iter().flat_map(|(_, item)| match &item["output"] {
                    Value::Array(blocks) => blocks.clone(),
                    Value::String(text) => vec![json!({"type":"input_text","text":text})],
                    other => vec![json!({"type":"input_text","text":other.to_string()})],
                }).collect::<Vec<_>>(),
                "responses_output_items":parts.iter().map(|(position, item)| {
                    json!({"output_index":position,"item":item})
                }).collect::<Vec<_>>()
            })
        })
        .collect())
}
