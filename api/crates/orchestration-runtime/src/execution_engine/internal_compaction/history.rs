use super::*;
use std::collections::BTreeSet;

/// Legal summary units end only when every tool call in a parallel batch has a
/// result. An unfinished tail stays in the final request. Canonical block and
/// legacy field representations may mirror one another, but not duplicate IDs
/// within either representation.
pub fn closed_prefix_ends(messages: &[ProviderMessage]) -> Result<Vec<usize>> {
    let mut pending = BTreeSet::new();
    let mut cuts = Vec::new();
    for (index, message) in messages.iter().enumerate() {
        let mut calls = BTreeSet::new();
        let mut results = BTreeSet::new();
        if message.role == ProviderMessageRole::Tool {
            results.insert(stable_id(message.tool_call_id.as_deref())?);
        }
        if let Some(values) = &message.tool_calls {
            for call in values.as_array().context("tool calls must be an array")? {
                ensure!(
                    calls.insert(stable_id(call.get("id").and_then(Value::as_str))?),
                    "duplicate tool call id"
                );
            }
        }
        let mut block_calls = BTreeSet::new();
        let mut block_results = BTreeSet::new();
        if let Some(blocks) = &message.content_blocks {
            for block in blocks
                .as_array()
                .context("content blocks must be an array")?
            {
                match block.get("type").and_then(Value::as_str) {
                    Some("tool_use") => {
                        ensure!(
                            block_calls.insert(stable_id(block.get("id").and_then(Value::as_str))?),
                            "duplicate tool use id"
                        );
                    }
                    Some("tool_result") => {
                        if let Some(content) = block.get("content").and_then(Value::as_array) {
                            ensure!(
                                content
                                    .iter()
                                    .all(|part| part.get("type").and_then(Value::as_str)
                                        == Some("text")
                                        && part.get("text").is_some_and(Value::is_string)),
                                "tool result contains unsupported nontextual history"
                            );
                        }
                        ensure!(
                            block_results.insert(stable_id(
                                block.get("tool_use_id").and_then(Value::as_str)
                            )?),
                            "duplicate tool result id"
                        );
                    }
                    Some("text") => {}
                    Some("reasoning") if block.get("text").is_some_and(Value::is_string) => {}
                    _ => bail!("internal compaction requires supported complete textual history"),
                }
            }
        }
        calls.extend(block_calls);
        results.extend(block_results);
        if results.is_empty() {
            ensure!(
                pending.is_empty(),
                "unclosed tool batch before next message"
            );
        }
        for id in results {
            ensure!(
                pending.remove(&id),
                "tool result has no preceding pending call"
            );
        }
        if !calls.is_empty() {
            ensure!(
                message.role == ProviderMessageRole::Assistant,
                "tool calls require assistant role"
            );
            ensure!(
                pending.is_empty(),
                "new tool batch before previous batch completed"
            );
            pending.extend(calls);
        }
        if pending.is_empty() {
            cuts.push(index + 1);
        }
    }
    Ok(cuts)
}

fn stable_id(value: Option<&str>) -> Result<String> {
    value
        .filter(|id| !id.is_empty())
        .map(str::to_owned)
        .context("tool call/result has no stable id")
}

use anyhow::Context;
