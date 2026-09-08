use super::*;
use control_plane_contracts::ports::{
    ManagedExecutionState, ManagedFrozenExecutionTarget, ResumeManagedLifecycleDelivery,
};

#[derive(Debug, serde::Deserialize, serde::Serialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ManagedExecutionTargetBody {
    pub graph_fingerprint: String,
    pub handler_id: String,
    pub handler_version: String,
}
impl From<ManagedExecutionTargetBody> for ManagedFrozenExecutionTarget {
    fn from(value: ManagedExecutionTargetBody) -> Self {
        Self {
            graph_fingerprint: value.graph_fingerprint,
            handler_id: value.handler_id,
            handler_version: value.handler_version,
        }
    }
}
#[derive(Debug, serde::Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ResumeManagedDeliveryBody {
    pub event_id: Uuid,
    pub subscriber_id: String,
    pub expected: ManagedExecutionTargetBody,
}
impl From<ResumeManagedDeliveryBody> for ResumeManagedLifecycleDelivery {
    fn from(value: ResumeManagedDeliveryBody) -> Self {
        Self {
            event_id: value.event_id,
            subscriber_id: value.subscriber_id,
            expected: value.expected.into(),
        }
    }
}

#[utoipa::path(get,path="/api/console/settings/extension-center/installed/{installation_id}/managed-execution",operation_id="extension_center_managed_execution_view",summary="View managed executions and durable lifecycle deliveries",responses((status=200),(status=403,body=crate::error_response::ErrorBody)))]
pub(super) async fn view_managed_execution(
    State(state): State<Arc<ApiState>>,
    Path(installation_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<ManagedExecutionState>>, ApiError> {
    let output = invoke_interface(
        state,
        headers,
        "http.console.extension-center.managed-execution.view.v1",
        interface::ExtensionCenterInput::QueryManagedExecution(installation_id),
        false,
    )
    .await?;
    let interface::ExtensionCenterOutput::ManagedExecution(state) = output else {
        unreachable!("managed execution binding returned different output")
    };
    Ok(Json(ApiSuccess::new(state)))
}
#[utoipa::path(post,path="/api/console/settings/extension-center/installed/{installation_id}/lifecycle-deliveries/resume",operation_id="extension_center_lifecycle_deliveries_resume",summary="Resume one exact paused lifecycle delivery",request_body=ResumeManagedDeliveryBody,responses((status=200),(status=403,body=crate::error_response::ErrorBody)))]
pub(super) async fn resume_managed_delivery(
    State(state): State<Arc<ApiState>>,
    Path(installation_id): Path<Uuid>,
    headers: HeaderMap,
    Json(body): Json<ResumeManagedDeliveryBody>,
) -> Result<Json<ApiSuccess<ManagedExecutionState>>, ApiError> {
    let output = invoke_interface(
        state,
        headers,
        "http.console.extension-center.lifecycle-deliveries.resume.v1",
        interface::ExtensionCenterInput::ResumeManagedDelivery(installation_id, body),
        true,
    )
    .await?;
    let interface::ExtensionCenterOutput::ManagedExecution(state) = output else {
        unreachable!("managed resume binding returned different output")
    };
    Ok(Json(ApiSuccess::new(state)))
}
#[utoipa::path(post,path="/api/console/settings/extension-center/installed/{installation_id}/managed-executions/retire",operation_id="extension_center_managed_executions_retire",summary="Retire one unreferenced frozen managed execution",request_body=ManagedExecutionTargetBody,responses((status=200),(status=403,body=crate::error_response::ErrorBody)))]
pub(super) async fn retire_managed_execution(
    State(state): State<Arc<ApiState>>,
    Path(installation_id): Path<Uuid>,
    headers: HeaderMap,
    Json(body): Json<ManagedExecutionTargetBody>,
) -> Result<Json<ApiSuccess<ManagedExecutionState>>, ApiError> {
    let output = invoke_interface(
        state,
        headers,
        "http.console.extension-center.managed-executions.retire.v1",
        interface::ExtensionCenterInput::RetireManagedExecution(installation_id, body),
        true,
    )
    .await?;
    let interface::ExtensionCenterOutput::ManagedExecution(state) = output else {
        unreachable!("managed retirement binding returned different output")
    };
    Ok(Json(ApiSuccess::new(state)))
}
