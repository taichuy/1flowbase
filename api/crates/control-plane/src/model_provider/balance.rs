use anyhow::Result;
use uuid::Uuid;

use crate::{
    data_source::{collect_secret_strings, redact_value},
    model_provider::{ModelProviderBalanceResult, ModelProviderUseCase},
    ports::{AuthRepository, ModelProviderRepository, PluginRepository, ProviderRuntimePort},
};

use super::shared::{empty_object, ModelProviderNodeArtifactContext};

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
    let target = super::account::resolve_provider_account_runtime_target(
        repository,
        runtime,
        provider_secret_master_key,
        actor_user_id,
        instance_id,
        node_artifact_context,
        use_case,
    )
    .await?;
    let instance = target.instance;
    let installation = target.installation;
    let provider_config = target.provider_config;
    let secret_json = repository
        .get_secret_json(instance.id, provider_secret_master_key)
        .await?
        .unwrap_or_else(empty_object);
    let secret_values = collect_secret_strings(&secret_json);

    let result = runtime.get_balance(&installation, provider_config).await?;
    let redacted = redact_value(&serde_json::to_value(result)?, &secret_values);
    Ok(serde_json::from_value(redacted)?)
}
