use super::*;
use control_plane::ports::{
    ProviderTrajectoryBody, ProviderTrajectoryPage, ProviderTrajectoryView,
};

#[derive(Debug, Default, Deserialize)]
pub struct ProviderTrajectoryQuery {
    pub request_id: Option<Uuid>,
    pub focus_event_id: Option<Uuid>,
    pub cursor: Option<i64>,
    pub limit: Option<i64>,
    #[serde(default)]
    pub view: ProviderTrajectoryView,
}

/// List recorded Native and historical supplier semantic trajectory summaries.
/// Returns a bounded page without reading protocol bodies, scoped to the visible application and exact node execution.
#[utoipa::path(get, path = "/api/console/applications/{id}/logs/runs/{run_id}/nodes/{node_run_id}/trajectory",
    params(("id" = Uuid, Path), ("run_id" = Uuid, Path), ("node_run_id" = Uuid, Path),
        ("cursor" = Option<i64>, Query), ("limit" = Option<i64>, Query), ("request_id" = Option<Uuid>, Query), ("focus_event_id" = Option<Uuid>, Query), ("view" = Option<String>, Query, description = "semantic (default) or protocol")),
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

/// Read Native step details or explicitly selected supplier protocol evidence.
/// Resolves only the selected durable event within the authorized run and node execution.
#[utoipa::path(get, path = "/api/console/applications/{id}/logs/runs/{run_id}/nodes/{node_run_id}/trajectory/{event_id}",
    params(("id" = Uuid, Path), ("run_id" = Uuid, Path), ("node_run_id" = Uuid, Path), ("event_id" = Uuid, Path), ("cursor" = Option<i64>, Query), ("limit" = Option<i64>, Query), ("request_id" = Option<Uuid>, Query), ("focus_event_id" = Option<Uuid>, Query), ("view" = Option<String>, Query, description = "semantic (default) or protocol")),
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

/// List the run-wide Native and historical supplier trajectory.
/// Reads bounded step summaries across all nodes in the authorized run without loading event bodies.
#[utoipa::path(get, path = "/api/console/applications/{id}/logs/runs/{run_id}/trajectory",
    params(("id" = Uuid, Path), ("run_id" = Uuid, Path), ("cursor" = Option<i64>, Query), ("limit" = Option<i64>, Query), ("request_id" = Option<Uuid>, Query), ("focus_event_id" = Option<Uuid>, Query)),
    responses((status = 200, body = serde_json::Value)))]
pub async fn list_run_trajectory(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path((application_id, run_id)): Path<(Uuid, Uuid)>,
    Query(query): Query<ProviderTrajectoryQuery>,
) -> Result<Json<ApiSuccess<ProviderTrajectoryPage>>, ApiError> {
    let output = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.applications.runtime.run.trajectory.list.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::Protocol { state, headers },
        interface_runtime_reads::ApplicationRuntimeReadsInput::RunTrajectoryPage {
            application_id,
            run_id,
            query,
        },
    )
    .await?;
    let interface_runtime_reads::ApplicationRuntimeReadsOutput::TrajectoryPage(page) = output
    else {
        unreachable!("run trajectory binding output")
    };
    Ok(Json(ApiSuccess::new(page)))
}

/// Read one selected run input or output payload.
/// Loads only the requested flow field, preserving original JSON and existing debug artifact references.
#[utoipa::path(get, path = "/api/console/applications/{id}/logs/runs/{run_id}/payloads/{section}",
    params(("id" = Uuid, Path), ("run_id" = Uuid, Path), ("section" = String, Path, description = "input_payload or output_payload")),
    responses((status = 200, body = serde_json::Value)))]
pub async fn get_run_payload(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path((application_id, run_id, section)): Path<(
        Uuid,
        Uuid,
        control_plane::ports::ApplicationRunPayloadSection,
    )>,
) -> Result<Json<ApiSuccess<serde_json::Value>>, ApiError> {
    let output = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.applications.runtime.run.payload.get.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::Protocol { state, headers },
        interface_runtime_reads::ApplicationRuntimeReadsInput::RunPayload {
            application_id,
            run_id,
            section,
        },
    )
    .await?;
    let interface_runtime_reads::ApplicationRuntimeReadsOutput::RunPayload(payload) = output else {
        unreachable!("run payload binding output")
    };
    Ok(Json(ApiSuccess::new(payload)))
}

#[derive(Debug, Default, Deserialize)]
pub struct ClientTrajectoryQuery {
    pub request_id: Option<Uuid>,
    pub focus_step_id: Option<Uuid>,
    pub node_run_id: Option<Uuid>,
    pub cursor: Option<i64>,
    pub limit: Option<i64>,
    pub section: Option<String>,
}

/// List original client Responses trajectory summaries.
/// Reads bounded metadata within the authorized application and run without loading protocol bodies.
#[utoipa::path(get, path = "/api/console/applications/{id}/logs/runs/{run_id}/client-trajectory",
    params(("id" = Uuid, Path), ("run_id" = Uuid, Path), ("node_run_id" = Option<Uuid>, Query), ("cursor" = Option<i64>, Query), ("limit" = Option<i64>, Query), ("request_id" = Option<Uuid>, Query), ("focus_step_id" = Option<Uuid>, Query)),
    responses((status = 200, body = serde_json::Value)))]
pub async fn list_client_trajectory(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path((application_id, run_id)): Path<(Uuid, Uuid)>,
    Query(query): Query<ClientTrajectoryQuery>,
) -> Result<Json<ApiSuccess<control_plane::ports::ClientTrajectoryPage>>, ApiError> {
    let output = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.applications.runtime.client-trajectory.list.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::Protocol { state, headers },
        interface_runtime_reads::ApplicationRuntimeReadsInput::ClientTrajectoryPage {
            application_id,
            run_id,
            query,
        },
    )
    .await?;
    let interface_runtime_reads::ApplicationRuntimeReadsOutput::ClientTrajectoryPage(page) = output
    else {
        unreachable!("client trajectory page binding output")
    };
    Ok(Json(ApiSuccess::new(page)))
}

/// Read one selected client protocol trajectory section.
/// Loads only the requested section of a step scoped to the authorized application, run and optional node.
#[utoipa::path(get, path = "/api/console/applications/{id}/logs/runs/{run_id}/client-trajectory/{step_id}",
    params(("id" = Uuid, Path), ("run_id" = Uuid, Path), ("step_id" = Uuid, Path), ("node_run_id" = Option<Uuid>, Query), ("section" = Option<String>, Query), ("cursor" = Option<i64>, Query), ("limit" = Option<i64>, Query), ("request_id" = Option<Uuid>, Query), ("focus_step_id" = Option<Uuid>, Query)),
    responses((status = 200, body = serde_json::Value)))]
pub async fn get_client_trajectory_section(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path((application_id, run_id, step_id)): Path<(Uuid, Uuid, Uuid)>,
    Query(query): Query<ClientTrajectoryQuery>,
) -> Result<Json<ApiSuccess<control_plane::ports::ClientTrajectorySection>>, ApiError> {
    let output = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.applications.runtime.client-trajectory.section.get.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::Protocol { state, headers },
        interface_runtime_reads::ApplicationRuntimeReadsInput::ClientTrajectorySection {
            application_id,
            run_id,
            step_id,
            query,
        },
    )
    .await?;
    let interface_runtime_reads::ApplicationRuntimeReadsOutput::ClientTrajectorySection(section) =
        output
    else {
        unreachable!("client trajectory section binding output")
    };
    Ok(Json(ApiSuccess::new(section)))
}
