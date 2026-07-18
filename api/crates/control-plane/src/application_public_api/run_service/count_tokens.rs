use std::sync::Arc;

use plugin_framework::{
    provider_contract::{
        ProviderCountTokensError, ProviderCountTokensInput, ProviderMessage, ProviderMessageRole,
        ProviderWireOperation,
    },
    provider_package::{ProviderConfigField, ProviderPackage},
};
use serde_json::{Map, Value};
use uuid::Uuid;

use super::super::{
    api_keys::{ApplicationApiKeyActor, ApplicationApiKeyService},
    native::NativeRunRequest,
    publications::ApplicationPublicationVersionRecord,
};
use super::{
    PublishedProviderManifestCapabilityRepository, PublishedRouteResolutionError,
    PublishedRouteResolver, ResolvedCountTokensProviderRoute,
};
use crate::ports::{
    ApiKeyRepository, ApplicationCompiledPlanRepository, ApplicationPublicationRepository,
    ApplicationRepository, AuthRepository, CacheStore, ModelProviderRepository, PluginRepository,
    ProviderRuntimePort,
};

#[derive(Debug, Clone)]
pub struct CountTokensCommand {
    pub bearer_token: String,
    pub request: NativeRunRequest,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublishedCountTokensResult {
    pub input_tokens: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PublishedCountTokensError {
    NotAuthenticated,
    ApplicationNotPublished,
    RouteUnavailable(PublishedRouteResolutionError),
    InvalidRequest,
    ProviderTargetUnavailable,
    Provider(ProviderCountTokensError),
}

pub struct ApplicationPublishedCountTokensService<R, H> {
    repository: R,
    runtime: H,
    provider_secret_master_key: String,
    last_used_cache: Option<Arc<dyn CacheStore>>,
}

impl<R, H> ApplicationPublishedCountTokensService<R, H>
where
    R: ApplicationRepository
        + ApiKeyRepository
        + AuthRepository
        + ApplicationPublicationRepository
        + ApplicationCompiledPlanRepository
        + ModelProviderRepository
        + PluginRepository
        + PublishedProviderManifestCapabilityRepository
        + Clone,
    H: ProviderRuntimePort,
{
    pub fn new(repository: R, runtime: H, provider_secret_master_key: impl Into<String>) -> Self {
        Self {
            repository,
            runtime,
            provider_secret_master_key: provider_secret_master_key.into(),
            last_used_cache: None,
        }
    }

    pub fn with_last_used_cache(mut self, cache: Arc<dyn CacheStore>) -> Self {
        self.last_used_cache = Some(cache);
        self
    }

    pub async fn count_tokens(
        &self,
        command: CountTokensCommand,
    ) -> Result<PublishedCountTokensResult, PublishedCountTokensError> {
        let actor = self
            .api_key_service()
            .authenticate_bearer_token(&command.bearer_token)
            .await
            .map_err(|_| PublishedCountTokensError::NotAuthenticated)?;
        self.ensure_application_exists(&actor).await?;

        let publication = self.load_enabled_publication(&actor).await?;
        let compiled_plan = self
            .repository
            .get_application_compiled_plan(publication.compiled_plan_id)
            .await
            .map_err(|_| PublishedCountTokensError::ApplicationNotPublished)?
            .ok_or(PublishedCountTokensError::ApplicationNotPublished)?;
        let route = PublishedRouteResolver::new(&self.repository)
            .resolve_count_tokens(actor.workspace_id, &publication, &compiled_plan)
            .await
            .map_err(PublishedCountTokensError::RouteUnavailable)?;
        let (installation, provider_config) = self
            .load_count_tokens_target(actor.workspace_id, &route)
            .await?;
        let input = ProviderCountTokensInput {
            operation: ProviderWireOperation::CountTokens,
            contract_version: Default::default(),
            provider_instance_id: route.llm_runtime.provider_instance_id.clone(),
            provider_code: route.llm_runtime.provider_code.clone(),
            protocol: route.llm_runtime.protocol.clone(),
            model: route.llm_runtime.model.clone(),
            provider_config,
            messages: count_tokens_messages(&command.request)?,
            system: command.request.system,
            request_context: command.request.request_context,
            required_capabilities: Default::default(),
            client_protocol_envelope: command.request.client_protocol_envelope,
        };
        let result = self
            .runtime
            .count_tokens(&installation, input)
            .await
            .map_err(published_count_tokens_runtime_error)?;

        Ok(PublishedCountTokensResult {
            input_tokens: result.input_tokens,
        })
    }

    fn api_key_service(&self) -> ApplicationApiKeyService<R> {
        let mut service = ApplicationApiKeyService::new(self.repository.clone());
        if let Some(cache) = &self.last_used_cache {
            service = service.with_last_used_cache(cache.clone());
        }
        service
    }

    async fn ensure_application_exists(
        &self,
        actor: &ApplicationApiKeyActor,
    ) -> Result<(), PublishedCountTokensError> {
        self.repository
            .get_application(actor.workspace_id, actor.application_id)
            .await
            .map_err(|_| PublishedCountTokensError::ApplicationNotPublished)?
            .ok_or(PublishedCountTokensError::ApplicationNotPublished)?;
        Ok(())
    }

    async fn load_enabled_publication(
        &self,
        actor: &ApplicationApiKeyActor,
    ) -> Result<ApplicationPublicationVersionRecord, PublishedCountTokensError> {
        let publication = self
            .repository
            .load_active_application_publication(actor.application_id)
            .await
            .map_err(|_| PublishedCountTokensError::ApplicationNotPublished)?;
        publication
            .filter(|publication| publication.api_enabled)
            .ok_or(PublishedCountTokensError::ApplicationNotPublished)
    }

    async fn load_count_tokens_target(
        &self,
        workspace_id: Uuid,
        route: &ResolvedCountTokensProviderRoute,
    ) -> Result<(domain::PluginInstallationRecord, Value), PublishedCountTokensError> {
        let provider_instance_id = Uuid::parse_str(&route.llm_runtime.provider_instance_id)
            .map_err(|_| PublishedCountTokensError::ProviderTargetUnavailable)?;
        let instance = self
            .repository
            .get_instance(workspace_id, provider_instance_id)
            .await
            .map_err(|_| PublishedCountTokensError::ProviderTargetUnavailable)?
            .ok_or(PublishedCountTokensError::ProviderTargetUnavailable)?;
        if instance.provider_code != route.llm_runtime.provider_code
            || instance.protocol != route.llm_runtime.protocol
            || instance.status != domain::ModelProviderInstanceStatus::Ready
            || !instance.included_in_main
            || (!instance.enabled_model_ids.is_empty()
                && !instance
                    .enabled_model_ids
                    .iter()
                    .any(|model_id| model_id == &route.llm_runtime.model))
        {
            return Err(PublishedCountTokensError::ProviderTargetUnavailable);
        }

        let installation = self
            .repository
            .get_installation(instance.installation_id)
            .await
            .map_err(|_| PublishedCountTokensError::ProviderTargetUnavailable)?
            .ok_or(PublishedCountTokensError::ProviderTargetUnavailable)?;
        let assigned = self
            .repository
            .list_assignments(workspace_id)
            .await
            .map_err(|_| PublishedCountTokensError::ProviderTargetUnavailable)?
            .into_iter()
            .any(|assignment| assignment.installation_id == installation.id);
        if !assigned
            || installation.desired_state == domain::PluginDesiredState::Disabled
            || installation.availability_status != domain::PluginAvailabilityStatus::Available
        {
            return Err(PublishedCountTokensError::ProviderTargetUnavailable);
        }

        let package = ProviderPackage::load_from_dir(&installation.installed_path)
            .map_err(|_| PublishedCountTokensError::ProviderTargetUnavailable)?;
        let provider_config = count_tokens_provider_config(
            &self.repository,
            &self.provider_secret_master_key,
            &package,
            &instance,
        )
        .await?;

        Ok((installation, provider_config))
    }
}

async fn count_tokens_provider_config<R>(
    repository: &R,
    provider_secret_master_key: &str,
    package: &ProviderPackage,
    instance: &domain::ModelProviderInstanceRecord,
) -> Result<Value, PublishedCountTokensError>
where
    R: ModelProviderRepository,
{
    let secret_json = repository
        .get_secret_json(instance.id, provider_secret_master_key)
        .await
        .map_err(|_| PublishedCountTokensError::ProviderTargetUnavailable)?
        .unwrap_or_else(empty_object);
    validate_count_tokens_required_fields(
        &package.provider.form_schema,
        &instance.config_json,
        &secret_json,
    )?;
    merge_count_tokens_config(&instance.config_json, &secret_json)
}

fn validate_count_tokens_required_fields(
    form_schema: &[ProviderConfigField],
    public_config: &Value,
    secret_config: &Value,
) -> Result<(), PublishedCountTokensError> {
    let public_object = public_config
        .as_object()
        .ok_or(PublishedCountTokensError::ProviderTargetUnavailable)?;
    let secret_object = secret_config
        .as_object()
        .ok_or(PublishedCountTokensError::ProviderTargetUnavailable)?;
    for field in form_schema.iter().filter(|field| field.required) {
        let value = if field.field_type.trim().eq_ignore_ascii_case("secret") {
            secret_object.get(&field.key)
        } else {
            public_object.get(&field.key)
        };
        if value.is_none()
            || value == Some(&Value::Null)
            || value == Some(&Value::String(String::new()))
        {
            return Err(PublishedCountTokensError::ProviderTargetUnavailable);
        }
    }
    Ok(())
}

fn merge_count_tokens_config(
    public_config: &Value,
    secret_config: &Value,
) -> Result<Value, PublishedCountTokensError> {
    let mut config = public_config
        .as_object()
        .cloned()
        .ok_or(PublishedCountTokensError::ProviderTargetUnavailable)?;
    let secret_config = secret_config
        .as_object()
        .ok_or(PublishedCountTokensError::ProviderTargetUnavailable)?;
    for (key, value) in secret_config {
        config.insert(key.clone(), value.clone());
    }
    Ok(Value::Object(config))
}

fn empty_object() -> Value {
    Value::Object(Map::new())
}

fn count_tokens_messages(
    request: &NativeRunRequest,
) -> Result<Vec<ProviderMessage>, PublishedCountTokensError> {
    let mut messages = request
        .history
        .iter()
        .map(count_tokens_history_message)
        .collect::<Result<Vec<_>, _>>()?;
    messages.push(ProviderMessage {
        role: ProviderMessageRole::User,
        content: request.query.clone(),
        name: None,
        tool_call_id: None,
        is_error: None,
        tool_calls: None,
        content_blocks: None,
    });
    Ok(messages)
}

fn count_tokens_history_message(
    value: &Value,
) -> Result<ProviderMessage, PublishedCountTokensError> {
    let object = value
        .as_object()
        .ok_or(PublishedCountTokensError::InvalidRequest)?;
    let role = match object.get("role").and_then(Value::as_str) {
        Some("system") => ProviderMessageRole::System,
        Some("user") => ProviderMessageRole::User,
        Some("assistant") => ProviderMessageRole::Assistant,
        _ => return Err(PublishedCountTokensError::InvalidRequest),
    };
    let content = object
        .get("content")
        .and_then(Value::as_str)
        .ok_or(PublishedCountTokensError::InvalidRequest)?;
    Ok(ProviderMessage {
        role,
        content: content.to_string(),
        name: None,
        tool_call_id: None,
        is_error: None,
        tool_calls: None,
        content_blocks: object.get("content_blocks").cloned(),
    })
}

fn published_count_tokens_runtime_error(error: anyhow::Error) -> PublishedCountTokensError {
    error
        .downcast_ref::<ProviderCountTokensError>()
        .cloned()
        .map(PublishedCountTokensError::Provider)
        .unwrap_or(PublishedCountTokensError::ProviderTargetUnavailable)
}
