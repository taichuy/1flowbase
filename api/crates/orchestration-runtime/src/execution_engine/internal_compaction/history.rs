use super::*;
use std::collections::BTreeSet;

/// A user-role tool result is a receipt, not a new user request. Only its
/// independent text or a top-level text block can introduce a new request;
/// text nested inside tool_result.content remains tool output.
fn is_user_request(message: &ProviderMessage) -> bool {
    if message.role != ProviderMessageRole::User {
        return false;
    }
    let blocks = message.content_blocks.as_ref().and_then(Value::as_array);
    !message.content.trim().is_empty()
        || blocks.is_some_and(|blocks| {
            blocks.iter().any(|block| {
                block.get("type").and_then(Value::as_str) == Some("text")
                    && block
                        .get("text")
                        .and_then(Value::as_str)
                        .is_some_and(|text| !text.trim().is_empty())
            })
        })
        || !blocks.is_some_and(|blocks| {
            blocks
                .iter()
                .any(|block| block.get("type").and_then(Value::as_str) == Some("tool_result"))
        })
}

/// Shared by selection and checkpoint validation. A mixed request/tool-result
/// message retains its entire causal group, including parallel calls/results.
/// `covered_count` must be a closed cut supplied by the caller.
pub(super) fn required_retained_refs<'a>(
    messages: &'a [ProviderMessage],
    covered_count: usize,
    cuts: &[usize],
) -> impl Iterator<Item = usize> + 'a {
    let protected_group = messages
        .iter()
        .rposition(is_user_request)
        .filter(|index| *index < covered_count)
        .map(|index| {
            let start = cuts
                .iter()
                .copied()
                .take_while(|cut| *cut <= index)
                .last()
                .unwrap_or(0);
            let end = cuts
                .iter()
                .copied()
                .find(|cut| *cut > index)
                .unwrap_or(messages.len());
            start..end
        });
    messages[..covered_count]
        .iter()
        .enumerate()
        .filter(move |(index, message)| {
            message.role == ProviderMessageRole::System
                || protected_group
                    .as_ref()
                    .is_some_and(|range| range.contains(index))
        })
        .map(|(index, _)| index)
}

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
