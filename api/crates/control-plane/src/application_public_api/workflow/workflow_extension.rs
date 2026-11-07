use std::collections::BTreeMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use thiserror::Error;
use time::OffsetDateTime;
use uuid::Uuid;

use super::{
    mapping::{WorkflowExtensionHttpMethod, WorkflowExtensionResponseMode},
    published_workflow_operation::{
        build_published_workflow_operations, resolve_published_workflow_operation,
        PublishedWorkflowOperation, PublishedWorkflowOperationError,
    },
    run_service::{public_compiled_plan_start_node_id, ApplicationPublishedFlowRunRepository},
    workflow_invocation::{
        InvokeWorkflowCommand, WorkflowInvocationError, WorkflowInvocationService,
        WorkflowInvocationTrigger,
    },
};
use crate::{
    application_public_api::ensure_application_view_permission,
    ports::{
        ApplicationCompiledPlanRepository, ApplicationPublicationRepository, ApplicationRepository,
        AuthRepository, PublishedPlanCache,
    },
};

#[derive(Debug, Clone)]
pub struct CreateWorkflowExtensionRunCommand {
    pub actor: domain::ActorContext,
    pub principal: WorkflowHttpPrincipal,
    pub request_path: String,
    pub method: WorkflowExtensionHttpMethod,
    pub parameters: WorkflowExtensionRequestParameters,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkflowHttpPrincipal {
    User,
    UserApiKey { api_key_id: Uuid },
}

impl WorkflowHttpPrincipal {
    fn api_key_id(self) -> Option<Uuid> {
        match self {
            Self::User => None,
            Self::UserApiKey { api_key_id } => Some(api_key_id),
        }
    }

    fn kind(self) -> domain::FlowRunPrincipalKind {
        match self {
            Self::User => domain::FlowRunPrincipalKind::User,
            Self::UserApiKey { .. } => domain::FlowRunPrincipalKind::UserApiKey,
        }
    }
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

/// Proof returned only after the existing workspace/application authorization owner succeeds.
/// The publication is immutable and retained for preparation, observation and execution.
#[derive(Debug, Clone)]
pub struct AuthorizedPublishedWorkflowOperation {
    operation: Arc<PublishedWorkflowOperation>,
    actor_user_id: Uuid,
    workspace_id: Uuid,
    generation: String,
}
impl AuthorizedPublishedWorkflowOperation {
    pub fn operation(&self) -> &PublishedWorkflowOperation {
        &self.operation
    }
    pub fn operation_snapshot(&self) -> Arc<PublishedWorkflowOperation> {
        self.operation.clone()
    }
    pub fn workspace_id(&self) -> Uuid {
        self.workspace_id
    }
    pub fn generation(&self) -> &str {
        &self.generation
    }
    pub fn prepare_input(
        &self,
        parameters: &WorkflowExtensionRequestParameters,
        request_path: &str,
    ) -> Result<Value, WorkflowExtensionRunError> {
        self.operation.prepare_input(parameters, request_path)
    }
}
#[derive(Debug)]
pub struct PreparedWorkflowExtensionRun {
    authorized: AuthorizedPublishedWorkflowOperation,
    principal: WorkflowHttpPrincipal,
    node_input_payload: Value,
}
impl PreparedWorkflowExtensionRun {
    pub fn operation(&self) -> &PublishedWorkflowOperation {
        self.authorized.operation()
    }
    pub fn node_input_payload(&self) -> &Value {
        &self.node_input_payload
    }
}

pub struct WorkflowExtensionRunService<R> {
    repository: R,
    published_plan_cache: Option<Arc<dyn PublishedPlanCache>>,
}

impl<R> WorkflowExtensionRunService<R>
where
    R: ApplicationRepository
        + AuthRepository
        + ApplicationPublicationRepository
        + ApplicationCompiledPlanRepository
        + ApplicationPublishedFlowRunRepository
        + Clone
        + 'static,
{
    pub fn new(repository: R) -> Self {
        Self {
            repository,
            published_plan_cache: None,
        }
    }

    pub fn with_published_plan_cache(mut self, cache: Arc<dyn PublishedPlanCache>) -> Self {
        self.published_plan_cache = Some(cache);
        self
    }

    pub async fn authorize_operation(
        &self,
        actor: &domain::ActorContext,
        method: WorkflowExtensionHttpMethod,
        request_path: &str,
    ) -> Result<AuthorizedPublishedWorkflowOperation, WorkflowExtensionRunError> {
        let operations = build_published_workflow_operations(
            self.repository
                .list_enabled_extension_publications()
                .await
                .map_err(WorkflowExtensionRunError::Internal)?,
        )
        .map_err(map_operation_error)?;
        if operations.iter().any(|operation| {
            operation.method != method && operation.match_path(request_path).is_some()
        }) && !operations.iter().any(|operation| {
            operation.method == method && operation.match_path(request_path).is_some()
        }) {
            return Err(WorkflowExtensionRunError::MethodNotAllowed);
        }
        let (operation, _) =
            resolve_published_workflow_operation(&operations, method, request_path)
                .map_err(map_operation_error)?;
        self.authorize_resolved_operation(actor, operation).await
    }

    pub async fn authorize_operation_id(
        &self,
        actor: &domain::ActorContext,
        operation_id: &str,
    ) -> Result<AuthorizedPublishedWorkflowOperation, WorkflowExtensionRunError> {
        let operations = build_published_workflow_operations(
            self.repository
                .list_enabled_extension_publications()
                .await
                .map_err(WorkflowExtensionRunError::Internal)?,
        )
        .map_err(map_operation_error)?;
        let operation = operations
            .into_iter()
            .find(|operation| operation.interface_id == operation_id)
            .ok_or(WorkflowExtensionRunError::ExtensionNotFound)?;
        self.authorize_resolved_operation(actor, operation).await
    }

    async fn authorize_resolved_operation(
        &self,
        actor: &domain::ActorContext,
        operation: PublishedWorkflowOperation,
    ) -> Result<AuthorizedPublishedWorkflowOperation, WorkflowExtensionRunError> {
        if actor.current_workspace_id != operation.workspace_id {
            return Err(WorkflowExtensionRunError::Forbidden);
        }
        let application = self
            .repository
            .get_application(actor.current_workspace_id, operation.application_id)
            .await
            .map_err(WorkflowExtensionRunError::Internal)?
            .ok_or(WorkflowExtensionRunError::ExtensionNotFound)?;
        ensure_application_view_permission(&self.repository, actor, &application)
            .await
            .map_err(map_authorization_error)?;
        if application.workflow_trigger_type != Some(domain::WorkflowTriggerType::Extension) {
            return Err(WorkflowExtensionRunError::TriggerTypeMismatch);
        }
        let generation = operation.snapshot_generation();
        Ok(AuthorizedPublishedWorkflowOperation {
            generation,
            operation: Arc::new(operation),
            actor_user_id: actor.user_id,
            workspace_id: actor.current_workspace_id,
        })
    }

    pub fn prepare_authorized_run(
        &self,
        authorized: AuthorizedPublishedWorkflowOperation,
        principal: WorkflowHttpPrincipal,
        parameters: &WorkflowExtensionRequestParameters,
        request_path: &str,
    ) -> Result<PreparedWorkflowExtensionRun, WorkflowExtensionRunError> {
        let node_input_payload = authorized.prepare_input(parameters, request_path)?;
        Ok(PreparedWorkflowExtensionRun {
            authorized,
            principal,
            node_input_payload,
        })
    }

    pub async fn create_run(
        &self,
        command: CreateWorkflowExtensionRunCommand,
    ) -> Result<WorkflowExtensionRunResult, WorkflowExtensionRunError> {
        let authorized = self
            .authorize_operation(&command.actor, command.method, &command.request_path)
            .await?;
        let prepared = self.prepare_authorized_run(
            authorized,
            command.principal,
            &command.parameters,
            &command.request_path,
        )?;
        self.create_prepared_run(prepared).await
    }

    pub async fn create_prepared_run(
        &self,
        prepared: PreparedWorkflowExtensionRun,
    ) -> Result<WorkflowExtensionRunResult, WorkflowExtensionRunError> {
        let PreparedWorkflowExtensionRun {
            authorized,
            principal,
            node_input_payload,
            ..
        } = prepared;
        let operation = authorized.operation();
        let publication = operation.publication.clone();
        let response_mode = operation.response_mode;
        let api_key_id = principal.api_key_id();
        let mut invocation = WorkflowInvocationService::new(self.repository.clone());
        if let Some(cache) = &self.published_plan_cache {
            invocation = invocation.with_published_plan_cache(cache.clone());
        }
        let invoked = invocation
            .invoke(InvokeWorkflowCommand {
                actor_user_id: authorized.actor_user_id,
                publication: publication.clone(),
                node_input_payload,
                trigger: WorkflowInvocationTrigger::Http {
                    api_key_id,
                    interface_id: operation.interface_id.clone(),
                    route_template: operation.route_template.clone(),
                    method: operation.method.as_str().to_string(),
                    principal: principal.kind(),
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
}
