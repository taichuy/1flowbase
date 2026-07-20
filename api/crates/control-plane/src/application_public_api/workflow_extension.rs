use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use thiserror::Error;
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

#[derive(Debug, Error)]
pub enum WorkflowExtensionRunError {
    #[error("workflow extension request is not authenticated")]
    NotAuthenticated,
    #[error("workflow extension was not found")]
    ExtensionNotFound,
    #[error("workflow application is not published")]
    ApplicationNotPublished,
    #[error("workflow extension invocation is forbidden")]
    Forbidden,
    #[error("workflow extension method is not allowed")]
    MethodNotAllowed,
    #[error("workflow trigger type does not support extension invocation")]
    TriggerTypeMismatch,
    #[error("workflow extension input contract is invalid")]
    InvalidMapping,
    #[error("workflow extension route contract is ambiguous")]
    RouteConflict,
    #[error("workflow extension service failed")]
    Internal(#[source] anyhow::Error),
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
                .map_err(WorkflowExtensionRunError::Internal)?,
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
                    .map_err(map_authentication_error)?;
                if user_api_key.actor.current_workspace_id != operation.workspace_id {
                    return Err(WorkflowExtensionRunError::Forbidden);
                }
                let application = self
                    .repository
                    .get_application(operation.workspace_id, operation.application_id)
                    .await
                    .map_err(WorkflowExtensionRunError::Internal)?
                    .ok_or(WorkflowExtensionRunError::ExtensionNotFound)?;
                ensure_application_view_permission(
                    &self.repository,
                    &user_api_key.actor,
                    &application,
                )
                .await
                .map_err(map_authorization_error)?;
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
            .map_err(WorkflowExtensionRunError::Internal)?
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
            .map_err(map_invocation_error)?;
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
            sync_timeout_ms: workflow_sync_timeout_ms(&invoked.compiled_plan.plan, &start_node_id),
            node_input_payload: flow_run.input_payload.clone(),
            created_at: flow_run.created_at,
        })
    }
}

fn map_authentication_error(error: anyhow::Error) -> WorkflowExtensionRunError {
    if error
        .downcast_ref::<crate::errors::ControlPlaneError>()
        .is_some_and(|error| matches!(error, crate::errors::ControlPlaneError::NotAuthenticated))
    {
        WorkflowExtensionRunError::NotAuthenticated
    } else {
        WorkflowExtensionRunError::Internal(error)
    }
}

fn map_authorization_error(error: anyhow::Error) -> WorkflowExtensionRunError {
    if error
        .downcast_ref::<crate::errors::ControlPlaneError>()
        .is_some_and(|error| matches!(error, crate::errors::ControlPlaneError::PermissionDenied(_)))
    {
        WorkflowExtensionRunError::Forbidden
    } else {
        WorkflowExtensionRunError::Internal(error)
    }
}

fn map_invocation_error(error: WorkflowInvocationError) -> WorkflowExtensionRunError {
    match error {
        WorkflowInvocationError::CompiledPlanUnavailable => {
            WorkflowExtensionRunError::ApplicationNotPublished
        }
        WorkflowInvocationError::InvalidInput(_) => WorkflowExtensionRunError::InvalidMapping,
        WorkflowInvocationError::Repository(error) => WorkflowExtensionRunError::Internal(error),
    }
}

fn map_operation_error(error: PublishedWorkflowOperationError) -> WorkflowExtensionRunError {
    match error {
        PublishedWorkflowOperationError::NotFound => WorkflowExtensionRunError::ExtensionNotFound,
        PublishedWorkflowOperationError::RouteConflict => WorkflowExtensionRunError::RouteConflict,
        PublishedWorkflowOperationError::InvalidContract
        | PublishedWorkflowOperationError::PathFieldsMismatch => {
            WorkflowExtensionRunError::InvalidMapping
        }
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

#[cfg(test)]
mod error_taxonomy_tests {
    use super::*;

    #[test]
    fn invocation_contract_errors_remain_client_errors() {
        assert!(matches!(
            map_invocation_error(WorkflowInvocationError::CompiledPlanUnavailable),
            WorkflowExtensionRunError::ApplicationNotPublished
        ));
        assert!(matches!(
            map_invocation_error(WorkflowInvocationError::InvalidInput(anyhow::anyhow!(
                "invalid workflow start input"
            ))),
            WorkflowExtensionRunError::InvalidMapping
        ));
    }

    #[test]
    fn invocation_repository_errors_remain_internal_errors_with_their_source() {
        let mapped = map_invocation_error(WorkflowInvocationError::Repository(anyhow::anyhow!(
            "append run event failed"
        )));

        match mapped {
            WorkflowExtensionRunError::Internal(error) => {
                assert_eq!(error.to_string(), "append run event failed");
            }
            other => panic!("expected internal error, got {other:?}"),
        }
    }

    #[test]
    fn authentication_only_maps_explicit_auth_failures_to_unauthorized() {
        let unauthenticated =
            map_authentication_error(crate::errors::ControlPlaneError::NotAuthenticated.into());
        assert!(matches!(
            unauthenticated,
            WorkflowExtensionRunError::NotAuthenticated
        ));

        let repository_failure = map_authentication_error(anyhow::anyhow!("auth store failed"));
        assert!(matches!(
            repository_failure,
            WorkflowExtensionRunError::Internal(_)
        ));
    }
}
