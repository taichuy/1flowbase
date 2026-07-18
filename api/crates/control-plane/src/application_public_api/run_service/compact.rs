use std::sync::Arc;

use plugin_framework::{
    provider_contract::{
        ProviderCompactError, ProviderCompactProfile, ProviderCompactResult,
        ProviderInvocationInput, ProviderMessage, ProviderMessageRole, ProviderWireOperation,
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
    PublishedRouteResolver, ResolvedCompactProviderRoute,
};
use crate::ports::{
    ApiKeyRepository, ApplicationCompiledPlanRepository, ApplicationPublicationRepository,
    ApplicationRepository, AuthRepository, CacheStore, ModelProviderRepository, PluginRepository,
    ProviderRuntimePort,
};

/// The HTTP adapter has already classified the request against explicit Codex
/// evidence. This service accepts only the two remote provider profiles.
#[derive(Debug, Clone)]
pub struct CompactCommand {
    pub bearer_token: String,
    pub request: NativeRunRequest,
    pub profile: ProviderCompactProfile,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PublishedCompactResult {
    pub result: ProviderCompactResult,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PublishedCompactError {
    NotAuthenticated,
    ApplicationNotPublished,
    RouteUnavailable(PublishedRouteResolutionError),
    InvalidRequest,
    ProviderTargetUnavailable,
    Provider(ProviderCompactError),
}

/// Resolves one frozen published Compact binding and calls the provider's
/// unary Compact contract. It never creates or starts a flow run.
pub struct ApplicationPublishedCompactService<R, H> {
    repository: R,
    runtime: H,
    provider_secret_master_key: String,
    last_used_cache: Option<Arc<dyn CacheStore>>,
}

impl<R, H> ApplicationPublishedCompactService<R, H>
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

    pub async fn compact(
        &self,
        command: CompactCommand,
    ) -> Result<PublishedCompactResult, PublishedCompactError> {
        let actor = self
            .api_key_service()
            .authenticate_bearer_token(&command.bearer_token)
            .await
            .map_err(|_| PublishedCompactError::NotAuthenticated)?;
        self.ensure_application_exists(&actor).await?;

        let publication = self.load_enabled_publication(&actor).await?;
        let compiled_plan = self
            .repository
            .get_application_compiled_plan(publication.compiled_plan_id)
            .await
            .map_err(|_| PublishedCompactError::ApplicationNotPublished)?
            .ok_or(PublishedCompactError::ApplicationNotPublished)?;
        let route = PublishedRouteResolver::new(&self.repository)
            .resolve_compact(
                actor.workspace_id,
                &publication,
                &compiled_plan,
                command.profile,
            )
            .await
            .map_err(PublishedCompactError::RouteUnavailable)?;
        let (installation, provider_config) =
            self.load_compact_target(actor.workspace_id, &route).await?;
        let input = compact_invocation_input(command.request, route, provider_config)?;
        let result = self
            .runtime
            .compact(&installation, input)
            .await
            .map_err(published_compact_runtime_error)?;

        Ok(PublishedCompactResult { result })
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
    ) -> Result<(), PublishedCompactError> {
        self.repository
            .get_application(actor.workspace_id, actor.application_id)
            .await
            .map_err(|_| PublishedCompactError::ApplicationNotPublished)?
            .ok_or(PublishedCompactError::ApplicationNotPublished)?;
        Ok(())
    }

    async fn load_enabled_publication(
        &self,
        actor: &ApplicationApiKeyActor,
    ) -> Result<ApplicationPublicationVersionRecord, PublishedCompactError> {
        let publication = self
            .repository
            .load_active_application_publication(actor.application_id)
            .await
            .map_err(|_| PublishedCompactError::ApplicationNotPublished)?;
        publication
            .filter(|publication| publication.api_enabled)
            .ok_or(PublishedCompactError::ApplicationNotPublished)
    }

    async fn load_compact_target(
        &self,
        workspace_id: Uuid,
        route: &ResolvedCompactProviderRoute,
    ) -> Result<(domain::PluginInstallationRecord, Value), PublishedCompactError> {
        let provider_instance_id = Uuid::parse_str(&route.llm_runtime.provider_instance_id)
            .map_err(|_| PublishedCompactError::ProviderTargetUnavailable)?;
        let instance = self
            .repository
            .get_instance(workspace_id, provider_instance_id)
            .await
            .map_err(|_| PublishedCompactError::ProviderTargetUnavailable)?
            .ok_or(PublishedCompactError::ProviderTargetUnavailable)?;
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
            return Err(PublishedCompactError::ProviderTargetUnavailable);
        }

        let installation = self
            .repository
            .get_installation(instance.installation_id)
            .await
            .map_err(|_| PublishedCompactError::ProviderTargetUnavailable)?
            .ok_or(PublishedCompactError::ProviderTargetUnavailable)?;
        let assigned = self
            .repository
            .list_assignments(workspace_id)
            .await
            .map_err(|_| PublishedCompactError::ProviderTargetUnavailable)?
            .into_iter()
            .any(|assignment| assignment.installation_id == installation.id);
        if !assigned
            || installation.desired_state == domain::PluginDesiredState::Disabled
            || installation.availability_status != domain::PluginAvailabilityStatus::Available
        {
            return Err(PublishedCompactError::ProviderTargetUnavailable);
        }

        let package = ProviderPackage::load_from_dir(&installation.installed_path)
            .map_err(|_| PublishedCompactError::ProviderTargetUnavailable)?;
        let provider_config = compact_provider_config(
            &self.repository,
            &self.provider_secret_master_key,
            &package,
            &instance,
        )
        .await?;

        Ok((installation, provider_config))
    }
}

fn compact_invocation_input(
    request: NativeRunRequest,
    route: ResolvedCompactProviderRoute,
    provider_config: Value,
) -> Result<ProviderInvocationInput, PublishedCompactError> {
    let messages = compact_messages(&request)?;
    Ok(ProviderInvocationInput {
        operation: ProviderWireOperation::Compact,
        profile: Some(route.profile),
        provider_instance_id: route.llm_runtime.provider_instance_id,
        provider_code: route.llm_runtime.provider_code,
        protocol: route.llm_runtime.protocol,
        model: route.llm_runtime.model,
        provider_config,
        messages,
        system: request.system,
        request_context: request.request_context,
        client_protocol_envelope: request.client_protocol_envelope,
        ..ProviderInvocationInput::default()
    })
}

async fn compact_provider_config<R>(
    repository: &R,
    provider_secret_master_key: &str,
    package: &ProviderPackage,
    instance: &domain::ModelProviderInstanceRecord,
) -> Result<Value, PublishedCompactError>
where
    R: ModelProviderRepository,
{
    let secret_json = repository
        .get_secret_json(instance.id, provider_secret_master_key)
        .await
        .map_err(|_| PublishedCompactError::ProviderTargetUnavailable)?
        .unwrap_or_else(empty_object);
    validate_compact_required_fields(
        &package.provider.form_schema,
        &instance.config_json,
        &secret_json,
    )?;
    merge_compact_config(&instance.config_json, &secret_json)
}

fn validate_compact_required_fields(
    form_schema: &[ProviderConfigField],
    public_config: &Value,
    secret_config: &Value,
) -> Result<(), PublishedCompactError> {
    let public_object = public_config
        .as_object()
        .ok_or(PublishedCompactError::ProviderTargetUnavailable)?;
    let secret_object = secret_config
        .as_object()
        .ok_or(PublishedCompactError::ProviderTargetUnavailable)?;
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
            return Err(PublishedCompactError::ProviderTargetUnavailable);
        }
    }
    Ok(())
}

fn merge_compact_config(
    public_config: &Value,
    secret_config: &Value,
) -> Result<Value, PublishedCompactError> {
    let mut config = public_config
        .as_object()
        .cloned()
        .ok_or(PublishedCompactError::ProviderTargetUnavailable)?;
    let secret_config = secret_config
        .as_object()
        .ok_or(PublishedCompactError::ProviderTargetUnavailable)?;
    for (key, value) in secret_config {
        config.insert(key.clone(), value.clone());
    }
    Ok(Value::Object(config))
}

fn empty_object() -> Value {
    Value::Object(Map::new())
}

fn compact_messages(
    request: &NativeRunRequest,
) -> Result<Vec<ProviderMessage>, PublishedCompactError> {
    let mut messages = request
        .history
        .iter()
        .map(compact_history_message)
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

fn compact_history_message(value: &Value) -> Result<ProviderMessage, PublishedCompactError> {
    let object = value
        .as_object()
        .ok_or(PublishedCompactError::InvalidRequest)?;
    let role = match object.get("role").and_then(Value::as_str) {
        Some("system") => ProviderMessageRole::System,
        Some("user") => ProviderMessageRole::User,
        Some("assistant") => ProviderMessageRole::Assistant,
        _ => return Err(PublishedCompactError::InvalidRequest),
    };
    let content = object
        .get("content")
        .and_then(Value::as_str)
        .ok_or(PublishedCompactError::InvalidRequest)?;
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

fn published_compact_runtime_error(error: anyhow::Error) -> PublishedCompactError {
    error
        .downcast_ref::<ProviderCompactError>()
        .cloned()
        .map(PublishedCompactError::Provider)
        .unwrap_or(PublishedCompactError::ProviderTargetUnavailable)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn k2b_compact_messages_preserve_the_typed_turn_order() {
        let request: NativeRunRequest = serde_json::from_value(json!({
            "query": "retain the latest turn",
            "history": [
                {"role": "user", "content": "earlier request"},
                {"role": "assistant", "content": "earlier answer"}
            ]
        }))
        .expect("compact fixture request should deserialize");

        let messages = compact_messages(&request).expect("compact input should remain typed");

        assert_eq!(messages.len(), 3);
        assert_eq!(messages[0].role, ProviderMessageRole::User);
        assert_eq!(messages[1].role, ProviderMessageRole::Assistant);
        assert_eq!(messages[2].content, "retain the latest turn");
    }

    #[test]
    fn k2b_provider_compact_failure_stays_typed() {
        let provider_error = ProviderCompactError::Runtime {
            error: plugin_framework::provider_contract::ProviderRuntimeError::new(
                plugin_framework::provider_contract::ProviderRuntimeErrorKind::ProviderInvalidResponse,
                "malformed Compact result",
            ),
        };

        assert_eq!(
            published_compact_runtime_error(anyhow::Error::new(provider_error.clone())),
            PublishedCompactError::Provider(provider_error)
        );
    }
}
