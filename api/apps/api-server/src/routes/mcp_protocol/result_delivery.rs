use control_plane::ports::{
    McpOperationOutcome, McpResultReceiptRepository, RecordMcpResultReceiptInput,
    EPHEMERAL_VALUE_MAX_BYTES,
};
use domain::ActorContext;
use serde_json::{json, Value};
use std::sync::Arc;
use storage_durable_postgres::MainDurableStore;
use time::Duration;
use uuid::Uuid;

use crate::host_infrastructure::CacheStore;

#[path = "result_delivery/selection.rs"]
mod selection;
pub(crate) use selection::ResultSelection;

#[path = "result_delivery/paging.rs"]
mod paging;
#[cfg(test)]
use paging::page_leaves;
use paging::{bounded_page, json_leaves, serialized, JsonLeaf};
pub(crate) use paging::{inline_limit, ContinuationCursor, DEFAULT_INLINE_CHARS};

#[cfg(test)]
#[path = "result_delivery/_tests/cache.rs"]
mod cache_tests;

/// Storage dependencies for MCP result continuation and receipt delivery.
///
/// The result-delivery core intentionally has no dependency on API state or
/// route composition so runtime execution can retain only the resources it
/// needs to persist and page a tool result.
#[derive(Clone)]
pub(crate) struct McpResultDeliveryDependencies {
    store: MainDurableStore,
    cache_store: Arc<dyn CacheStore>,
}

impl McpResultDeliveryDependencies {
    pub(crate) fn new(store: MainDurableStore, cache_store: Arc<dyn CacheStore>) -> Self {
        Self { store, cache_store }
    }
}

#[cfg(test)]
pub(crate) const MAX_INLINE_CHARS: usize = 16_000;
pub(crate) const DETAIL_TTL_SECONDS: i64 = 10 * 60;

const CACHE_KEY_PREFIX: &str = "mcp-result";
// The chunk admission check is against the actual JSON value written to the
// CacheStore. Raw serialized-detail bytes are not a safe proxy because the
// chunk itself is stored as a JSON string and may add escaping overhead.
const DETAIL_CHUNK_MAX_BYTES: usize = EPHEMERAL_VALUE_MAX_BYTES;
// Preserve the existing total continuation ceiling while allowing each stored
// JSON string chunk to use the full CacheStore value budget.
const MAX_DETAIL_TOTAL_BYTES: usize = MAX_DETAIL_CHUNKS * (EPHEMERAL_VALUE_MAX_BYTES / 2);
const MAX_DETAIL_CHUNKS: usize = 16;

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

pub(crate) fn exceeds_inline_limit(detail: &Value, inline_chars: usize) -> bool {
    contains_base64_like(detail, None) || serialized(detail).chars().count() > inline_chars
}

pub(crate) fn tool_inline_limit(
    arguments: &Value,
    tool: &domain::McpToolRecord,
) -> Result<usize, &'static str> {
    if arguments.get("max_inline_chars").is_some() {
        return inline_limit(arguments);
    }
    tool.max_inline_chars
        .map(|value| {
            usize::try_from(value)
                .ok()
                .filter(|value| *value > 0)
                .ok_or("Invalid tool max_inline_chars")
        })
        .unwrap_or(Ok(DEFAULT_INLINE_CHARS))
}

pub(crate) async fn deliver_result(
    dependencies: &McpResultDeliveryDependencies,
    actor: &ActorContext,
    operation: CompletedOperation<'_>,
    detail: Value,
    inline_chars: usize,
    selection: &ResultSelection,
    tool: &domain::McpToolRecord,
) -> Value {
    if selection.is_default() && !exceeds_inline_limit(&detail, inline_chars) {
        return tool_result(detail);
    }
    let defaults = json!({
        "max_inline_chars": tool.max_inline_chars.unwrap_or(DEFAULT_INLINE_CHARS as i64),
        "response_fields": tool.response_fields
    });
    let mut delivered = deliver_cached_result(
        dependencies,
        actor,
        operation,
        detail.clone(),
        Some(&defaults),
    )
    .await;
    let compact = delivered
        .get_mut("structuredContent")
        .expect("delivery has structured content");
    let result_ref = compact
        .pointer("/detail/result_ref")
        .and_then(Value::as_str)
        .and_then(|value| Uuid::parse_str(value).ok());
    let Some(result_ref) = result_ref else {
        if selection.is_default() {
            return delivered;
        }
        let leaves = match selection.leaves(&detail) {
            Ok(leaves) => leaves,
            Err(error) => {
                compact["selection_error"] = error;
                return tool_result(compact.clone());
            }
        };
        let mut page = bounded_page(
            &leaves,
            ContinuationCursor::default(),
            inline_chars,
            compact.clone(),
        );
        page["next_cursor"] = Value::Null;
        return tool_result(page);
    };
    if selection.is_default() && serialized(compact).chars().count() <= inline_chars {
        return tool_result(compact.clone());
    }
    if !save_selection(
        dependencies,
        actor.current_workspace_id,
        result_ref,
        selection,
    )
    .await
    {
        compact["detail"] =
            json!({"status": "detail_unavailable", "reason": "selection_cache_unavailable"});
        return tool_result(compact.clone());
    }
    let leaves = match selection.leaves(&detail) {
        Ok(leaves) => leaves,
        Err(error) => {
            compact["selection_error"] = error;
            return tool_result(compact.clone());
        }
    };
    let cursor = ContinuationCursor {
        selection_id: selection.identity(),
        ..Default::default()
    };
    compact["detail"]["next_cursor"] = Value::Null;
    tool_result(bounded_page(&leaves, cursor, inline_chars, compact.clone()))
}

fn selection_key(workspace_id: Uuid, result_ref: Uuid, selection_id: u64) -> String {
    format!("{CACHE_KEY_PREFIX}-selection:{workspace_id}:{result_ref}:{selection_id:016x}")
}

async fn save_selection(
    dependencies: &McpResultDeliveryDependencies,
    workspace_id: Uuid,
    result_ref: Uuid,
    selection: &ResultSelection,
) -> bool {
    let Some(selection_id) = selection.identity() else {
        return true;
    };
    matches!(
        store_cache_value(
            dependencies,
            workspace_id,
            result_ref,
            &selection_key(workspace_id, result_ref, selection_id),
            serde_json::to_value(selection).expect("selection serializes")
        )
        .await,
        DetailCacheStatus::Available
    )
}

#[cfg(test)]
pub(crate) async fn deliver_oversized_result(
    dependencies: &McpResultDeliveryDependencies,
    actor: &ActorContext,
    operation: CompletedOperation<'_>,
    detail: Value,
) -> Value {
    deliver_cached_result(dependencies, actor, operation, detail, None).await
}

async fn deliver_cached_result(
    dependencies: &McpResultDeliveryDependencies,
    actor: &ActorContext,
    operation: CompletedOperation<'_>,
    detail: Value,
    return_defaults: Option<&Value>,
) -> Value {
    let result_ref = Uuid::now_v7();
    let summary = compact_summary(operation.operation_id_ref(), &detail);
    let cache_status = cache_detail(
        dependencies,
        actor.current_workspace_id,
        result_ref,
        &detail,
        return_defaults,
    )
    .await;

    let receipt = if operation.is_write() {
        match dependencies
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

pub(crate) async fn read_result(
    dependencies: &McpResultDeliveryDependencies,
    actor: &ActorContext,
    result_ref: Uuid,
    arguments: &Value,
) -> Value {
    let cached = dependencies
        .cache_store
        .get_json(&cache_key(actor.current_workspace_id, result_ref))
        .await
        .ok()
        .flatten();
    let policy = cached
        .as_ref()
        .and_then(|manifest| manifest.get("return_defaults"))
        .cloned()
        .unwrap_or_else(|| json!({}));
    let inline_chars = if arguments.get("max_inline_chars").is_some() {
        inline_limit(arguments)
    } else {
        inline_limit(&policy)
    };
    let inline_chars = match inline_chars {
        Ok(value) => value,
        Err(reason) => {
            return tool_result(
                json!({"detail_status": "invalid_request", "reason": reason, "retry_original": false}),
            )
        }
    };
    let requested_cursor = match arguments.get("cursor") {
        None => None,
        Some(Value::String(value)) => match ContinuationCursor::parse(value) {
            Some(cursor) => Some(cursor),
            None => {
                return tool_result(
                    json!({"detail_status": "invalid_cursor", "retry_original": false}),
                )
            }
        },
        _ => {
            return tool_result(json!({"detail_status": "invalid_cursor", "retry_original": false}))
        }
    };
    let default_fields = policy
        .get("response_fields")
        .filter(|value| !value.is_null())
        .and_then(|value| serde_json::from_value::<Vec<String>>(value.clone()).ok());
    let has_selection =
        arguments.get("response_fields").is_some() || arguments.get("string_ranges").is_some();
    let selection = if !has_selection
        && requested_cursor
            .and_then(|cursor| cursor.selection_id)
            .is_some()
    {
        let selection_id = requested_cursor
            .and_then(|cursor| cursor.selection_id)
            .expect("selection cursor");
        dependencies
            .cache_store
            .get_json(&selection_key(
                actor.current_workspace_id,
                result_ref,
                selection_id,
            ))
            .await
            .ok()
            .flatten()
            .and_then(|value| serde_json::from_value::<ResultSelection>(value).ok())
            .ok_or("Selection expired or unavailable")
    } else {
        ResultSelection::parse(arguments, default_fields.as_deref())
    };
    let selection = match selection {
        Ok(selection) => selection,
        Err(reason) => {
            return tool_result(
                json!({"detail_status": "invalid_selection", "reason": reason, "retry_original": false}),
            )
        }
    };
    let cursor = requested_cursor.unwrap_or(ContinuationCursor {
        selection_id: selection.identity(),
        ..Default::default()
    });
    if cursor.selection_id != selection.identity() {
        return tool_result(
            json!({"detail_status": "invalid_cursor", "reason": "cursor_selection_mismatch", "retry_original": false}),
        );
    }
    let receipt = dependencies
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
    let detail = match cached {
        Some(manifest) => {
            resolve_cached_detail(
                dependencies,
                actor.current_workspace_id,
                result_ref,
                manifest,
            )
            .await
        }
        None => None,
    };

    let Some(detail) = detail else {
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

    let leaves = match selection.leaves(&detail) {
        Ok(leaves) => leaves,
        Err(error) => {
            return tool_result(
                json!({"result_ref": result_ref, "detail_status": "invalid_selection", "error": error, "retry_original": false}),
            )
        }
    };
    if !save_selection(
        dependencies,
        actor.current_workspace_id,
        result_ref,
        &selection,
    )
    .await
    {
        return tool_result(
            json!({"result_ref": result_ref, "detail_status": "detail_unavailable", "reason": "selection_cache_unavailable", "retry_original": false}),
        );
    }
    if !cursor.is_valid_for(&leaves) {
        return tool_result(json!({
            "result_ref": result_ref,
            "detail_status": "invalid_cursor",
            "retry_original": false
        }));
    }
    let mut page = json!({
        "result_ref": result_ref,
        "detail_status": "available",
        "retry_original": false
    });
    if let Some(receipt) = receipt {
        page["receipt"] = receipt_projection(&receipt);
    }
    tool_result(bounded_page(&leaves, cursor, inline_chars, page))
}

pub(crate) fn tool_result(value: Value) -> Value {
    let text = serialized(&value);
    json!({
        "content": [{"type":"text","text":text}],
        "structuredContent": value,
        "isError": false
    })
}

#[cfg(test)]
pub(crate) async fn read_continuation(
    dependencies: &McpResultDeliveryDependencies,
    actor: &ActorContext,
    result_ref: Uuid,
    cursor: ContinuationCursor,
    inline_chars: usize,
) -> Value {
    read_result(
        dependencies,
        actor,
        result_ref,
        &json!({
            "cursor": cursor.encode(), "max_inline_chars": inline_chars
        }),
    )
    .await
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
    dependencies: &McpResultDeliveryDependencies,
    workspace_id: Uuid,
    result_ref: Uuid,
    detail: &Value,
    return_defaults: Option<&Value>,
) -> DetailCacheStatus {
    if contains_base64_like(detail, None) {
        return DetailCacheStatus::Unavailable("binary_or_base64_content");
    }
    let serialized_detail = match serde_json::to_string(detail) {
        Ok(serialized_detail) => serialized_detail,
        Err(_) => return DetailCacheStatus::Unavailable("serialization_failed"),
    };
    let mut inline_value = inline_cache_value(detail);
    if let Some(defaults) = return_defaults {
        inline_value["return_defaults"] = defaults.clone();
    }
    if serialized(&inline_value).len() <= DETAIL_CHUNK_MAX_BYTES {
        return store_cache_value(
            dependencies,
            workspace_id,
            result_ref,
            &cache_key(workspace_id, result_ref),
            inline_value,
        )
        .await;
    }
    if serialized_detail.len() > MAX_DETAIL_TOTAL_BYTES {
        return DetailCacheStatus::Unavailable("cache_capacity_exceeded");
    }

    let Some(chunks) = split_at_json_string_bytes(&serialized_detail, DETAIL_CHUNK_MAX_BYTES)
    else {
        return DetailCacheStatus::Unavailable("cache_capacity_exceeded");
    };
    if chunks.len() > MAX_DETAIL_CHUNKS {
        return DetailCacheStatus::Unavailable("cache_capacity_exceeded");
    }
    for (index, chunk) in chunks.iter().enumerate() {
        let status = store_cache_value(
            dependencies,
            workspace_id,
            result_ref,
            &chunk_cache_key(workspace_id, result_ref, index),
            Value::String(chunk.clone()),
        )
        .await;
        if let DetailCacheStatus::Unavailable(reason) = status {
            return DetailCacheStatus::Unavailable(reason);
        }
    }
    // The manifest is written last so a reader never observes it before all
    // chunks are in place.
    let mut manifest = json!({ "format": "chunked", "chunk_count": chunks.len() });
    if let Some(defaults) = return_defaults {
        manifest["return_defaults"] = defaults.clone();
    }
    store_cache_value(
        dependencies,
        workspace_id,
        result_ref,
        &cache_key(workspace_id, result_ref),
        manifest,
    )
    .await
}

fn inline_cache_value(detail: &Value) -> Value {
    json!({ "format": "inline", "detail": detail })
}

async fn store_cache_value(
    dependencies: &McpResultDeliveryDependencies,
    workspace_id: Uuid,
    result_ref: Uuid,
    key: &str,
    value: Value,
) -> DetailCacheStatus {
    match dependencies
        .cache_store
        .set_json(key, value, Some(Duration::seconds(DETAIL_TTL_SECONDS)))
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

/// Reassembles a cached detail from its manifest; `None` means the cached
/// entry is missing, incomplete, or corrupt, and callers should treat it the
/// same as a cache miss.
async fn resolve_cached_detail(
    dependencies: &McpResultDeliveryDependencies,
    workspace_id: Uuid,
    result_ref: Uuid,
    manifest: Value,
) -> Option<Value> {
    match manifest.get("format").and_then(Value::as_str) {
        Some("inline") => manifest.get("detail").cloned(),
        Some("chunked") => {
            let chunk_count = manifest
                .get("chunk_count")
                .and_then(Value::as_u64)
                .and_then(|count| usize::try_from(count).ok())
                .filter(|count| (1..=MAX_DETAIL_CHUNKS).contains(count))?;
            let mut serialized_detail = String::new();
            for index in 0..chunk_count {
                let chunk = dependencies
                    .cache_store
                    .get_json(&chunk_cache_key(workspace_id, result_ref, index))
                    .await
                    .ok()
                    .flatten()?;
                serialized_detail.push_str(chunk.as_str()?);
            }
            serde_json::from_str(&serialized_detail).ok()
        }
        _ => None,
    }
}

fn split_at_json_string_bytes(value: &str, max_bytes: usize) -> Option<Vec<String>> {
    let mut chunks = Vec::new();
    let mut current = String::new();
    let mut current_json_bytes = 2;
    for character in value.chars() {
        let character_json_bytes = serde_json::to_vec(&Value::String(character.to_string()))
            .expect("serializing a JSON string character must succeed")
            .len()
            - 2;
        if 2 + character_json_bytes > max_bytes {
            return None;
        }
        if current_json_bytes + character_json_bytes > max_bytes && !current.is_empty() {
            chunks.push(std::mem::take(&mut current));
            current_json_bytes = 2;
        }
        current.push(character);
        current_json_bytes += character_json_bytes;
    }
    if !current.is_empty() {
        chunks.push(current);
    }
    Some(chunks)
}

fn cache_key(workspace_id: Uuid, result_ref: Uuid) -> String {
    format!("{CACHE_KEY_PREFIX}:{workspace_id}:{result_ref}")
}

fn chunk_cache_key(workspace_id: Uuid, result_ref: Uuid, index: usize) -> String {
    format!("{CACHE_KEY_PREFIX}:{workspace_id}:{result_ref}:chunk:{index}")
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

#[cfg(test)]
mod assistant_mcp_tests {
    use super::*;

    #[test]
    fn split_at_char_boundaries_respects_utf8_and_reassembles() {
        let value = format!("{}界{}", "a".repeat(9), "b".repeat(9));
        let chunks = split_at_json_string_bytes(&value, 10).unwrap();
        assert!(chunks.iter().all(|chunk| {
            serde_json::to_vec(&Value::String(chunk.clone()))
                .unwrap()
                .len()
                <= 10
        }));
        assert!(chunks.len() >= 2);
        assert_eq!(chunks.concat(), value);
    }

    #[test]
    fn split_at_json_string_bytes_accounts_for_escape_expansion() {
        let value = "\\\"\n\r\t\u{0000}界".repeat(20);
        let chunks = split_at_json_string_bytes(&value, 32).unwrap();

        assert!(chunks.iter().all(|chunk| {
            serde_json::to_vec(&Value::String(chunk.clone()))
                .unwrap()
                .len()
                <= 32
        }));
        assert_eq!(chunks.concat(), value);
    }

    #[test]
    fn assistant_mcp_small_result_remains_inline() {
        assert!(!exceeds_inline_limit(
            &json!({"items": [1, 2, 3]}),
            DEFAULT_INLINE_CHARS,
        ));
    }

    #[test]
    fn inline_cache_admission_counts_the_complete_envelope_at_the_exact_boundary() {
        let empty = serialized(&inline_cache_value(&json!(""))).len();
        let exact_detail = json!("a".repeat(DETAIL_CHUNK_MAX_BYTES - empty));
        let over_detail = json!("a".repeat(DETAIL_CHUNK_MAX_BYTES - empty + 1));

        assert_eq!(
            serialized(&inline_cache_value(&exact_detail)).len(),
            DETAIL_CHUNK_MAX_BYTES
        );
        assert!(serialized(&inline_cache_value(&over_detail)).len() > DETAIL_CHUNK_MAX_BYTES);
    }

    #[test]
    fn inline_cache_admission_counts_utf8_and_escape_expansion() {
        let detail = json!("界\\\"\n".repeat((DETAIL_CHUNK_MAX_BYTES - 12) / 9));
        let serialized_size = serialized(&inline_cache_value(&detail)).len();

        assert!(serialized_size > DETAIL_CHUNK_MAX_BYTES);
        let raw_detail_size = serialized(&detail).len();
        assert!(raw_detail_size <= DETAIL_CHUNK_MAX_BYTES);
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
                selection_id: None,
            })
        );
        assert_eq!(
            ContinuationCursor::parse("v2:3:12000"),
            Some(ContinuationCursor {
                leaf_index: 3,
                char_offset: 12_000,
                selection_id: None,
            })
        );
        assert_eq!(ContinuationCursor::parse("v2:bad:cursor"), None);
    }
}
