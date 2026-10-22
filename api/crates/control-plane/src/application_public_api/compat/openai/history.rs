//! Ordered proof of the complete Responses history. Only current-format hashes are persisted.
use anyhow::{ensure, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    io::{self, Write},
};

#[path = "history/canonical.rs"]
mod canonical;
#[cfg(feature = "responses-history-migration")]
#[path = "history/migration.rs"]
pub mod migration;

// Storage contract identity, not an algorithm selector. Keep the existing domain bytes.
const CURRENT_VERSION: u32 = 3;

fn normalize_item(item: &Value) -> Result<Value> {
    canonical::normalize_item(item)
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct History {
    version: u32,
    item_count: usize,
    digest: String,
}

impl History {
    fn empty() -> Self {
        Self {
            version: CURRENT_VERSION,
            item_count: 0,
            digest: format!(
                "{:x}",
                base16ct::HexDisplay(&Sha256::digest(b"1flowbase.responses.history.v3"))
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
            history.version == CURRENT_VERSION,
            "native_history_version_unsupported"
        );
        Ok(history)
    }

    fn append(&mut self, item: &Value) -> Result<()> {
        let item = normalize_item(item)?;
        let next_count = self
            .item_count
            .checked_add(1)
            .ok_or_else(|| anyhow::anyhow!("native_history_item_count_overflow"))?;
        self.digest = append_digest(
            &self.digest,
            b"1flowbase.responses.history.item.v3\0",
            &item,
        )?;
        self.item_count = next_count;
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
    match body.get("input") {
        Some(Value::Array(items)) => {
            for item in items {
                history.append(item)?;
            }
        }
        Some(Value::String(text)) => {
            history.append(&json!({"type":"message","role":"user","content":[{"type":"input_text","text":text}]}))?;
        }
        _ => return Ok(None),
    }
    for item in output {
        history.append(item)?;
    }
    Ok(Some(serde_json::to_value(history)?))
}

/// References into the unchanged wire body, after the proven predecessor prefix.
/// Context may occur before, between or after the owned outputs.
pub(crate) struct FullContextInput<'a> {
    pub(crate) tool_outputs: Vec<&'a Value>,
    pub(crate) context: Vec<&'a Value>,
    pub(crate) ordered: &'a [Value],
}

impl FullContextInput<'_> {
    pub(crate) fn continuation(&self) -> domain::orchestration::ResponsesContinuation {
        domain::orchestration::ResponsesContinuation {
            ordered_input: self.ordered.to_vec(),
            ordered_messages: Vec::new(),
        }
    }
}

/// Proves ordered predecessor history and the complete callback-output set.
/// Output contents remain bound to the durable callback receipt.
pub(crate) fn prove_full_context_input<'a>(
    input: &'a Value,
    trusted_history: &Value,
    owned_call_ids: &[String],
) -> Result<FullContextInput<'a>> {
    let expected = History::parse(trusted_history)?;
    let items = input
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("native_history_item_count_mismatch"))?;
    let _minimum_item_count = expected
        .item_count
        .checked_add(owned_call_ids.len())
        .filter(|end| expected.item_count > 0 && *end <= items.len())
        .ok_or_else(|| anyhow::anyhow!("native_history_item_count_mismatch"))?;
    let mut actual = History::empty();
    for item in &items[..expected.item_count] {
        actual.append(item)?;
    }
    ensure!(actual.digest == expected.digest, "native_history_mismatch");
    let expected_ids: BTreeSet<&str> = owned_call_ids.iter().map(String::as_str).collect();
    ensure!(
        expected_ids.len() == owned_call_ids.len() && !expected_ids.is_empty(),
        "native_history_tool_outputs_invalid"
    );
    let mut proof = FullContextInput {
        tool_outputs: Vec::with_capacity(owned_call_ids.len()),
        context: Vec::new(),
        ordered: &items[expected.item_count..],
    };
    for item in &items[expected.item_count..] {
        match item.get("type").and_then(Value::as_str) {
            Some("function_call_output" | "custom_tool_call_output")
                if !super::is_standalone_function_output(item) =>
            {
                let call_id = item
                    .get("call_id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| anyhow::anyhow!("native_history_tool_outputs_invalid"))?;
                ensure!(
                    expected_ids.contains(call_id) && item.get("output").is_some(),
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
    let results = super::responses_index::native_tool_results(proof.tool_outputs.iter().copied())
        .map_err(|_| anyhow::anyhow!("native_history_tool_outputs_invalid"))?;
    ensure!(
        results.len() == expected_ids.len(),
        "native_history_tool_outputs_invalid"
    );
    Ok(proof)
}

// Shared encoding only: callers supply already normalized semantic evidence.
fn append_digest(previous: &str, domain: &[u8], item: &Value) -> Result<String> {
    let mut byte_count = ByteCounter(0);
    write_canonical_json(item, &mut byte_count)?;
    let mut hash = Sha256::new();
    hash.update(domain);
    hash.update(previous.as_bytes());
    hash.update((byte_count.0 as u64).to_be_bytes());
    write_canonical_json(item, &mut DigestWriter(&mut hash))?;
    Ok(format!("{:x}", base16ct::HexDisplay(&hash.finalize())))
}

struct ByteCounter(usize);

impl Write for ByteCounter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0 = self
            .0
            .checked_add(bytes.len())
            .ok_or_else(|| io::Error::other("native_history_item_size_overflow"))?;
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

struct DigestWriter<'a>(&'a mut Sha256);

impl Write for DigestWriter<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.update(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn write_canonical_json(value: &Value, out: &mut impl Write) -> Result<()> {
    match value {
        Value::Object(object) => {
            out.write_all(b"{")?;
            let mut keys = object.keys().collect::<Vec<_>>();
            keys.sort_unstable();
            for (index, key) in keys.into_iter().enumerate() {
                if index > 0 {
                    out.write_all(b",")?;
                }
                serde_json::to_writer(&mut *out, key)?;
                out.write_all(b":")?;
                write_canonical_json(&object[key], out)?;
            }
            out.write_all(b"}")?;
        }
        Value::Array(items) => {
            out.write_all(b"[")?;
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    out.write_all(b",")?;
                }
                write_canonical_json(item, out)?;
            }
            out.write_all(b"]")?;
        }
        _ => serde_json::to_writer(out, value)?,
    }
    Ok(())
}

#[cfg(test)]
#[path = "../../../_tests/compat/openai_history_tests.rs"]
mod tests;

/// Extend a trusted prefix with the exact public output or verified ordered delta.
pub(crate) fn append_items(prefix: &Value, items: &[Value]) -> Result<Value> {
    let mut history = History::parse(prefix)?;
    for item in items {
        history.append(item)?;
    }
    Ok(serde_json::to_value(history)?)
}
