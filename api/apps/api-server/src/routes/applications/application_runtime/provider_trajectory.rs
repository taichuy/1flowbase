use super::*;
use control_plane::ports::{ProviderTrajectoryBody, ProviderTrajectoryPage};

#[derive(Debug, Default, Deserialize)]
pub struct ProviderTrajectoryQuery {
    pub cursor: Option<i64>,
    pub limit: Option<i64>,
}

/// List recorded provider protocol trajectory summaries.
/// Returns a bounded page without reading protocol bodies, scoped to the visible application and exact node execution.
#[utoipa::path(get, path = "/api/console/applications/{id}/logs/runs/{run_id}/nodes/{node_run_id}/trajectory",
    params(("id" = Uuid, Path), ("run_id" = Uuid, Path), ("node_run_id" = Uuid, Path),
        ("cursor" = Option<i64>, Query), ("limit" = Option<i64>, Query)),
    responses((status = 200, body = serde_json::Value)))]
pub async fn list_provider_trajectory(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path((application_id, run_id, node_run_id)): Path<(Uuid, Uuid, Uuid)>,
    Query(query): Query<ProviderTrajectoryQuery>,
) -> Result<Json<ApiSuccess<ProviderTrajectoryPage>>, ApiError> {
    let output = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.applications.runtime.trajectory.list.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::Protocol { state, headers },
        interface_runtime_reads::ApplicationRuntimeReadsInput::TrajectoryPage {
            application_id,
            run_id,
            node_run_id,
            query,
        },
    )
    .await?;
    let interface_runtime_reads::ApplicationRuntimeReadsOutput::TrajectoryPage(page) = output
    else {
        unreachable!("trajectory page binding output")
    };
    Ok(Json(ApiSuccess::new(page)))
}

/// Read a bounded page of original evidence for one semantic step or protocol observation.
/// Resolves only the selected durable event within the authorized run and node execution.
#[utoipa::path(get, path = "/api/console/applications/{id}/logs/runs/{run_id}/nodes/{node_run_id}/trajectory/{event_id}",
    params(("id" = Uuid, Path), ("run_id" = Uuid, Path), ("node_run_id" = Uuid, Path), ("event_id" = Uuid, Path), ("cursor" = Option<i64>, Query), ("limit" = Option<i64>, Query)),
    responses((status = 200, body = serde_json::Value)))]
pub async fn get_provider_trajectory_body(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path((application_id, run_id, node_run_id, event_id)): Path<(Uuid, Uuid, Uuid, Uuid)>,
    Query(query): Query<ProviderTrajectoryQuery>,
) -> Result<Json<ApiSuccess<ProviderTrajectoryBody>>, ApiError> {
    let output = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.applications.runtime.trajectory.body.get.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::Protocol { state, headers },
        interface_runtime_reads::ApplicationRuntimeReadsInput::TrajectoryBody {
            application_id,
            run_id,
            node_run_id,
            event_id,
            query,
        },
    )
    .await?;
    let interface_runtime_reads::ApplicationRuntimeReadsOutput::TrajectoryBody(body) = output
    else {
        unreachable!("trajectory body binding output")
    };
    Ok(Json(ApiSuccess::new(body)))
}
