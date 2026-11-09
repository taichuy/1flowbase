//! Canonical imported conversation timeline. Positioning never changes membership.
use super::*;
use control_plane::ports::{ClientTrajectorySection, ConversationClientTrajectoryQueryPage};

#[derive(Debug, Default, Deserialize, ToSchema, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct ConversationTrajectoryQuery {
    /// Predicates on original event fields and source provenance; see query-fields.
    pub filter: Option<serde_json::Value>,
    /// Opaque previous or next cursor. Reuse the same query including focus.
    pub cursor: Option<String>,
    /// Positive output page size, not a search or conversation capacity limit.
    pub limit: Option<i64>,
    /// Search restored overview/parameters/result sections throughout this conversation.
    pub keyword: Option<String>,
    pub search_sections: Option<Vec<String>>,
    /// Initially position at this member record; older and newer events remain readable.
    pub focus_record_id: Option<Uuid>,
    pub focus_step_id: Option<Uuid>,
    pub index_only: Option<bool>,
}

#[derive(Debug, Default, Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct ConversationTrajectorySectionQuery {
    pub section: Option<String>,
    pub cursor: Option<i64>,
    pub limit: Option<i64>,
}

/// Query one canonical conversation including every owned subagent.
/// Orders original events by timestamp and UUID. Subagent is a provenance filter, not an event category. Refresh without a cursor to include late-arriving history.
#[utoipa::path(post, path="/api/console/applications/{id}/logs/conversations/{log_conversation_id}/client-trajectory/query",
    params(("id"=Uuid,Path),("log_conversation_id"=Uuid,Path)),
    request_body=ConversationTrajectoryQuery,
    responses((status=200,body=schemas::ConversationTrajectoryQueryPageSchema)))]
pub async fn query_conversation_trajectory(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path((application_id, log_conversation_id)): Path<(Uuid, Uuid)>,
    Json(query): Json<ConversationTrajectoryQuery>,
) -> Result<Json<ApiSuccess<ConversationClientTrajectoryQueryPage>>, ApiError> {
    let output = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.applications.runtime.conversation-trajectory.query.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::Protocol { state, headers },
        interface_runtime_reads::ApplicationRuntimeReadsInput::QueryConversationTrajectory {
            application_id,
            log_conversation_id,
            query,
        },
    )
    .await?;
    let interface_runtime_reads::ApplicationRuntimeReadsOutput::ConversationTrajectoryQuery(page) =
        output
    else {
        unreachable!("conversation trajectory binding output")
    };
    Ok(Json(ApiSuccess::new(page)))
}

/// Read an original event section in the authorized canonical conversation.
/// Resolves the event's actual owning record, including descendants, without copying or flattening its source identity.
#[utoipa::path(get, path="/api/console/applications/{id}/logs/conversations/{log_conversation_id}/client-trajectory/{step_id}",
    params(("id"=Uuid,Path),("log_conversation_id"=Uuid,Path),("step_id"=Uuid,Path),
        ("section"=Option<String>,Query),("cursor"=Option<i64>,Query),("limit"=Option<i64>,Query)),
    responses((status=200,body=schemas::TrajectorySectionSchema)))]
pub async fn get_conversation_trajectory_section(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path((application_id, log_conversation_id, step_id)): Path<(Uuid, Uuid, Uuid)>,
    Query(query): Query<ConversationTrajectorySectionQuery>,
) -> Result<Json<ApiSuccess<ClientTrajectorySection>>, ApiError> {
    let output = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.applications.runtime.conversation-trajectory.section.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::Protocol { state, headers },
        interface_runtime_reads::ApplicationRuntimeReadsInput::ConversationTrajectorySection {
            application_id,
            log_conversation_id,
            step_id,
            query,
        },
    )
    .await?;
    let interface_runtime_reads::ApplicationRuntimeReadsOutput::ClientTrajectorySection(section) =
        output
    else {
        unreachable!("conversation trajectory section binding output")
    };
    Ok(Json(ApiSuccess::new(section)))
}
