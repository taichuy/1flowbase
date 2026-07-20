use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use time::OffsetDateTime;
use uuid::Uuid;

use super::{
    mapping::{
        WorkflowExtensionAccessPolicy, WorkflowExtensionHttpMethod, WorkflowExtensionResponseMode,
    },
    published_workflow_operation::{
        build_published_workflow_operations, resolve_published_workflow_operation,
        PublishedWorkflowOperationError,
    },
    run_service::{public_compiled_plan_start_node_id, ApplicationPublishedFlowRunRepository},
    workflow_invocation::{
        InvokeWorkflowCommand, WorkflowInvocationError, WorkflowInvocationService,
        WorkflowInvocationTrigger,
    },
    workflow_start_http_inputs::{
        build_workflow_start_node_input_payload, parse_workflow_start_http_inputs,
    },
};
use crate::{
    application_public_api::ensure_application_view_permission,
    auth::ApiKeyService,
    ports::{
        ApiKeyRepository, ApplicationCompiledPlanRepository, ApplicationPublicationRepository,
        ApplicationRepository, AuthRepository,
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
        let extension = publication
            .mapping_snapshot
            .extension
            .as_ref()
            .ok_or(WorkflowExtensionRunError::InvalidMapping)?;

        let mut parameters = command.parameters;
        parameters.path = path;
        let start_contract = parse_workflow_start_http_inputs(&publication.document_snapshot)
            .map_err(|_| WorkflowExtensionRunError::InvalidMapping)?;
        let node_input_payload =
            build_workflow_start_node_input_payload(&start_contract, &parameters)
                .map_err(|_| WorkflowExtensionRunError::InvalidMapping)?;
        let response_mode = extension.response_mode;
        let invoked = WorkflowInvocationService::new(self.repository.clone())
            .invoke(InvokeWorkflowCommand {
                actor_user_id,
                publication: publication.clone(),
                node_input_payload,
                trigger: WorkflowInvocationTrigger::Http {
                    api_key_id,
                    interface_id: operation.interface_id.clone(),
                    route_template: operation.route_template.clone(),
                    method: command.method.as_str().to_string(),
                    principal: operation.access_policy.as_str().to_string(),
                    response_mode: response_mode.as_str().to_string(),
                },
            })
            .await
            .map_err(|error| match error {
                WorkflowInvocationError::CompiledPlanUnavailable => {
                    WorkflowExtensionRunError::ApplicationNotPublished
                }
                WorkflowInvocationError::InvalidInvocation(_) => {
                    WorkflowExtensionRunError::InvalidMapping
                }
            })?;
        let flow_run = invoked.flow_run;
        let start_node_id = public_compiled_plan_start_node_id(&invoked.compiled_plan.plan)
            .ok_or(WorkflowExtensionRunError::InvalidMapping)?;

        Ok(WorkflowExtensionRunResult {
            id: flow_run.id,
            application_id: flow_run.application_id,
            api_key_id,
            publication_version_id: publication.id,
            status: flow_run.status,
            response_mode,
            sync_timeout_ms: workflow_sync_timeout_ms(
                &invoked.compiled_plan.plan,
                &start_node_id,
            ),
            node_input_payload: flow_run.input_payload.clone(),
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
