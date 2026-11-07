//! Lazy metadata-only trace readers for imported records.
use super::*;
#[derive(Debug, Default, Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct RecordTraceChildrenQuery {
    pub cursor: Option<String>,
    pub page_size: Option<i64>,
}

/// Read the selected task root without loading transcripts or descendants.
/// Every node has an explicit record_id for its own client trajectory.
#[utoipa::path(get, path="/api/console/applications/{id}/logs/records/{record_id}/trace-tree",
    params(("id"=Uuid,Path),("record_id"=Uuid,Path)),
    responses((status=200,body=log_query::schemas::RecordTraceTreeSchema)))]
pub async fn get_record_trace_tree(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path((application_id, record_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<ApiSuccess<control_plane::ports::ApplicationLogRecordTraceTree>>, ApiError> {
    let output = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.applications.runtime.record.trace-tree.get.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::Protocol { state, headers },
        interface_runtime_reads::ApplicationRuntimeReadsInput::RecordTraceTree {
            application_id,
            record_id,
        },
    )
    .await?;
    let interface_runtime_reads::ApplicationRuntimeReadsOutput::RecordTraceTree(tree) = output
    else {
        unreachable!("record trace tree binding output")
    };
    Ok(Json(ApiSuccess::new(tree)))
}

/// Page direct child task metadata in this record's source namespace.
/// Reject foreign nodes and cursors; load each child's own content through its record_id.
#[utoipa::path(get, path="/api/console/applications/{id}/logs/records/{record_id}/trace-tree/{trace_node_id}/children",
    params(("id"=Uuid,Path),("record_id"=Uuid,Path),("trace_node_id"=Uuid,Path),("cursor"=Option<String>,Query),("page_size"=Option<i64>,Query)),
    responses((status=200,body=log_query::schemas::RecordTraceChildrenSchema)))]
pub async fn list_record_trace_children(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path((application_id, record_id, trace_node_id)): Path<(Uuid, Uuid, Uuid)>,
    Query(query): Query<RecordTraceChildrenQuery>,
) -> Result<Json<ApiSuccess<control_plane::ports::ApplicationLogRecordTraceChildren>>, ApiError> {
    let output = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.applications.runtime.record.trace-tree.children.list.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::Protocol { state, headers },
        interface_runtime_reads::ApplicationRuntimeReadsInput::RecordTraceChildren {
            application_id,
            record_id,
            trace_node_id,
            query,
        },
    )
    .await?;
    let interface_runtime_reads::ApplicationRuntimeReadsOutput::RecordTraceChildren(page) = output
    else {
        unreachable!("record trace children binding output")
    };
    Ok(Json(ApiSuccess::new(page)))
}
