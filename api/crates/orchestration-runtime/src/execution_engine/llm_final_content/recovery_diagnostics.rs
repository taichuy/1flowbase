//! Bounded error projection, not recovery policy. Provider-specific recognition
//! remains in the provider; this boundary admits only safe diagnostic facts.
use extension_contracts::provider_contract::{
    recovery_receipt_from_details, ProviderRuntimeErrorKind, PROVIDER_RECOVERY_RECEIPT_METADATA_KEY,
};
use serde_json::{json, Value};
const KEY: &str = "1flowbase_provider_recovery_diagnostics";

pub(super) fn attach(payload: &mut Value, details: Option<&Value>) {
    let Some(details) = details else { return };
    if let Ok(Some(receipt)) = recovery_receipt_from_details(details) {
        payload[PROVIDER_RECOVERY_RECEIPT_METADATA_KEY] = json!(receipt);
    }
    let Some(source) = details.get(KEY).and_then(Value::as_object) else {
        return;
    };
    let mut result = json!({});
    for field in ["first_failure", "last_failure"] {
        if let Some(value) = source.get(field).and_then(failure) {
            result[field] = value;
        }
    }
    result["attempts"] = Value::Array(
        source
            .get("attempts")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .take(16)
            .filter_map(failure)
            .collect(),
    );
    if let Some(valid) = source.get("association_valid").and_then(Value::as_bool) {
        result["association_valid"] = json!(valid);
    }
    payload[KEY] = result;
}

fn failure(source: &Value) -> Option<Value> {
    let source = source.as_object()?;
    let mut result = json!({});
    // Closed diagnostics carry no arbitrary payload strings. This allowlist
    // validates projection data; it never selects an action or parses an error.
    for (field, allowed) in [
        (
            "websocket_error_kind",
            &[
                "connection_closed",
                "already_closed",
                "io",
                "tls",
                "capacity",
                "protocol",
                "write_buffer_full",
                "utf8",
                "attack_attempt",
                "url",
                "http",
                "http_format",
                "queue_count_limit",
                "queue_bytes_limit",
                "write_timeout",
                "owner_cancelled",
                "owner_stopped",
            ][..],
        ),
        ("failure_phase", &["idle", "active", "closing"][..]),
        (
            "semantic_event_kind",
            &[
                "text_delta",
                "reasoning_delta",
                "tool_call_delta",
                "tool_call_commit",
                "raw_responses_delta",
                "message_added",
                "message_done",
                "reasoning_added",
                "reasoning_done",
                "tool_item_added",
                "tool_item_done",
                "unknown_item",
                "other",
            ][..],
        ),
        (
            "recovery_decision",
            &[
                "retry_websocket",
                "retry_http",
                "terminal",
                "committed",
                "budget_exhausted",
                "deadline_exceeded",
            ][..],
        ),
        (
            "io_error_kind",
            &[
                "connection_refused",
                "connection_reset",
                "connection_aborted",
                "not_connected",
                "broken_pipe",
                "timed_out",
                "unexpected_eof",
                "permission_denied",
                "interrupted",
                "other",
            ][..],
        ),
        (
            "kind",
            &[
                "websocket_close",
                "websocket_error",
                "network_error",
                "provider_typed",
                "provider_untyped",
                "owner_rejected",
                "attempt_admission",
            ][..],
        ),
        (
            "reason_category",
            &[
                "continuation_unavailable",
                "previous_response_unavailable",
                "proxy_failed",
                "policy_rejected",
                "transport_disconnected",
                "unclassified",
                "deadline_exceeded",
                "budget_exhausted",
                "authorization_rejected",
            ][..],
        ),
        (
            "reason",
            &[
                "upstream continuation connection is unavailable",
                "previous_response_id is no longer available",
                "upstream websocket proxy failed",
                "upstream policy rejected the request; reason redacted",
                "websocket disconnected before response.completed",
                "provider failure; unclassified details redacted",
                "provider recovery deadline exceeded",
                "provider recovery attempt budget exhausted",
                "upstream authorization rejected the request",
            ][..],
        ),
    ] {
        if let Some(value) = source
            .get(field)
            .and_then(Value::as_str)
            .filter(|value| allowed.contains(value))
        {
            result[field] = json!(value);
        }
    }
    if let Some(value) = source
        .get("semantic_event_type_digest")
        .and_then(Value::as_str)
        .filter(|value| {
            value.strip_prefix("sha256:").is_some_and(|digest| {
                digest.len() == 64
                    && digest
                        .bytes()
                        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            })
        })
    {
        result["semantic_event_type_digest"] = json!(value);
    }
    for field in ["routing_token_present", "association_present"] {
        if let Some(value) = source.get(field).and_then(Value::as_bool) {
            result[field] = json!(value);
        }
    }
    if let Some(kind) = source
        .get("provider_error_kind")
        .and_then(|value| serde_json::from_value::<ProviderRuntimeErrorKind>(value.clone()).ok())
    {
        result["provider_error_kind"] = json!(kind);
    }
    for field in [
        "socket_incarnation",
        "owner_socket_incarnation",
        "transport_generation",
    ] {
        if let Some(value) = source
            .get(field)
            .and_then(Value::as_u64)
            .filter(|value| *value > 0)
        {
            result[field] = json!(value);
        }
    }
    for (field, max) in [
        ("idle_duration_ms", 86_400_000),
        ("close_code", 4999),
        ("attempt", 15),
        ("consumed_attempts", 16),
        ("buffered_scaffold_events", 1024),
        ("buffered_scaffold_bytes", 16 * 1024 * 1024),
    ] {
        if let Some(value) = source
            .get(field)
            .and_then(Value::as_u64)
            .filter(|value| *value <= max)
        {
            result[field] = json!(value);
        }
    }
    Some(result)
}

#[cfg(test)]
#[path = "_tests/recovery_diagnostics.rs"]
mod tests;
