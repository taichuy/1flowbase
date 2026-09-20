//! Responses history equivalence v2, owned by the protocol mapping layer.
//!
//! Source: openai/codex 7498521d, protocol/src/models.rs ResponseItem and
//! core/src/client.rs prepare_response_items_for_request. The public typed item
//! and its extension bag form the canonical representation. This is deliberately
//! narrower than Codex's provider-conditional removal of internal metadata:
//! result records, opaque data and unrecognized extensions remain evidence.
use anyhow::Result;
use serde::Deserialize;
use serde_json::{json, Map, Value};

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ItemKind {
    Message,
    FunctionCall,
    CustomToolCall,
    FunctionCallOutput,
    CustomToolCallOutput,
    Reasoning,
    #[serde(alias = "compaction_summary")]
    Compaction,
    ContextCompaction,
    #[serde(other)]
    Extension,
}

// Typed, optional tracking fields. Flatten keeps every unrecognized field;
// a malformed known field is preserved, never treated as harmless tracking.
#[derive(Deserialize)]
struct DeliveryTracking {
    #[serde(default)]
    turn_id: Option<String>,
    #[serde(flatten)]
    extensions: Map<String, Value>,
}
#[derive(Deserialize)]
struct InternalTracking {
    #[serde(default)]
    turn_id: Option<String>,
    #[serde(default)]
    create_time: Option<serde_json::Number>,
    #[serde(flatten)]
    extensions: Map<String, Value>,
}

fn project_tracking(object: &mut Map<String, Value>, delivery_envelope: bool) {
    if let Some(value) = object.get("metadata").filter(|_| delivery_envelope) {
        if let Ok(DeliveryTracking {
            turn_id,
            extensions,
        }) = serde_json::from_value(value.clone())
        {
            // Incident evidence establishes this field as delivery turn tracking,
            // not tool arguments. Do not generalize to nested/result metadata.
            let _ = turn_id;
            replace_extensions(object, "metadata", extensions);
        }
    }
    if let Some(value) = object.get("internal_chat_message_metadata_passthrough") {
        if let Ok(InternalTracking {
            turn_id,
            create_time,
            extensions,
        }) = serde_json::from_value(value.clone())
        {
            let _ = (turn_id, create_time);
            // Includes executed_tool_calls/tool_result_metadata, cell binding,
            // completeness, content kinds and any future fields unchanged.
            replace_extensions(
                object,
                "internal_chat_message_metadata_passthrough",
                extensions,
            );
        }
    }
}

fn replace_extensions(object: &mut Map<String, Value>, name: &str, extensions: Map<String, Value>) {
    if extensions.is_empty() {
        object.remove(name);
    } else {
        object.insert(name.into(), Value::Object(extensions));
    }
}

fn optional_fields(object: &mut Map<String, Value>, fields: &[&str]) {
    for field in fields {
        if object.get(*field).is_some_and(Value::is_null) {
            object.remove(*field);
        }
    }
}

pub(super) fn normalize_item(item: &Value) -> Result<Value> {
    let mut item = item.clone();
    let object = item
        .as_object_mut()
        .ok_or_else(|| anyhow::anyhow!("native_history_item_invalid"))?;
    if !object.contains_key("type") && object.contains_key("role") {
        object.insert("type".into(), json!("message"));
    }
    let kind: ItemKind = serde_json::from_value(Value::Object(object.clone()))
        .map_err(|_| anyhow::anyhow!("native_history_item_invalid"))?;
    if matches!(kind, ItemKind::Extension) {
        return Ok(item);
    }

    // Optional item IDs are delivery identities; never apply this to call_id.
    if object.get("id").is_some_and(|id| {
        id.is_null()
            || id.as_str().is_some_and(|id| {
                !id.split_once('_')
                    .is_some_and(|(prefix, suffix)| !prefix.is_empty() && !suffix.is_empty())
            })
    }) {
        object.remove("id");
    }
    project_tracking(
        object,
        !matches!(
            kind,
            ItemKind::FunctionCallOutput | ItemKind::CustomToolCallOutput
        ),
    );
    match kind {
        ItemKind::Message => {
            optional_fields(object, &["phase"]);
            completed_delivery_status(object);
            if let Some(Value::String(text)) = object.get("content") {
                let kind = if object.get("role").and_then(Value::as_str) == Some("assistant") {
                    "output_text"
                } else {
                    "input_text"
                };
                object.insert("content".into(), json!([{"type":kind,"text":text}]));
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
        ItemKind::FunctionCall => {
            optional_fields(object, &["namespace", "encrypted_function_args"]);
            completed_delivery_status(object);
        }
        ItemKind::CustomToolCall => optional_fields(object, &["namespace", "status"]),
        ItemKind::FunctionCallOutput => optional_fields(object, &["call_id", "name", "namespace"]),
        ItemKind::CustomToolCallOutput => optional_fields(object, &["name"]),
        ItemKind::Reasoning => {
            optional_fields(object, &["content", "encrypted_content"]);
            // Codex's should_serialize_reasoning_content omits an empty list.
            // Do not copy its broader omission of non-ReasoningText content:
            // opaque or future nonempty parts remain semantic evidence here.
            if object
                .get("content")
                .is_some_and(|v| v.as_array().is_some_and(Vec::is_empty))
            {
                object.remove("content");
            }
        }
        ItemKind::Compaction => {
            object.insert("type".into(), json!("compaction"));
        }
        ItemKind::ContextCompaction => optional_fields(object, &["encrypted_content"]),
        ItemKind::Extension => unreachable!(),
    }
    Ok(item)
}

fn completed_delivery_status(object: &mut Map<String, Value>) {
    if object.get("status").and_then(Value::as_str) == Some("completed") {
        object.remove("status");
    }
}
