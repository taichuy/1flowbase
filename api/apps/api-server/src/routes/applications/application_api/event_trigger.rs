use super::*;

#[derive(Debug, Clone, Copy, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowEventTriggerPhaseBody {
    Subscribed,
    Published,
    Consumed,
}
impl From<domain::event_center::EventTriggerPhase> for WorkflowEventTriggerPhaseBody {
    fn from(value: domain::event_center::EventTriggerPhase) -> Self {
        match value {
            domain::event_center::EventTriggerPhase::Subscribed => Self::Subscribed,
            domain::event_center::EventTriggerPhase::Published => Self::Published,
            domain::event_center::EventTriggerPhase::Consumed => Self::Consumed,
        }
    }
}
#[derive(Debug, Deserialize, ToSchema, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowEventTriggerBody {
    #[serde(default)]
    #[schema(value_type = Option<serde_json::Value>)]
    pub source: Option<domain::event_center::EventSourceSelection>,
    pub enabled: bool,
    pub contract_id: String,
    pub trigger_phase: WorkflowEventTriggerPhaseBody,
    /// Workflow Start input name to RFC6901 pointer into the lifecycle signal.
    pub input_mapping: std::collections::BTreeMap<String, String>,
}
impl WorkflowEventTriggerBody {
    pub(super) fn into_domain(self) -> domain::event_center::WorkflowEventTriggerInput {
        domain::event_center::WorkflowEventTriggerInput {
            source: self.source,
            enabled: self.enabled,
            contract_id: self.contract_id,
            trigger_phase: match self.trigger_phase {
                WorkflowEventTriggerPhaseBody::Subscribed => {
                    domain::event_center::EventTriggerPhase::Subscribed
                }
                WorkflowEventTriggerPhaseBody::Published => {
                    domain::event_center::EventTriggerPhase::Published
                }
                WorkflowEventTriggerPhaseBody::Consumed => {
                    domain::event_center::EventTriggerPhase::Consumed
                }
            },
            input_mapping: self.input_mapping,
        }
    }
}
#[derive(Debug, Serialize, ToSchema)]
pub struct WorkflowEventTriggerResponse {
    #[schema(value_type = Option<serde_json::Value>)]
    pub source: Option<domain::event_center::EventSourceSelection>,
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub application_id: Uuid,
    pub enabled: bool,
    pub contract_id: String,
    pub trigger_phase: WorkflowEventTriggerPhaseBody,
    pub input_mapping: std::collections::BTreeMap<String, String>,
    pub updated_by: Uuid,
    pub updated_at: String,
}
pub(super) fn to_workflow_event_trigger_response(
    record: domain::event_center::WorkflowEventTriggerRecord,
) -> Result<WorkflowEventTriggerResponse, ApiError> {
    Ok(WorkflowEventTriggerResponse {
        source: record.configuration.source,
        id: record.id,
        workspace_id: record.workspace_id,
        application_id: record.application_id,
        enabled: record.configuration.enabled,
        contract_id: record.configuration.contract_id,
        trigger_phase: record.configuration.trigger_phase.into(),
        input_mapping: record.configuration.input_mapping,
        updated_by: record.updated_by,
        updated_at: record
            .updated_at
            .format(&Rfc3339)
            .map_err(anyhow::Error::from)?,
    })
}

#[utoipa::path(get, path = "/api/console/applications/{application_id}/workflow-event-trigger",
    summary = "Get Workflow event trigger configuration",
    description = "Returns lifecycle event identity, phase, input mapping and desired enabled state for an event-triggered Workflow in the current workspace.",
    params(("application_id" = Uuid, Path, description = "Application id")),
    responses((status = 200, body = Option<WorkflowEventTriggerResponse>), (status = 400, body = crate::error_response::ErrorBody), (status = 401, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody), (status = 404, body = crate::error_response::ErrorBody)))]
pub async fn get_workflow_event_trigger(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(application_id): Path<Uuid>,
) -> Result<Json<ApiSuccess<Option<WorkflowEventTriggerResponse>>>, ApiError> {
    let output: interface_event::WorkflowEventOutput = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.applications.workflow-event.get.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::Protocol { state, headers },
        interface_event::WorkflowEventInput::Get { application_id },
    )
    .await?;
    Ok(Json(ApiSuccess::new(output.into_optional()?)))
}
#[utoipa::path(put, path = "/api/console/applications/{application_id}/workflow-event-trigger",
    summary = "Replace Workflow event trigger configuration",
    description = "Replaces event identity, lifecycle phase, RFC6901 input mapping and desired enabled state. Unpublished Workflows may save configuration; activation waits for a publication.",
    params(("application_id" = Uuid, Path, description = "Application id")), request_body = WorkflowEventTriggerBody,
    responses((status = 200, body = WorkflowEventTriggerResponse), (status = 400, body = crate::error_response::ErrorBody), (status = 401, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody), (status = 404, body = crate::error_response::ErrorBody)))]
pub async fn replace_workflow_event_trigger(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(application_id): Path<Uuid>,
    Json(body): Json<WorkflowEventTriggerBody>,
) -> Result<Json<ApiSuccess<WorkflowEventTriggerResponse>>, ApiError> {
    let output: interface_event::WorkflowEventOutput = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.applications.workflow-event.replace.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf { state, headers },
        interface_event::WorkflowEventInput::Replace {
            application_id,
            body,
        },
    )
    .await?;
    Ok(Json(ApiSuccess::new(output.into_required()?)))
}
