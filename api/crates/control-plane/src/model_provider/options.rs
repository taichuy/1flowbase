use anyhow::Result;
use serde_json::json;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::{
    audit::audit_log,
    errors::ControlPlaneError,
    installed_provider_package::load_installed_provider_package,
    model_provider::{ModelProviderModelCatalog, ModelProviderUseCase},
    ports::{
        AuthRepository, ModelProviderRepository, PluginRepository, ProviderRuntimePort,
        UpsertModelProviderCatalogCacheInput,
    },
};

use super::{
    instances::build_provider_runtime_config,
    shared::{
        empty_object, ensure_model_provider_permission, is_secret_field,
        load_actor_context_for_user, map_catalog_source, map_model_discovery_mode,
        ready_model_provider_installation, ModelProviderNodeArtifactContext,
    },
};

pub(super) async fn list_models<R>(
    repository: &R,
    actor_user_id: Uuid,
    instance_id: Uuid,
    node_artifact_context: Option<ModelProviderNodeArtifactContext<'_>>,
    use_case: ModelProviderUseCase,
) -> Result<ModelProviderModelCatalog>
where
    R: AuthRepository + PluginRepository + ModelProviderRepository,
{
    let actor = load_actor_context_for_user(repository, actor_user_id).await?;
    ensure_model_provider_permission(&actor, "view", &use_case).await?;
    let instance = repository
        .get_instance(actor.current_workspace_id, instance_id)
        .await?
        .ok_or(ControlPlaneError::NotFound("model_provider_instance"))?;
    if let Some(cache) = repository.get_catalog_cache(instance.id).await? {
        return Ok(ModelProviderModelCatalog {
            provider_instance_id: instance.id,
            refresh_status: cache.refresh_status,
            source: cache.source,
            last_error_message: cache.last_error_message,
            refreshed_at: cache.refreshed_at,
            models: serde_json::from_value(cache.models_json).unwrap_or_default(),
        });
    }

    let installation = ready_model_provider_installation(
        repository,
        node_artifact_context,
        instance.installation_id,
    )
    .await?;
    let package = load_installed_provider_package(&installation)?;

    Ok(ModelProviderModelCatalog {
        provider_instance_id: instance.id,
        refresh_status: domain::ModelProviderCatalogRefreshStatus::Idle,
        source: map_catalog_source(package.provider.model_discovery_mode),
        last_error_message: None,
        refreshed_at: None,
        models: package.predefined_models,
    })
}

pub(super) async fn refresh_models<R, H>(
    repository: &R,
    runtime: &H,
    provider_secret_master_key: &str,
    actor_user_id: Uuid,
    instance_id: Uuid,
    node_artifact_context: Option<ModelProviderNodeArtifactContext<'_>>,
    use_case: ModelProviderUseCase,
) -> Result<ModelProviderModelCatalog>
where
    R: AuthRepository + PluginRepository + ModelProviderRepository,
    H: ProviderRuntimePort,
{
    let actor = load_actor_context_for_user(repository, actor_user_id).await?;
    ensure_model_provider_permission(&actor, "manage", &use_case).await?;
    let instance = repository
        .get_instance(actor.current_workspace_id, instance_id)
        .await?
        .ok_or(ControlPlaneError::NotFound("model_provider_instance"))?;
    let installation = ready_model_provider_installation(
        repository,
        node_artifact_context,
        instance.installation_id,
    )
    .await?;
    if installation.availability_status() != domain::PluginAvailabilityStatus::Available {
        return Err(ControlPlaneError::Conflict("plugin_installation_unavailable").into());
    }
    let package = load_installed_provider_package(&installation)?;
    let provider_config =
        build_provider_runtime_config(repository, provider_secret_master_key, &package, &instance)
            .await?;
    let existing_cache = repository.get_catalog_cache(instance.id).await?;

    let refresh_result = async {
        runtime.ensure_loaded(&installation).await?;
        let models = runtime.list_models(&installation, provider_config).await?;
        let cache = repository
            .upsert_catalog_cache(&UpsertModelProviderCatalogCacheInput {
                provider_instance_id: instance.id,
                model_discovery_mode: map_model_discovery_mode(
                    package.provider.model_discovery_mode,
                ),
                refresh_status: domain::ModelProviderCatalogRefreshStatus::Ready,
                source: map_catalog_source(package.provider.model_discovery_mode),
                models_json: serde_json::to_value(&models)?,
                last_error_message: None,
                refreshed_at: Some(OffsetDateTime::now_utc()),
            })
            .await?;
        repository
            .append_audit_log(&audit_log(
                Some(actor.current_workspace_id),
                Some(actor_user_id),
                "model_provider_instance",
                Some(instance.id),
                "model_provider.models_refreshed",
                json!({
                    "provider_code": instance.provider_code,
                    "model_count": models.len(),
                }),
            ))
            .await?;
        Ok::<ModelProviderModelCatalog, anyhow::Error>(ModelProviderModelCatalog {
            provider_instance_id: instance.id,
            refresh_status: cache.refresh_status,
            source: cache.source,
            last_error_message: cache.last_error_message,
            refreshed_at: cache.refreshed_at,
            models,
        })
    }
    .await;

    match refresh_result {
        Ok(result) => Ok(result),
        Err(error) => {
            let _ = repository
                .upsert_catalog_cache(&UpsertModelProviderCatalogCacheInput {
                    provider_instance_id: instance.id,
                    model_discovery_mode: map_model_discovery_mode(
                        package.provider.model_discovery_mode,
                    ),
                    refresh_status: domain::ModelProviderCatalogRefreshStatus::Failed,
                    source: map_catalog_source(package.provider.model_discovery_mode),
                    models_json: existing_cache
                        .as_ref()
                        .map(|cache| cache.models_json.clone())
                        .unwrap_or_else(|| json!([])),
                    last_error_message: Some(error.to_string()),
                    refreshed_at: existing_cache.and_then(|cache| cache.refreshed_at),
                })
                .await;
            let _ = repository
                .append_audit_log(&audit_log(
                    Some(actor.current_workspace_id),
                    Some(actor_user_id),
                    "model_provider_instance",
                    Some(instance.id),
                    "model_provider.models_refresh_failed",
                    json!({
                        "provider_code": instance.provider_code,
                        "message": error.to_string(),
                    }),
                ))
                .await;
            Err(error)
        }
    }
}

pub(super) async fn reveal_secret<R>(
    repository: &R,
    provider_secret_master_key: &str,
    actor_user_id: Uuid,
    instance_id: Uuid,
    key: &str,
    node_artifact_context: Option<ModelProviderNodeArtifactContext<'_>>,
    use_case: ModelProviderUseCase,
) -> Result<String>
where
    R: AuthRepository + PluginRepository + ModelProviderRepository,
{
    let actor = load_actor_context_for_user(repository, actor_user_id).await?;
    ensure_model_provider_permission(&actor, "manage", &use_case).await?;
    let instance = repository
        .get_instance(actor.current_workspace_id, instance_id)
        .await?
        .ok_or(ControlPlaneError::NotFound("model_provider_instance"))?;
    let installation = ready_model_provider_installation(
        repository,
        node_artifact_context,
        instance.installation_id,
    )
    .await?;
    let package = load_installed_provider_package(&installation)?;
    let field = package
        .provider
        .form_schema
        .iter()
        .find(|field| field.key == key)
        .ok_or(ControlPlaneError::InvalidInput("key"))?;
    if !is_secret_field(&field.field_type) {
        return Err(ControlPlaneError::InvalidInput("key").into());
    }

    let secret_json = repository
        .get_secret_json(instance.id, provider_secret_master_key)
        .await?
        .unwrap_or_else(empty_object);
    secret_json
        .get(key)
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
        .ok_or(ControlPlaneError::NotFound("model_provider_secret").into())
}
