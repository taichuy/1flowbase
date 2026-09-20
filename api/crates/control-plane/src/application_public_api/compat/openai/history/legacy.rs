// Frozen version 1 canonicalization. Never reinterpret a stored digest with v2.
use anyhow::Result;
use serde_json::{json, Value};

/// Narrow wire equivalences from Codex ResponseItem and prepare_response_items_for_request.
/// Never discard opaque reasoning, compaction, arguments, phase, tool metadata or unknown fields.
pub(super) fn normalize_item(item: &Value) -> Result<Value> {
    let mut item = item.clone();
    let object = item
        .as_object_mut()
        .ok_or_else(|| anyhow::anyhow!("native_history_item_invalid"))?;
    if !object.contains_key("type") && object.contains_key("role") {
        object.insert("type".into(), json!("message"));
    }
    let kind = object
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    // Codex drops legacy unprefixed item IDs; call_id is always retained.
    if object.get("id").is_some_and(|id| {
        id.is_null()
            || id.as_str().is_some_and(|id| {
                !id.split_once('_')
                    .is_some_and(|(a, b)| !a.is_empty() && !b.is_empty())
            })
    }) {
        object.remove("id");
    }
    // These two ResponseItem variants do not carry the response delivery status.
    if matches!(kind.as_str(), "message" | "function_call")
        && object.get("status").and_then(Value::as_str) == Some("completed")
    {
        object.remove("status");
    }
    if kind == "message" {
        if let Some(Value::String(text)) = object.get("content") {
            let content_type = if object.get("role").and_then(Value::as_str) == Some("assistant") {
                "output_text"
            } else {
                "input_text"
            };
            let content = json!([{"type":content_type,"text":text}]);
            object.insert("content".into(), content);
        }
        if let Some(parts) = object.get_mut("content").and_then(Value::as_array_mut) {
            for part in parts {
                if let Some(part) = part.as_object_mut() {
                    if part.get("type").and_then(Value::as_str) == Some("output_text") {
                        for field in ["annotations", "logprobs"] {
                            if part
                                .get(field)
                                .is_some_and(|v| v.as_array().is_some_and(Vec::is_empty))
                            {
                                part.remove(field);
                            }
                        }
                    }
                }
            }
        }
    }
    Ok(item)
}
