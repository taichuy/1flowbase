use control_plane::ports::{
    McpOperationOutcome, McpResultReceiptRepository, RecordMcpResultReceiptInput,
    EPHEMERAL_VALUE_MAX_BYTES,
};
use domain::ActorContext;
use serde_json::{json, Value};
use time::Duration;
use uuid::Uuid;

use crate::app_state::ApiState;

pub(crate) const DEFAULT_INLINE_CHARS: usize = 4_000;
pub(crate) const MAX_INLINE_CHARS: usize = 16_000;
pub(crate) const DETAIL_TTL_SECONDS: i64 = 10 * 60;

const CACHE_KEY_PREFIX: &str = "mcp-result";
const PAGE_ENVELOPE_RESERVE_CHARS: usize = 768;

#[derive(Clone, Copy)]
pub(crate) enum CompletedOperation<'a> {
    Read { operation_id: &'a str },
    Write { operation_id: &'a str },
}

impl<'a> CompletedOperation<'a> {
    fn operation_id_ref(self) -> &'a str {
        match self {
            Self::Read { operation_id } | Self::Write { operation_id } => operation_id,
        }
    }

    fn is_write(self) -> bool {
        matches!(self, Self::Write { .. })
    }
}

pub(crate) fn inline_limit(arguments: &Value) -> Result<usize, &'static str> {
    let Some(value) = arguments.get("max_inline_chars") else {
        return Ok(DEFAULT_INLINE_CHARS);
    };
    let Some(value) = value.as_u64().and_then(|value| usize::try_from(value).ok()) else {
        return Err("Invalid max_inline_chars");
    };
    if value == 0 || value > MAX_INLINE_CHARS {
        return Err("Invalid max_inline_chars");
    }
    Ok(value)
}

pub(crate) fn exceeds_inline_limit(detail: &Value, inline_chars: usize) -> bool {
    contains_base64_like(detail, None) || serialized(detail).chars().count() > inline_chars
}

pub(crate) async fn deliver_oversized_result(
    state: &ApiState,
    actor: &ActorContext,
    operation: CompletedOperation<'_>,
    detail: Value,
) -> Value {
    let result_ref = Uuid::now_v7();
    let summary = compact_summary(operation.operation_id_ref(), &detail);
    let cache_status = cache_detail(state, actor.current_workspace_id, result_ref, &detail).await;

    let receipt = if operation.is_write() {
        match state
            .store
            .record_mcp_result_receipt(&RecordMcpResultReceiptInput {
                receipt_id: result_ref,
                workspace_id: actor.current_workspace_id,
                actor_user_id: actor.user_id,
                operation_id: operation.operation_id_ref().to_string(),
                outcome: McpOperationOutcome::Succeeded,
                summary: summary.clone(),
            })
            .await
        {
            Ok(receipt) => Some((Some(receipt.receipt_id), "available")),
            Err(error) => {
                tracing::error!(
                    operation_id = operation.operation_id_ref(),
                    workspace_id = %actor.current_workspace_id,
                    error = %error,
                    "failed to persist a completed MCP operation receipt"
                );
                Some((None, "unavailable"))
            }
        }
    } else {
        None
    };

    let detail_delivery = match cache_status {
        DetailCacheStatus::Available => json!({
            "status": "continuation_available",
            "result_ref": result_ref,
            "next_cursor": ContinuationCursor::default().encode(),
            "expires_in_seconds": DETAIL_TTL_SECONDS
        }),
        DetailCacheStatus::Unavailable(reason) => json!({
            "status": "detail_unavailable",
            "reason": reason
        }),
    };
    let mut compact = json!({
        "outcome": "succeeded",
        "operation_id": operation.operation_id_ref(),
        "summary": summary,
        "detail": detail_delivery,
        "retry_original": false
    });
    if let Some((receipt_id, receipt_status)) = receipt {
        compact["receipt_status"] = json!(receipt_status);
        if let Some(receipt_id) = receipt_id {
            compact["receipt_id"] = json!(receipt_id);
        }
    }
    tool_result(compact)
}

pub(crate) async fn read_continuation(
    state: &ApiState,
    actor: &ActorContext,
    result_ref: Uuid,
    cursor: ContinuationCursor,
    inline_chars: usize,
) -> Value {
    let receipt = state
        .store
        .get_mcp_result_receipt(actor.current_workspace_id, result_ref)
        .await
        .map_err(|error| {
            tracing::warn!(
                workspace_id = %actor.current_workspace_id,
                result_ref = %result_ref,
                error = %error,
                "failed to read MCP result receipt"
            );
            error
        })
        .ok()
        .flatten();
    let cached = state
        .infrastructure
        .cache_store()
        .get_json(&cache_key(actor.current_workspace_id, result_ref))
        .await
        .map_err(|error| {
            tracing::warn!(
                workspace_id = %actor.current_workspace_id,
                result_ref = %result_ref,
                error = %error,
                "failed to read cached MCP result detail"
            );
            error
        })
        .ok()
        .flatten();

    let Some(detail) = cached else {
        let mut unavailable = json!({
            "result_ref": result_ref,
            "detail_status": "detail_unavailable",
            "retry_original": false
        });
        if let Some(receipt) = receipt {
            unavailable["receipt"] = receipt_projection(&receipt);
        }
        return tool_result(unavailable);
    };

    let leaves = json_leaves(&detail);
    if !cursor.is_valid_for(&leaves) {
        return tool_result(json!({
            "result_ref": result_ref,
            "detail_status": "invalid_cursor",
            "retry_original": false
        }));
    }
    let Some((entries, next_cursor)) = page_leaves(&leaves, cursor, inline_chars) else {
        return tool_result(json!({
            "result_ref": result_ref,
            "detail_status": "page_budget_too_small",
            "max_inline_chars": MAX_INLINE_CHARS,
            "retry_original": false
        }));
    };

    let mut page = json!({
        "result_ref": result_ref,
        "detail_status": "available",
        "entries": entries,
        "next_cursor": next_cursor.map(ContinuationCursor::encode),
        "retry_original": false
    });
    if let Some(receipt) = receipt {
        page["receipt"] = receipt_projection(&receipt);
    }
    tool_result(page)
}

pub(crate) fn tool_result(value: Value) -> Value {
    let text = serialized(&value);
    json!({
        "content": [{"type":"text","text":text}],
        "structuredContent": value,
        "isError": false
    })
}

fn receipt_projection(receipt: &control_plane::ports::McpResultReceipt) -> Value {
    json!({
        "receipt_id": receipt.receipt_id,
        "operation_id": receipt.operation_id,
        "outcome": receipt.outcome.as_str(),
        "summary": receipt.summary,
        "created_at": receipt.created_at
    })
}

enum DetailCacheStatus {
    Available,
    Unavailable(&'static str),
}

async fn cache_detail(
    state: &ApiState,
    workspace_id: Uuid,
    result_ref: Uuid,
    detail: &Value,
) -> DetailCacheStatus {
    if contains_base64_like(detail, None) {
        return DetailCacheStatus::Unavailable("binary_or_base64_content");
    }
    let bytes = match serde_json::to_vec(detail) {
        Ok(bytes) => bytes,
        Err(_) => return DetailCacheStatus::Unavailable("serialization_failed"),
    };
    if bytes.len() > EPHEMERAL_VALUE_MAX_BYTES {
        return DetailCacheStatus::Unavailable("cache_capacity_exceeded");
    }
    match state
        .infrastructure
        .cache_store()
        .set_json(
            &cache_key(workspace_id, result_ref),
            detail.clone(),
            Some(Duration::seconds(DETAIL_TTL_SECONDS)),
        )
        .await
    {
        Ok(()) => DetailCacheStatus::Available,
        Err(error) => {
            tracing::warn!(
                workspace_id = %workspace_id,
                result_ref = %result_ref,
                error = %error,
                "failed to cache MCP result detail"
            );
            DetailCacheStatus::Unavailable("cache_store_unavailable")
        }
    }
}

fn cache_key(workspace_id: Uuid, result_ref: Uuid) -> String {
    format!("{CACHE_KEY_PREFIX}:{workspace_id}:{result_ref}")
}

fn compact_summary(operation_id: &str, value: &Value) -> Value {
    if operation_id == "import_mcp_bundle_library_release" {
        if let (Some(manifest), Some(effect_summary)) =
            (value.get("manifest"), value.get("effect_summary"))
        {
            return json!({
                "bundle": {
                    "organization": manifest.get("organization"),
                    "bundle_id": manifest.get("bundle_id"),
                    "bundle_version": manifest.get("bundle_version"),
                    "locale": manifest.get("locale")
                },
                "status": value.get("status"),
                "effect_summary": effect_summary
            });
        }
    }
    match value {
        Value::Null => json!({ "value_type": "null" }),
        Value::Bool(_) => json!({ "value_type": "boolean" }),
        Value::Number(_) => json!({ "value_type": "number" }),
        Value::String(value) => {
            json!({ "value_type": "string", "character_count": value.chars().count() })
        }
        Value::Array(values) => {
            json!({ "value_type": "array", "item_count": values.len() })
        }
        Value::Object(values) => {
            json!({ "value_type": "object", "field_count": values.len() })
        }
    }
}

#[derive(Clone)]
struct JsonLeaf {
    path: String,
    value: Value,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct ContinuationCursor {
    leaf_index: usize,
    char_offset: usize,
}

impl ContinuationCursor {
    pub(crate) fn parse(value: &str) -> Option<Self> {
        if let Some(encoded) = value.strip_prefix("v2:") {
            let (leaf_index, char_offset) = encoded.split_once(':')?;
            return Some(Self {
                leaf_index: leaf_index.parse().ok()?,
                char_offset: char_offset.parse().ok()?,
            });
        }
        Some(Self {
            leaf_index: value.parse().ok()?,
            char_offset: 0,
        })
    }

    fn encode(self) -> String {
        if self.char_offset == 0 {
            self.leaf_index.to_string()
        } else {
            format!("v2:{}:{}", self.leaf_index, self.char_offset)
        }
    }

    fn is_valid_for(self, leaves: &[JsonLeaf]) -> bool {
        if self.leaf_index == leaves.len() {
            return self.char_offset == 0;
        }
        let Some(leaf) = leaves.get(self.leaf_index) else {
            return false;
        };
        match &leaf.value {
            Value::String(value) => self.char_offset < value.chars().count(),
            _ => self.char_offset == 0,
        }
    }
}

fn json_leaves(value: &Value) -> Vec<JsonLeaf> {
    let mut leaves = Vec::new();
    collect_json_leaves(value, String::new(), &mut leaves);
    leaves
}

fn collect_json_leaves(value: &Value, path: String, leaves: &mut Vec<JsonLeaf>) {
    match value {
        Value::Object(values) if !values.is_empty() => {
            let mut fields = values.iter().collect::<Vec<_>>();
            fields.sort_by_key(|(field, _)| *field);
            for (field, value) in fields {
                collect_json_leaves(
                    value,
                    format!("{path}/{}", escape_json_pointer(field)),
                    leaves,
                );
            }
        }
        Value::Array(values) if !values.is_empty() => {
            for (index, value) in values.iter().enumerate() {
                collect_json_leaves(value, format!("{path}/{index}"), leaves);
            }
        }
        _ => leaves.push(JsonLeaf {
            path,
            value: value.clone(),
        }),
    }
}

fn escape_json_pointer(segment: &str) -> String {
    segment.replace('~', "~0").replace('/', "~1")
}

fn page_leaves(
    leaves: &[JsonLeaf],
    cursor: ContinuationCursor,
    inline_chars: usize,
) -> Option<(Vec<Value>, Option<ContinuationCursor>)> {
    if cursor.leaf_index == leaves.len() {
        return Some((Vec::new(), None));
    }
    let entry_budget = inline_chars.saturating_sub(PAGE_ENVELOPE_RESERVE_CHARS);
    let mut entries = Vec::new();
    let mut used = 2;
    let mut next = cursor;
    while let Some(leaf) = leaves.get(next.leaf_index) {
        let entry = json!({ "path": leaf.path, "value": leaf.value });
        let entry_chars = serialized(&entry).chars().count() + usize::from(!entries.is_empty());
        if next.char_offset == 0 && used + entry_chars <= entry_budget {
            used += entry_chars;
            entries.push(entry);
            next.leaf_index += 1;
            continue;
        }

        let largest_regular_entry = entry_budget.saturating_sub(2);
        if next.char_offset == 0 && serialized(&entry).chars().count() <= largest_regular_entry {
            break;
        }

        let Value::String(value) = &leaf.value else {
            break;
        };
        let separator_chars = usize::from(!entries.is_empty());
        let available = entry_budget.saturating_sub(used + separator_chars);
        let Some((chunk, chunk_chars)) =
            fitting_string_chunk(leaf, value, next.char_offset, available)
        else {
            break;
        };
        used += serialized(&chunk).chars().count() + separator_chars;
        entries.push(chunk);
        next.char_offset += chunk_chars;
        if next.char_offset == value.chars().count() {
            next.leaf_index += 1;
            next.char_offset = 0;
        }
    }
    if entries.is_empty() {
        return None;
    }
    Some((entries, (next.leaf_index < leaves.len()).then_some(next)))
}

fn fitting_string_chunk(
    leaf: &JsonLeaf,
    value: &str,
    char_offset: usize,
    available_chars: usize,
) -> Option<(Value, usize)> {
    let total_chars = value.chars().count();
    let remaining_chars = total_chars.checked_sub(char_offset)?;
    let mut low = 1;
    let mut high = remaining_chars;
    let mut best = None;
    while low <= high {
        let candidate_chars = low + (high - low) / 2;
        let candidate = string_chunk_entry(leaf, value, char_offset, candidate_chars, total_chars);
        if serialized(&candidate).chars().count() <= available_chars {
            best = Some((candidate, candidate_chars));
            low = candidate_chars + 1;
        } else {
            high = candidate_chars.saturating_sub(1);
        }
    }
    best
}

fn string_chunk_entry(
    leaf: &JsonLeaf,
    value: &str,
    char_offset: usize,
    char_count: usize,
    total_chars: usize,
) -> Value {
    let chunk = value
        .chars()
        .skip(char_offset)
        .take(char_count)
        .collect::<String>();
    json!({
        "path": leaf.path,
        "value_type": "string_chunk",
        "value": chunk,
        "char_offset": char_offset,
        "char_count": char_count,
        "total_chars": total_chars,
        "complete": char_offset + char_count == total_chars
    })
}

fn contains_base64_like(value: &Value, field_name: Option<&str>) -> bool {
    match value {
        Value::String(value) => {
            value.starts_with("data:") && value.contains(";base64,")
                || value.len() >= 64
                    && field_name.is_some_and(|field| {
                        field.to_ascii_lowercase().contains("base64")
                            || field.to_ascii_lowercase().contains("binary")
                    })
                || value.len() >= 1024
                    && value.ends_with('=')
                    && value.bytes().all(|byte| {
                        byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'/' | b'=')
                    })
        }
        Value::Array(values) => values
            .iter()
            .any(|value| contains_base64_like(value, field_name)),
        Value::Object(values) => values
            .iter()
            .any(|(field, value)| contains_base64_like(value, Some(field))),
        _ => false,
    }
}

fn serialized(value: &Value) -> String {
    serde_json::to_string(value).expect("serde_json::Value serialization must be infallible")
}

#[cfg(test)]
mod assistant_mcp_tests {
    use super::*;

    #[test]
    fn assistant_mcp_small_result_remains_inline() {
        assert!(!exceeds_inline_limit(
            &json!({"items": [1, 2, 3]}),
            DEFAULT_INLINE_CHARS,
        ));
    }

    #[test]
    fn assistant_mcp_result_continuation_pages_have_stable_cursors() {
        let detail = json!({
            "alpha": "a".repeat(700),
            "beta": "b".repeat(700),
            "gamma": "c".repeat(700)
        });
        let leaves = json_leaves(&detail);
        let (first, next) = page_leaves(&leaves, ContinuationCursor::default(), 1_600)
            .expect("first page must fit");
        let next = next.expect("detail must require continuation");
        let (second, final_cursor) =
            page_leaves(&leaves, next, MAX_INLINE_CHARS).expect("remaining page must fit");

        assert!(!first.is_empty());
        assert!(!second.is_empty());
        assert_eq!(first.len() + second.len(), leaves.len());
        assert_eq!(final_cursor, None);
    }

    #[test]
    fn issue_1733_ac_001_ac_003_large_unicode_leaf_is_pageable() {
        let source_code = "界".repeat(17_416);
        let detail = json!({"source_code": &source_code});
        let leaves = json_leaves(&detail);

        let mut cursor = ContinuationCursor::default();
        let mut reconstructed = String::new();
        let mut pages = 0;
        loop {
            let (page, next_cursor) = page_leaves(&leaves, cursor, MAX_INLINE_CHARS)
                .expect("a large string leaf must produce a continuation page");
            pages += 1;
            for chunk in &page {
                assert_eq!(chunk["path"], json!("/source_code"));
                assert_eq!(chunk["value_type"], json!("string_chunk"));
                assert_eq!(chunk["char_offset"], json!(reconstructed.chars().count()));
                assert_eq!(chunk["total_chars"], json!(17_416));
                reconstructed.push_str(chunk["value"].as_str().expect("chunk value"));
            }
            let Some(next_cursor) = next_cursor else {
                break;
            };
            assert!(next_cursor.encode().starts_with("v2:"));
            cursor = next_cursor;
        }

        let first_chunk = page_leaves(&leaves, ContinuationCursor::default(), MAX_INLINE_CHARS)
            .expect("first page")
            .0
            .remove(0);

        assert_eq!(first_chunk["path"], json!("/source_code"));
        assert_eq!(first_chunk["value_type"], json!("string_chunk"));
        assert_eq!(first_chunk["char_offset"], json!(0));
        assert_eq!(first_chunk["total_chars"], json!(17_416));
        assert_eq!(first_chunk["complete"], json!(false));
        assert!(first_chunk["value"]
            .as_str()
            .is_some_and(|value| !value.is_empty()));
        assert!(pages > 1);
        assert_eq!(reconstructed, source_code);
    }

    #[test]
    fn issue_1733_ac_002_cursor_accepts_legacy_leaf_index_and_opaque_v2_offset() {
        assert_eq!(
            ContinuationCursor::parse("3"),
            Some(ContinuationCursor {
                leaf_index: 3,
                char_offset: 0,
            })
        );
        assert_eq!(
            ContinuationCursor::parse("v2:3:12000"),
            Some(ContinuationCursor {
                leaf_index: 3,
                char_offset: 12_000,
            })
        );
        assert_eq!(ContinuationCursor::parse("v2:bad:cursor"), None);
    }
}
