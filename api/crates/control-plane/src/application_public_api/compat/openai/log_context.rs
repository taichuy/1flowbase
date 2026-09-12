use control_plane_contracts::ports::{resolve_client_log_identity, ApplicationRunLogContext};
use serde_json::Value;
use std::collections::BTreeMap;

const OPENAI_RESPONSES_LOG_PROTOCOL: &str = "openai_responses";

/// Logging captures an independent, bounded identity projection. It never
/// fills the Native conversation or history that participates in inference.
///
/// Codex transports one canonical turn metadata blob and projects it three
/// ways: flat `client_metadata` keys, `client_metadata["x-codex-turn-metadata"]`
/// and the ingress `x-codex-turn-metadata` header. All projections must agree.
pub(crate) fn capture_application_run_log_context(
    body: &Value,
    ingress_turn_metadata: Option<&Value>,
) -> ApplicationRunLogContext {
    let mut context = ApplicationRunLogContext {
        protocol: Some(OPENAI_RESPONSES_LOG_PROTOCOL.into()),
        ..Default::default()
    };
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
    let ingress = ingress_turn_metadata.unwrap_or(&Value::Null);
    if metadata.is_some() {
        context.identity_sources.push("client_metadata".into());
    }
    if nested.is_object() {
        context
            .identity_sources
            .push("client_metadata.x-codex-turn-metadata".into());
    }
    if ingress.is_object() {
        context
            .identity_sources
            .push("header.x-codex-turn-metadata".into());
    }
    // HTTP sends the header per request, but a WebSocket handshake header is
    // captured once (usually from the prewarm) and replayed for every turn on
    // that connection. Thread-scoped fields must agree everywhere; turn-scoped
    // fields come from the body whenever the body declares them at all.
    let body_declares = |name: &str, nested_name: &str| {
        metadata.and_then(|v| v.get(name)).is_some() || nested.get(nested_name).is_some()
    };
    let field = |name: &str, nested_name: &str, turn_scoped: bool| {
        let declarations = [
            metadata.and_then(|v| v.get(name)),
            nested.get(nested_name),
            ingress
                .get(nested_name)
                .filter(|_| !(turn_scoped && body_declares(name, nested_name))),
        ];
        if declarations.iter().flatten().any(|v| !v.is_string()) {
            return Err("invalid_identity");
        }
        resolve_client_log_identity(&declarations.map(|v| v.and_then(Value::as_str)))
    };
    let parsed = (|| -> Result<(), &'static str> {
        if metadata.is_some_and(|m| !m.is_object()) {
            return Err("invalid_identity");
        }
        context.thread_id = field("thread_id", "thread_id", false)?;
        context.turn_id = field("turn_id", "turn_id", true)?;
        context.session_id = field("session_id", "session_id", false)?;
        context.parent_thread_id = field("x-codex-parent-thread-id", "parent_thread_id", false)?;
        context.parent_turn_id = field("parent_turn_id", "parent_turn_id", true)?;
        context.forked_from_thread_id =
            field("forked_from_thread_id", "forked_from_thread_id", false)?;
        context.root_turn_id = field("root_turn_id", "root_turn_id", true)?;
        context.request_kind = field("request_kind", "request_kind", true)?;
        context.subagent_kind = resolve_client_log_identity(&[
            metadata
                .and_then(|v| v.get("x-openai-subagent"))
                .and_then(Value::as_str)
                .map(subagent_kind_from_header),
            nested.get("subagent_kind").and_then(Value::as_str),
            ingress.get("subagent_kind").and_then(Value::as_str),
        ])?;
        context.previous_response_id = resolve_client_log_identity(&[body
            .get("previous_response_id")
            .and_then(Value::as_str)])?;
        Ok(())
    })();
    if let Err(reason) = parsed {
        return ApplicationRunLogContext {
            identity_status: reason.into(),
            protocol: Some(OPENAI_RESPONSES_LOG_PROTOCOL.into()),
            ..Default::default()
        };
    }
    context.identity_status = identity_status(&context).into();
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

/// Ingress headers are another projection of the same Codex snapshot:
/// `thread-id` and `x-openai-subagent` must agree with the body. The envelope's
/// `session-id` is not consulted: the route rewrites it into a key-scoped
/// reconnect identity, so the client session is read from the body and the
/// `x-codex-turn-metadata` projection only.
pub(crate) fn reconcile_client_log_headers(
    context: &mut ApplicationRunLogContext,
    headers: &BTreeMap<String, Vec<String>>,
) {
    if matches!(
        context.identity_status.as_str(),
        "invalid_identity" | "conflicting_identity"
    ) {
        return;
    }
    let reconciled = (|| -> Result<(), &'static str> {
        if let Some(values) = headers.get("thread-id") {
            context.identity_sources.push("header.thread-id".into());
            let mut declarations = vec![context.thread_id.as_deref()];
            declarations.extend(values.iter().map(|v| Some(v.as_str())));
            context.thread_id = resolve_client_log_identity(&declarations)?;
        }
        if let Some(values) = headers.get("x-openai-subagent") {
            context
                .identity_sources
                .push("header.x-openai-subagent".into());
            let mut declarations = vec![context.subagent_kind.as_deref()];
            declarations.extend(
                values
                    .iter()
                    .map(|v| Some(subagent_kind_from_header(v.as_str()))),
            );
            context.subagent_kind = resolve_client_log_identity(&declarations)?;
        }
        Ok(())
    })();
    match reconciled {
        Ok(()) => context.identity_status = identity_status(context).into(),
        Err(reason) => {
            context.thread_id = None;
            context.turn_id = None;
            context.identity_status = reason.into();
        }
    }
}

fn identity_status(context: &ApplicationRunLogContext) -> &'static str {
    match (&context.thread_id, &context.turn_id) {
        (Some(_), Some(_)) => "identified",
        (Some(_), None) => "unknown_turn",
        _ => "missing_identity",
    }
}

/// Codex sends `SubAgentSource::kind()` inside turn metadata but a separate
/// header vocabulary in `x-openai-subagent`; only the thread spawn label differs.
fn subagent_kind_from_header(value: &str) -> &str {
    match value {
        "collab_spawn" => "thread_spawn",
        other => other,
    }
}
