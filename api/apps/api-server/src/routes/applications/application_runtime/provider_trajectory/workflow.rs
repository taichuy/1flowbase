use super::*;
use control_plane::ports::{
    WorkflowTrajectoryBody, WorkflowTrajectoryPage, WorkflowTrajectoryQuery,
};

/// List workflow events with execution-time node names and explicit task relations.
/// Filters the authorized task and descendants before stable keyset pagination; no event bodies are loaded.
#[utoipa::path(get, path = "/api/console/applications/{id}/logs/runs/{run_id}/workflow-trajectory",
    params(("id" = Uuid, Path), ("run_id" = Uuid, Path),
        ("category" = Option<String>, Query, description = "all, nodes, requests, tools, rounds or agents"),
        ("node_run_id" = Option<Uuid>, Query), ("request_id" = Option<Uuid>, Query),
        ("from" = Option<String>, Query), ("to" = Option<String>, Query),
        ("cursor" = Option<String>, Query), ("limit" = Option<i64>, Query)),
    responses((status = 200, body = serde_json::Value)))]
pub async fn list_workflow_trajectory(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path((application_id, run_id)): Path<(Uuid, Uuid)>,
    Query(query): Query<WorkflowTrajectoryQuery>,
) -> Result<Json<ApiSuccess<WorkflowTrajectoryPage>>, ApiError> {
    let output = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.applications.runtime.workflow-trajectory.list.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::Protocol { state, headers },
        interface_runtime_reads::ApplicationRuntimeReadsInput::WorkflowTrajectoryPage {
            application_id,
            run_id,
            query,
        },
    )
    .await?;
    let interface_runtime_reads::ApplicationRuntimeReadsOutput::WorkflowTrajectoryPage(page) =
        output
    else {
        unreachable!("workflow trajectory page binding output")
    };
    Ok(Json(ApiSuccess::new(page)))
}

/// Read a selected workflow event or node lifecycle snapshot.
/// Rechecks task membership and application visibility before loading the selected payload.
#[utoipa::path(get, path = "/api/console/applications/{id}/logs/runs/{run_id}/workflow-trajectory/{event_id}",
    params(("id" = Uuid, Path), ("run_id" = Uuid, Path), ("event_id" = String, Path)),
    responses((status = 200, body = serde_json::Value)))]
pub async fn get_workflow_trajectory_body(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path((application_id, run_id, event_id)): Path<(Uuid, Uuid, String)>,
) -> Result<Json<ApiSuccess<WorkflowTrajectoryBody>>, ApiError> {
    let output = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.applications.runtime.workflow-trajectory.body.get.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::Protocol { state, headers },
        interface_runtime_reads::ApplicationRuntimeReadsInput::WorkflowTrajectoryBody {
            application_id,
            run_id,
            event_id,
        },
    )
    .await?;
    let interface_runtime_reads::ApplicationRuntimeReadsOutput::WorkflowTrajectoryBody(body) =
        output
    else {
        unreachable!("workflow trajectory body binding output")
    };
    Ok(Json(ApiSuccess::new(body)))
}
