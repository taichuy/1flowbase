use anyhow::Result;
use uuid::Uuid;

use crate::{
    data_source::{collect_secret_strings, redact_value},
    errors::ControlPlaneError,
    model_provider::{ModelProviderBalanceResult, ModelProviderUseCase},
    ports::{AuthRepository, ModelProviderRepository, PluginRepository, ProviderRuntimePort},
};

use super::{
    instances::build_provider_runtime_config,
    shared::{
        empty_object, ensure_model_provider_permission, load_actor_context_for_user,
        load_provider_package, ready_model_provider_installation, ModelProviderNodeArtifactContext,
    },
};

pub(super) async fn get_balance<R, H>(
    repository: &R,
    runtime: &H,
    provider_secret_master_key: &str,
    actor_user_id: Uuid,
    instance_id: Uuid,
    node_artifact_context: Option<ModelProviderNodeArtifactContext<'_>>,
    use_case: ModelProviderUseCase,
) -> Result<ModelProviderBalanceResult>
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
    let package = load_provider_package(
        installation
            .local_path()
            .ok_or(ControlPlaneError::Conflict("plugin_artifact_path_missing"))?,
    )?;
    let secret_json = repository
        .get_secret_json(instance.id, provider_secret_master_key)
        .await?
        .unwrap_or_else(empty_object);
    let secret_values = collect_secret_strings(&secret_json);
    let provider_config =
        build_provider_runtime_config(repository, provider_secret_master_key, &package, &instance)
            .await?;

    let result = runtime.get_balance(&installation, provider_config).await?;
    let redacted = redact_value(&serde_json::to_value(result)?, &secret_values);
    Ok(serde_json::from_value(redacted)?)
}
