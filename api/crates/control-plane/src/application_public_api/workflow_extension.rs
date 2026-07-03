use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use time::OffsetDateTime;
use uuid::Uuid;

use super::{
    api_keys::ApplicationApiKeyService,
    mapping::{
        WorkflowExtensionHttpMethod, WorkflowExtensionParameterSource,
        WorkflowExtensionResponseMode,
    },
    native::write_selector,
    publications::ApplicationPublicationVersionRecord,
    run_service::{
        public_compiled_plan_start_node_id, public_freeze_run_input_environment,
        ApplicationPublishedFlowRunRepository,
    },
};
use crate::{
    flow_run_title::build_flow_run_title,
    ports::{
        ApiKeyRepository, ApplicationCompiledPlanRepository, ApplicationPublicationRepository,
        ApplicationRepository, AuthRepository, CacheStore, CreateFlowRunInput,
    },
};

#[derive(Debug, Clone)]
pub struct CreateWorkflowExtensionRunCommand {
    pub bearer_token: String,
    pub slug: String,
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
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowExtensionRunResult {
    pub id: Uuid,
    pub application_id: Uuid,
    pub api_key_id: Uuid,
    pub publication_version_id: Uuid,
    pub status: domain::FlowRunStatus,
    pub response_mode: WorkflowExtensionResponseMode,
    pub sync_timeout_ms: u64,
    pub node_input_payload: Value,
    pub created_at: OffsetDateTime,
}

pub struct WorkflowExtensionRunService<R> {
    repository: R,
    last_used_cache: Option<std::sync::Arc<dyn CacheStore>>,
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
        Self {
            repository,
            last_used_cache: None,
        }
    }

    pub fn with_last_used_cache(mut self, cache: std::sync::Arc<dyn CacheStore>) -> Self {
        self.last_used_cache = Some(cache);
        self
    }

    pub async fn create_run(
        &self,
        command: CreateWorkflowExtensionRunCommand,
    ) -> Result<WorkflowExtensionRunResult, WorkflowExtensionRunError> {
        let actor = self
            .api_key_service()
            .authenticate_bearer_token(&command.bearer_token)
            .await
            .map_err(|_| WorkflowExtensionRunError::NotAuthenticated)?;
        let publication = self
            .repository
            .load_active_application_publication_by_extension_slug(&command.slug)
            .await
            .map_err(|_| WorkflowExtensionRunError::ExtensionNotFound)?
            .ok_or(WorkflowExtensionRunError::ExtensionNotFound)?;
        if !publication.api_enabled {
            return Err(WorkflowExtensionRunError::ApplicationNotPublished);
        }
        if publication.application_id != actor.application_id {
            return Err(WorkflowExtensionRunError::Forbidden);
        }
        let application = self
            .repository
            .get_application(actor.workspace_id, actor.application_id)
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
        if extension.method != command.method {
            return Err(WorkflowExtensionRunError::MethodNotAllowed);
        }

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
            .list_application_environment_variables(actor.workspace_id, actor.application_id)
            .await
            .map_err(|_| WorkflowExtensionRunError::InvalidMapping)?;

        let node_input_payload = map_extension_parameters(&publication, &command.parameters)
            .map_err(|_| WorkflowExtensionRunError::InvalidMapping)?;
        let input_payload = public_freeze_run_input_environment(
            node_input_payload,
            &environment_variables,
            None,
            Some(&start_node_id),
        );
        let started_at = OffsetDateTime::now_utc();
        let created = self
            .repository
            .create_published_flow_run(&CreateFlowRunInput {
                actor_user_id: actor.creator_user_id,
                application_id: actor.application_id,
                flow_id: publication.flow_id,
                flow_draft_id: compiled_plan.draft_id,
                compiled_plan_id: publication.compiled_plan_id,
                debug_session_id: String::new(),
                flow_schema_version: publication.flow_schema_version.clone(),
                document_hash: publication.document_hash.clone(),
                run_mode: domain::FlowRunMode::PublishedApiRun,
                target_node_id: None,
                title: build_flow_run_title(None, &format!("Workflow extension {}", command.slug)),
                status: domain::FlowRunStatus::Queued,
                input_payload: input_payload.clone(),
                started_at,
                api_key_id: Some(actor.api_key_id),
                publication_version_id: Some(publication.id),
                external_user: None,
                external_conversation_id: None,
                external_trace_id: Some(format!("workflow-extension:{}", command.slug)),
                compatibility_mode: Some("workflow_extension_v1".to_string()),
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
                    "api_key_id": actor.api_key_id,
                    "application_id": actor.application_id,
                    "publication_version_id": publication.id,
                    "trigger_source": "workflow_extension",
                    "slug": command.slug,
                    "method": command.method.as_str(),
                    "response_mode": extension.response_mode.as_str(),
                }),
            })
            .await
            .map_err(|_| WorkflowExtensionRunError::InvalidMapping)?;

        Ok(WorkflowExtensionRunResult {
            id: flow_run.id,
            application_id: flow_run.application_id,
            api_key_id: actor.api_key_id,
            publication_version_id: publication.id,
            status: flow_run.status,
            response_mode: extension.response_mode,
            sync_timeout_ms: workflow_sync_timeout_ms(&compiled_plan.plan, &start_node_id),
            node_input_payload: input_payload,
            created_at: flow_run.created_at,
        })
    }

    fn api_key_service(&self) -> ApplicationApiKeyService<R> {
        let service = ApplicationApiKeyService::new(self.repository.clone());
        match &self.last_used_cache {
            Some(cache) => service.with_last_used_cache(cache.clone()),
            None => service,
        }
    }
}

fn map_extension_parameters(
    publication: &ApplicationPublicationVersionRecord,
    parameters: &WorkflowExtensionRequestParameters,
) -> Result<Value, ()> {
    let extension = publication.mapping_snapshot.extension.as_ref().ok_or(())?;
    let mut node_input_payload = Value::Object(Map::new());
    for parameter in &extension.parameters {
        let value = match parameter.source {
            WorkflowExtensionParameterSource::Path => parameters.path.get(&parameter.name),
            WorkflowExtensionParameterSource::Query => parameters.query.get(&parameter.name),
            WorkflowExtensionParameterSource::Form => parameters.form.get(&parameter.name),
            WorkflowExtensionParameterSource::Body => parameters
                .body
                .as_object()
                .and_then(|body| body.get(&parameter.name)),
        }
        .cloned()
        .ok_or(())?;
        write_selector(&mut node_input_payload, &parameter.target, value).map_err(|_| ())?;
    }
    Ok(node_input_payload)
}

fn workflow_sync_timeout_ms(plan: &Value, start_node_id: &str) -> u64 {
    plan.get("nodes")
        .and_then(|nodes| nodes.get(start_node_id))
        .and_then(|node| node.get("config"))
        .and_then(|config| config.get("sync_timeout_ms"))
        .and_then(Value::as_u64)
        .unwrap_or(u64::from(domain::WORKFLOW_SYNC_TIMEOUT_MS))
}
