use super::*;
use crate::routes::mcp_protocol::client_context::{current_client_context, McpClientContext};

#[derive(Debug, Serialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CurrentRunContextStatus {
    Resolved,
    MissingIdentity,
    InvalidIdentity,
    ConflictingIdentity,
    NotFound,
    Ambiguous,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct CurrentRunContextResponse {
    pub status: CurrentRunContextStatus,
    pub thread_id: Option<String>,
    pub turn_id: Option<String>,
    /// Client process/session identity, independent of the conversation thread.
    pub session_id: Option<String>,
    pub application_id: Option<Uuid>,
    /// Canonical log record identity used by the application's run details.
    pub record_id: Option<Uuid>,
    pub native_run_id: Option<Uuid>,
    pub log_conversation_id: Option<Uuid>,
}

impl CurrentRunContextResponse {
    pub(crate) fn from_context(context: &McpClientContext) -> Self {
        let status = match context.error {
            Some("conflicting_identity") => CurrentRunContextStatus::ConflictingIdentity,
            Some(_) => CurrentRunContextStatus::InvalidIdentity,
            None if context.thread_id.is_none() || context.turn_id.is_none() => {
                CurrentRunContextStatus::MissingIdentity
            }
            None => CurrentRunContextStatus::NotFound,
        };
        Self {
            status,
            thread_id: context.thread_id.clone(),
            turn_id: context.turn_id.clone(),
            session_id: context.session_id.clone(),
            application_id: None,
            record_id: None,
            native_run_id: None,
            log_conversation_id: None,
        }
    }

    pub(crate) fn resolve(&mut self, page: ApplicationLogRecordsPage) {
        if page.items.len() > 1 || page.next_cursor.is_some() {
            self.status = CurrentRunContextStatus::Ambiguous;
        } else if let Some(record) = page.items.into_iter().next() {
            self.status = CurrentRunContextStatus::Resolved;
            self.application_id = Some(record.application_id);
            self.record_id = Some(record.record_id);
            self.native_run_id = if record.source_kind == "native" {
                Some(record.record_id)
            } else {
                record.native_run_id
            };
            self.log_conversation_id = record.log_conversation_id;
        }
    }
}

/// Get the current MCP caller's turn and log record identity.
/// Reads client-declared MCP correlation metadata and searches only readable applications in the current workspace. Missing, invalid, conflicting, absent or ambiguous identities return explicit statuses with no record identity; never selects the latest run. HTTP calls outside an MCP invocation report missing_identity.
#[utoipa::path(get, path="/api/console/applications/logs/current-context", responses(
    (status=200, body=CurrentRunContextResponse),
    (status=401, body=crate::error_response::ErrorBody),
    (status=403, body=crate::error_response::ErrorBody)
))]
pub async fn get_current_run_context(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<CurrentRunContextResponse>>, ApiError> {
    let output = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.applications.runtime.current-context.get.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::Protocol { state, headers },
        interface_runtime_reads::ApplicationRuntimeReadsInput::CurrentContext {
            context: current_client_context(),
        },
    )
    .await?;
    let interface_runtime_reads::ApplicationRuntimeReadsOutput::CurrentContext(response) = output
    else {
        unreachable!("current context binding output")
    };
    Ok(Json(ApiSuccess::new(response)))
}

/// Exact persisted identity predicates; imported Codex session IDs denote threads.
pub(crate) fn context_filter(context: &McpClientContext) -> Option<serde_json::Value> {
    if context.error.is_some() {
        return None;
    }
    let (thread, turn) = (context.thread_id.as_deref()?, context.turn_id.as_deref()?);
    Some(serde_json::json!({"$or":[
        {"source_kind":{"$eq":"native"},"client_thread_id":{"$eq":thread},"client_turn_id":{"$eq":turn}},
        {"source_kind":{"$eq":"imported"},"source_client":{"$eq":"codex"},"source_session_id":{"$eq":thread},"source_task_id":{"$eq":turn}}
    ]}))
}

#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
#[path = "_tests/current_context.rs"]
mod tests;
