use control_plane_contracts::gateway_logs::{GatewayLogContext, resolve_gateway_identity};
use serde_json::Value;
use std::collections::BTreeMap;

/// Logging captures an independent, bounded identity projection. It never
/// fills the Native conversation or history that participates in inference.
pub(crate) fn capture_gateway_log_context(body: &Value) -> GatewayLogContext {
    let mut context = GatewayLogContext::default();
    let metadata = body.get("client_metadata");
    let nested = metadata.and_then(|v| v.get("x-codex-turn-metadata"));
    let nested = match nested {
        Some(Value::String(value)) if value.len() <= 262_144 => {
            serde_json::from_str::<Value>(value)
                .ok()
                .filter(Value::is_object)
        }
        None => Some(Value::Null),
        _ => None,
    };
    let Some(nested) = nested else {
        context.identity_status = "invalid_identity".into();
        return context;
    };
    if metadata.is_some() {
        context.identity_sources.push("client_metadata".into());
    }
    if nested.is_object() {
        context
            .identity_sources
            .push("client_metadata.x-codex-turn-metadata".into());
    }
    let field = |name: &str, nested_name: &str| {
        let flat = metadata.and_then(|v| v.get(name));
        let nested = nested.get(nested_name);
        if [flat, nested].iter().flatten().any(|v| !v.is_string()) {
            return Err("invalid_identity");
        }
        resolve_gateway_identity(&[flat.and_then(Value::as_str), nested.and_then(Value::as_str)])
    };
    let parsed = (|| -> Result<(), &'static str> {
        if metadata.is_some_and(|m| !m.is_object()) {
            return Err("invalid_identity");
        }
        context.thread_id = field("thread_id", "thread_id")?;
        context.turn_id = field("turn_id", "turn_id")?;
        context.session_id = field("session_id", "session_id")?;
        context.parent_thread_id = field("x-codex-parent-thread-id", "parent_thread_id")?;
        context.parent_turn_id = field("parent_turn_id", "parent_turn_id")?;
        context.forked_from_thread_id = field("forked_from_thread_id", "forked_from_thread_id")?;
        context.root_turn_id = field("root_turn_id", "root_turn_id")?;
        context.request_kind = field("request_kind", "request_kind")?;
        context.previous_response_id =
            resolve_gateway_identity(&[body.get("previous_response_id").and_then(Value::as_str)])?;
        Ok(())
    })();
    if let Err(reason) = parsed {
        return GatewayLogContext {
            identity_status: reason.into(),
            ..Default::default()
        };
    }
    context.identity_status = match (&context.thread_id, &context.turn_id) {
        (Some(_), Some(_)) => "identified",
        (Some(_), None) => "unknown_turn",
        _ => "missing_identity",
    }
    .into();
    if let Some(input) = body.get("input").and_then(Value::as_array) {
        context.prompt = input
            .iter()
            .rev()
            .find(|item| item.get("role").and_then(Value::as_str) == Some("user"))
            .cloned();
        context.tool_results = input
            .iter()
            .filter(|item| {
                matches!(
                    item.get("type").and_then(Value::as_str),
                    Some("custom_tool_call_output" | "function_call_output")
                )
            })
            .rev()
            .take(128)
            .cloned()
            .collect();
        context.tool_results.reverse();
    } else if let Some(input) = body.get("input").and_then(Value::as_str) {
        context.prompt = Some(serde_json::json!({"role":"user","content":input}));
    }
    // Large input content is omitted explicitly, never injected into model input.
    for item in context
        .prompt
        .iter_mut()
        .chain(context.tool_results.iter_mut())
    {
        if serde_json::to_vec(item).is_ok_and(|v| v.len() > 262_144) {
            *item = serde_json::json!({"type":item.get("type"),"call_id":item.get("call_id"),"content_omitted":true});
        }
    }
    context
}

pub(crate) fn reconcile_gateway_log_headers(
    context: &mut GatewayLogContext,
    headers: &BTreeMap<String, Vec<String>>,
) {
    if matches!(
        context.identity_status.as_str(),
        "invalid_identity" | "conflicting_identity"
    ) {
        return;
    }
    let mut declarations = vec![context.thread_id.as_deref()];
    if let Some(values) = headers.get("thread-id") {
        declarations.extend(values.iter().map(|v| Some(v.as_str())));
        context.identity_sources.push("header.thread-id".into());
    }
    match resolve_gateway_identity(&declarations) {
        Ok(thread) => {
            context.thread_id = thread;
            context.identity_status = match (&context.thread_id, &context.turn_id) {
                (Some(_), Some(_)) => "identified",
                (Some(_), None) => "unknown_turn",
                _ => "missing_identity",
            }
            .into();
        }
        Err(reason) => {
            context.thread_id = None;
            context.turn_id = None;
            context.identity_status = reason.into();
        }
    }
}
