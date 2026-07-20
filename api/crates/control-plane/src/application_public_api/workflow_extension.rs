use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use time::OffsetDateTime;
use uuid::Uuid;

use super::{
    mapping::{WorkflowExtensionAccessPolicy, WorkflowExtensionHttpMethod, WorkflowExtensionResponseMode},
    published_workflow_operation::{
        build_published_workflow_operations, resolve_published_workflow_operation,
        PublishedWorkflowOperationError,
    },
    run_service::{
        public_compiled_plan_start_node_id, public_freeze_workflow_run_input_environment,
        ApplicationPublishedFlowRunRepository, WorkflowRunTriggerContext,
    },
    workflow_start_http_inputs::{build_workflow_start_node_input_payload, parse_workflow_start_http_inputs},
};
use crate::{
    application_public_api::ensure_application_view_permission,
    auth::ApiKeyService,
    flow_run_title::build_flow_run_title,
    ports::{
        ApiKeyRepository, ApplicationCompiledPlanRepository, ApplicationPublicationRepository,
        ApplicationRepository, AuthRepository, CreateFlowRunInput,
    },
};

#[derive(Debug, Clone)]
pub struct CreateWorkflowExtensionRunCommand {
    pub bearer_token: Option<String>,
    pub request_path: String,
    pub method: WorkflowExtensionHttpMethod,
    pub parameters: WorkflowExtensionRequestParameters,
}

#[derive(Debug, Clone, Default)]
pub struct WorkflowExtensionRequestParameters {
    pub path: BTreeMap<String, Value>,
    pub query: Map<String, Value>,
    pub form: Map<String, Value>,
    pub body: Value,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkflowExtensionRunError {
    NotAuthenticated,
    ExtensionNotFound,
    ApplicationNotPublished,
    Forbidden,
    MethodNotAllowed,
    TriggerTypeMismatch,
    InvalidMapping,
    RouteConflict,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowExtensionRunResult {
    pub id: Uuid,
    pub application_id: Uuid,
    pub api_key_id: Option<Uuid>,
    pub publication_version_id: Uuid,
    pub status: domain::FlowRunStatus,
    pub response_mode: WorkflowExtensionResponseMode,
    pub sync_timeout_ms: u64,
    pub node_input_payload: Value,
    pub created_at: OffsetDateTime,
}

pub struct WorkflowExtensionRunService<R> {
    repository: R,
}

impl<R> WorkflowExtensionRunService<R>
where
    R: ApplicationRepository
        + ApiKeyRepository
        + AuthRepository
        + ApplicationPublicationRepository
        + ApplicationCompiledPlanRepository
        + ApplicationPublishedFlowRunRepository
        + Clone,
{
    pub fn new(repository: R) -> Self {
        Self { repository }
    }

    pub async fn create_run(
        &self,
        command: CreateWorkflowExtensionRunCommand,
    ) -> Result<WorkflowExtensionRunResult, WorkflowExtensionRunError> {
        let operations = build_published_workflow_operations(
            self.repository
                .list_enabled_extension_publications()
                .await
                .map_err(|_| WorkflowExtensionRunError::ExtensionNotFound)?,
        )
        .map_err(map_operation_error)?;
        if operations.iter().any(|operation| {
            operation.method != command.method
                && operation.match_path(&command.request_path).is_some()
        }) && !operations.iter().any(|operation| {
            operation.method == command.method
                && operation.match_path(&command.request_path).is_some()
        }) {
            return Err(WorkflowExtensionRunError::MethodNotAllowed);
        }
        let (operation, path) = resolve_published_workflow_operation(
            &operations,
            command.method,
            &command.request_path,
        )
        .map_err(map_operation_error)?;
        let publication = operation.publication.clone();
        let (actor_user_id, workspace_id, api_key_id) = match operation.access_policy {
            WorkflowExtensionAccessPolicy::UserApiKey => {
                let token = command
                    .bearer_token
                    .as_deref()
                    .ok_or(WorkflowExtensionRunError::NotAuthenticated)?;
                let user_api_key = ApiKeyService::new(self.repository.clone())
                    .authenticate_user_api_key(token)
                    .await
                    .map_err(|_| WorkflowExtensionRunError::NotAuthenticated)?;
                if user_api_key.actor.current_workspace_id != operation.workspace_id {
                    return Err(WorkflowExtensionRunError::Forbidden);
                }
                let application = self
                    .repository
                    .get_application(operation.workspace_id, operation.application_id)
                    .await
                    .map_err(|_| WorkflowExtensionRunError::ExtensionNotFound)?
                    .ok_or(WorkflowExtensionRunError::ExtensionNotFound)?;
                ensure_application_view_permission(
                    &self.repository,
                    &user_api_key.actor,
                    &application,
                )
                .await
                .map_err(|_| WorkflowExtensionRunError::Forbidden)?;
                (
                    user_api_key.user.id,
                    user_api_key.actor.current_workspace_id,
                    Some(user_api_key.api_key.id),
                )
            }
            WorkflowExtensionAccessPolicy::Public => {
                (publication.created_by, operation.workspace_id, None)
            }
        };
        let application = self
            .repository
            .get_application(workspace_id, operation.application_id)
            .await
            .map_err(|_| WorkflowExtensionRunError::ExtensionNotFound)?
            .ok_or(WorkflowExtensionRunError::ExtensionNotFound)?;
        if application.workflow_trigger_type != Some(domain::WorkflowTriggerType::Extension) {
            return Err(WorkflowExtensionRunError::TriggerTypeMismatch);
        }
        let extension = publication.mapping_snapshot.extension.as_ref().ok_or(WorkflowExtensionRunError::InvalidMapping)?;

        let compiled_plan = self
            .repository
            .get_application_compiled_plan(publication.compiled_plan_id)
            .await
            .map_err(|_| WorkflowExtensionRunError::ApplicationNotPublished)?
            .ok_or(WorkflowExtensionRunError::ApplicationNotPublished)?;
        let start_node_id = public_compiled_plan_start_node_id(&compiled_plan.plan)
            .ok_or(WorkflowExtensionRunError::InvalidMapping)?;
        let environment_variables = self
            .repository
            .list_application_environment_variables(workspace_id, operation.application_id)
            .await
            .map_err(|_| WorkflowExtensionRunError::InvalidMapping)?;

        let mut parameters = command.parameters;
        parameters.path = path;
        let start_contract = parse_workflow_start_http_inputs(&publication.document_snapshot)
            .map_err(|_| WorkflowExtensionRunError::InvalidMapping)?;
        let node_input_payload = build_workflow_start_node_input_payload(&start_contract, &parameters)
            .map_err(|_| WorkflowExtensionRunError::InvalidMapping)?;
        let input_payload = public_freeze_workflow_run_input_environment(
            node_input_payload,
            &environment_variables,
            WorkflowRunTriggerContext::Extension,
        )
        .map_err(|_| WorkflowExtensionRunError::InvalidMapping)?;
        let started_at = OffsetDateTime::now_utc();
        let created = self
            .repository
            .create_published_flow_run(&CreateFlowRunInput {
                actor_user_id,
                application_id: operation.application_id,
                flow_id: publication.flow_id,
                flow_draft_id: compiled_plan.draft_id,
                compiled_plan_id: publication.compiled_plan_id,
                debug_session_id: String::new(),
                flow_schema_version: publication.flow_schema_version.clone(),
                document_hash: publication.document_hash.clone(),
                run_mode: domain::FlowRunMode::WorkflowHttpRun,
                target_node_id: None,
                title: build_flow_run_title(None, &format!("Workflow HTTP {}", operation.route_template)),
                status: domain::FlowRunStatus::Queued,
                input_payload: input_payload.clone(),
                started_at,
                api_key_id,
                publication_version_id: Some(publication.id),
                external_user: None,
                external_conversation_id: None,
                external_trace_id: Some(format!("workflow-http:{}", operation.interface_id)),
                compatibility_mode: Some("workflow_http_v1".to_string()),
                idempotency_key: None,
            })
            .await
            .map_err(|_| WorkflowExtensionRunError::InvalidMapping)?;
        let flow_run = created.flow_run;

        self.repository
            .append_published_run_event(&crate::ports::AppendRunEventInput {
                flow_run_id: flow_run.id,
                node_run_id: None,
                event_type: "workflow_extension_run_started".to_string(),
                payload: json!({
                    "api_key_id": api_key_id,
                    "principal": operation.access_policy.as_str(),
                    "application_id": operation.application_id,
                    "publication_version_id": publication.id,
                    "trigger_source": "workflow_extension",
                    "route_template": operation.route_template,
                    "method": command.method.as_str(),
                    "response_mode": extension.response_mode.as_str(),
                }),
            })
            .await
            .map_err(|_| WorkflowExtensionRunError::InvalidMapping)?;

        Ok(WorkflowExtensionRunResult {
            id: flow_run.id,
            application_id: flow_run.application_id,
            api_key_id,
            publication_version_id: publication.id,
            status: flow_run.status,
            response_mode: extension.response_mode,
            sync_timeout_ms: workflow_sync_timeout_ms(&compiled_plan.plan, &start_node_id),
            node_input_payload: input_payload,
            created_at: flow_run.created_at,
        })
    }

}

fn map_operation_error(error: PublishedWorkflowOperationError) -> WorkflowExtensionRunError {
    match error {
        PublishedWorkflowOperationError::NotFound => WorkflowExtensionRunError::ExtensionNotFound,
        PublishedWorkflowOperationError::RouteConflict => WorkflowExtensionRunError::RouteConflict,
        PublishedWorkflowOperationError::InvalidContract
        | PublishedWorkflowOperationError::PathFieldsMismatch => WorkflowExtensionRunError::InvalidMapping,
    }
}

fn workflow_sync_timeout_ms(plan: &Value, start_node_id: &str) -> u64 {
    plan.get("nodes")
        .and_then(|nodes| nodes.get(start_node_id))
        .and_then(|node| node.get("config"))
        .and_then(|config| config.get("sync_timeout_ms"))
        .and_then(Value::as_u64)
        .unwrap_or(u64::from(domain::WORKFLOW_SYNC_TIMEOUT_MS))
}
