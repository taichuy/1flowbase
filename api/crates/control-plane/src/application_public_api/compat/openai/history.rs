//! Versioned, ordered proof of the complete Responses history. Only hashes are persisted.
use anyhow::{ensure, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct History {
    version: u32,
    item_count: usize,
    digest: String,
}

impl History {
    fn empty() -> Self {
        Self {
            version: 1,
            item_count: 0,
            digest: format!("{:x}", Sha256::digest(b"1flowbase.responses.history.v1")),
        }
    }

    fn parse(value: &Value) -> Result<Self> {
        ensure!(!value.is_null(), "native_history_evidence_missing");
        let history: Self = serde_json::from_value(value.clone())
            .map_err(|_| anyhow::anyhow!("native_history_evidence_invalid"))?;
        ensure!(
            history.version == 1
                && history.digest.len() == 64
                && history.digest.bytes().all(|c| c.is_ascii_hexdigit()),
            "native_history_evidence_invalid"
        );
        Ok(history)
    }

    fn append(&mut self, item: &Value) -> Result<()> {
        let item = normalize_item(item)?;
        let bytes = serde_json::to_vec(&canonical_json(&item))?;
        let mut hash = Sha256::new();
        hash.update(b"1flowbase.responses.history.item.v1\0");
        hash.update(self.digest.as_bytes());
        hash.update((bytes.len() as u64).to_be_bytes());
        hash.update(bytes);
        self.digest = format!("{:x}", hash.finalize());
        self.item_count += 1;
        Ok(())
    }
}

/// Build only from the host-owned request and completed native output. An incremental
/// request without trusted predecessor evidence cannot establish complete history.
/// The caller must establish successful completion, including a successful empty prewarm.
pub(crate) fn completed_history(
    body: &Value,
    predecessor: Option<&Value>,
    output: &[Value],
) -> Result<Option<Value>> {
    let mut history = if body
        .get("previous_response_id")
        .is_some_and(|id| !id.is_null())
    {
        let Some(predecessor) = predecessor else {
            return Ok(None);
        };
        History::parse(predecessor)?
    } else {
        History::empty()
    };
    let input = match body.get("input") {
        Some(Value::Array(items)) => items.clone(),
        Some(Value::String(text)) => vec![
            json!({"type":"message","role":"user","content":[{"type":"input_text","text":text}]}),
        ],
        _ => return Ok(None),
    };
    for item in input.iter().chain(output) {
        history.append(item)?;
    }
    Ok(Some(serde_json::to_value(history)?))
}

/// A full retry is precisely the proven history followed by this callback's outputs.
/// Ownership and equality to the accepted callback output payload remain the callback owner.
pub(crate) fn validate_full_retry_input(
    input: &Value,
    trusted_history: &Value,
    owned_call_ids: &[String],
) -> Result<()> {
    let expected = History::parse(trusted_history)?;
    let items = input
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("native_history_incomplete"))?;
    ensure!(
        expected.item_count > 0
            && expected.item_count.checked_add(owned_call_ids.len()) == Some(items.len()),
        "native_history_incomplete"
    );
    let mut actual = History::empty();
    for item in &items[..expected.item_count] {
        actual.append(item)?;
    }
    ensure!(actual.digest == expected.digest, "native_history_mismatch");
    let mut remaining: BTreeSet<&str> = owned_call_ids.iter().map(String::as_str).collect();
    ensure!(
        remaining.len() == owned_call_ids.len() && !remaining.is_empty(),
        "native_history_tool_outputs_invalid"
    );
    for item in &items[expected.item_count..] {
        ensure!(
            matches!(
                item.get("type").and_then(Value::as_str),
                Some("function_call_output" | "custom_tool_call_output")
            ),
            "native_history_tool_outputs_invalid"
        );
        let call_id = item
            .get("call_id")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("native_history_tool_outputs_invalid"))?;
        ensure!(
            remaining.remove(call_id) && item.get("output").is_some(),
            "native_history_tool_outputs_invalid"
        );
    }
    ensure!(remaining.is_empty(), "native_history_tool_outputs_invalid");
    Ok(())
}

fn canonical_json(value: &Value) -> Value {
    match value {
        Value::Object(object) => Value::Object(
            object
                .iter()
                .map(|(k, v)| (k.clone(), canonical_json(v)))
                .collect::<std::collections::BTreeMap<_, _>>()
                .into_iter()
                .collect(),
        ),
        Value::Array(items) => Value::Array(items.iter().map(canonical_json).collect()),
        value => value.clone(),
    }
}

/// Narrow wire equivalences from Codex ResponseItem and prepare_response_items_for_request.
/// Never discard opaque reasoning, compaction, arguments, phase, tool metadata or unknown fields.
fn normalize_item(item: &Value) -> Result<Value> {
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

#[cfg(test)]
#[path = "../../../_tests/compat/openai_history_tests.rs"]
mod tests;
