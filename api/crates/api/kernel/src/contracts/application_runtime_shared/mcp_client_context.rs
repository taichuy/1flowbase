//! Client-declared correlation only; authentication and log visibility remain independent.
use std::future::Future;

use axum::http::HeaderMap;
use serde_json::Value;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct McpClientContext {
    pub(crate) thread_id: Option<String>,
    pub(crate) turn_id: Option<String>,
    pub(crate) session_id: Option<String>,
    pub(crate) error: Option<&'static str>,
}

impl McpClientContext {
    pub(super) fn from_protocol(meta: Option<&Value>, headers: &HeaderMap) -> Self {
        match parse(meta, headers) {
            Ok(context) => context,
            Err(error) => Self {
                error: Some(error),
                ..Self::default()
            },
        }
    }
}

fn parse(meta: Option<&Value>, headers: &HeaderMap) -> Result<McpClientContext, &'static str> {
    let mut declarations = Vec::new();
    if let Some(meta) = meta.filter(|value| !value.is_null()) {
        if !meta.is_object() {
            return Err("invalid_identity");
        }
        declarations.push(meta.clone());
        if let Some(value) = meta.get("x-codex-turn-metadata") {
            declarations.push(metadata_object(value)?);
        }
    }
    for value in headers.get_all("x-codex-turn-metadata") {
        let value = value.to_str().map_err(|_| "invalid_identity")?;
        declarations.push(metadata_object(&Value::String(value.to_owned()))?);
    }
    let identity = |keys: &[&str]| {
        let mut values = Vec::new();
        for declaration in &declarations {
            for key in keys {
                if let Some(value) = declaration.get(*key).filter(|value| !value.is_null()) {
                    values.push(Some(value.as_str().ok_or("invalid_identity")?));
                }
            }
        }
        control_plane_contracts::ports::resolve_client_log_identity(&values)
    };
    Ok(McpClientContext {
        thread_id: identity(&["threadId", "thread_id"])?,
        turn_id: identity(&["turnId", "turn_id"])?,
        session_id: identity(&["sessionId", "session_id"])?,
        error: None,
    })
}

fn metadata_object(value: &Value) -> Result<Value, &'static str> {
    let value = match value {
        Value::String(value) => serde_json::from_str(value).map_err(|_| "invalid_identity")?,
        value => value.clone(),
    };
    value.is_object().then_some(value).ok_or("invalid_identity")
}

tokio::task_local! {
    static CLIENT_CONTEXT: McpClientContext;
}

pub(crate) async fn with_client_context<F: Future>(
    context: McpClientContext,
    future: F,
) -> F::Output {
    CLIENT_CONTEXT.scope(context, future).await
}

pub(crate) fn current_client_context() -> McpClientContext {
    CLIENT_CONTEXT.try_with(Clone::clone).unwrap_or_default()
}

#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
#[path = "_tests/client_context.rs"]
mod tests;
