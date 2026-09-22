//! Versioned, ordered proof of the complete Responses history. Only hashes are persisted.
use anyhow::{ensure, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

#[path = "history/canonical.rs"]
mod canonical;
#[path = "history/legacy.rs"]
mod legacy;

const CURRENT_VERSION: u32 = 2;

fn normalize_item(item: &Value) -> Result<Value> {
    canonical::normalize_item(item)
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct History {
    version: u32,
    item_count: usize,
    digest: String,
}

impl History {
    fn empty(version: u32) -> Self {
        Self {
            version,
            item_count: 0,
            digest: format!(
                "{:x}",
                Sha256::digest(format!("1flowbase.responses.history.v{version}"))
            ),
        }
    }

    fn parse(value: &Value) -> Result<Self> {
        ensure!(!value.is_null(), "native_history_evidence_missing");
        let history: Self = serde_json::from_value(value.clone())
            .map_err(|_| anyhow::anyhow!("native_history_evidence_invalid"))?;
        ensure!(
            history.digest.len() == 64 && history.digest.bytes().all(|c| c.is_ascii_hexdigit()),
            "native_history_evidence_invalid"
        );
        ensure!(
            matches!(history.version, 1 | CURRENT_VERSION),
            "native_history_version_unsupported"
        );
        Ok(history)
    }

    fn append(&mut self, item: &Value) -> Result<()> {
        let item = match self.version {
            1 => legacy::normalize_item(item)?,
            CURRENT_VERSION => normalize_item(item)?,
            _ => anyhow::bail!("native_history_version_unsupported"),
        };
        let bytes = serde_json::to_vec(&canonical_json(&item))?;
        let mut hash = Sha256::new();
        hash.update(format!(
            "1flowbase.responses.history.item.v{}\0",
            self.version
        ));
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
        History::empty(CURRENT_VERSION)
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
    let proof = prove_full_context_input(input, trusted_history, owned_call_ids)?;
    ensure!(
        proof.context.is_empty(),
        "native_history_item_count_mismatch"
    );
    Ok(())
}

/// References into the unchanged wire body, after the proven predecessor prefix.
/// Context may occur before, between or after the owned outputs.
pub(crate) struct FullContextInput<'a> {
    pub(crate) tool_outputs: Vec<&'a Value>,
    pub(crate) context: Vec<&'a Value>,
}

/// Proves ordered predecessor history and the complete callback-output set.
/// Output contents remain bound to the durable callback receipt.
pub(crate) fn prove_full_context_input<'a>(
    input: &'a Value,
    trusted_history: &Value,
    owned_call_ids: &[String],
) -> Result<FullContextInput<'a>> {
    let expected = History::parse(trusted_history)?;
    // A v1 hash cannot attest the v2 client equivalence contract. Preserve old
    // continuation proofs as v1, but require a fresh trusted round for recovery.
    ensure!(
        expected.version == CURRENT_VERSION,
        "native_history_version_unsupported"
    );
    let items = input
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("native_history_item_count_mismatch"))?;
    let _minimum_item_count = expected
        .item_count
        .checked_add(owned_call_ids.len())
        .filter(|end| expected.item_count > 0 && *end <= items.len())
        .ok_or_else(|| anyhow::anyhow!("native_history_item_count_mismatch"))?;
    let mut actual = History::empty(CURRENT_VERSION);
    for item in &items[..expected.item_count] {
        actual.append(item)?;
    }
    ensure!(actual.digest == expected.digest, "native_history_mismatch");
    let mut remaining: BTreeSet<&str> = owned_call_ids.iter().map(String::as_str).collect();
    ensure!(
        remaining.len() == owned_call_ids.len() && !remaining.is_empty(),
        "native_history_tool_outputs_invalid"
    );
    let mut proof = FullContextInput {
        tool_outputs: Vec::with_capacity(owned_call_ids.len()),
        context: Vec::new(),
    };
    for item in &items[expected.item_count..] {
        match item.get("type").and_then(Value::as_str) {
            Some("function_call_output" | "custom_tool_call_output") => {
                let call_id = item
                    .get("call_id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| anyhow::anyhow!("native_history_tool_outputs_invalid"))?;
                ensure!(
                    remaining.remove(call_id) && item.get("output").is_some(),
                    "native_history_tool_outputs_invalid"
                );
                proof.tool_outputs.push(item);
            }
            // A later tool round has its own correlation owner, never this receipt.
            Some("function_call" | "custom_tool_call") => {
                anyhow::bail!("native_history_tool_outputs_invalid");
            }
            _ => proof.context.push(item),
        }
    }
    ensure!(remaining.is_empty(), "native_history_tool_outputs_invalid");
    Ok(proof)
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

#[cfg(test)]
#[path = "../../../_tests/compat/openai_history_tests.rs"]
mod tests;
