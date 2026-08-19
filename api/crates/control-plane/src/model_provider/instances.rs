use anyhow::Result;
use plugin_framework::{
    provider_contract::{ProviderAuthOperation, ProviderAuthResult, ProviderAuthStatus},
    provider_package::ProviderConfigField,
};
use serde_json::{Map, Value};

use crate::{
    errors::ControlPlaneError,
    model_provider::ModelProviderInstanceView,
    ports::{ModelProviderRepository, PatchModelProviderSecretInput, ProviderRuntimePort},
};

use super::shared::{
    empty_object, mask_secret_config, merge_json_object, validate_required_fields,
};

pub(super) async fn build_provider_runtime_config<R>(
    repository: &R,
    provider_secret_master_key: &str,
    package: &plugin_framework::provider_package::ProviderPackage,
    instance: &domain::ModelProviderInstanceRecord,
) -> Result<Value>
where
    R: ModelProviderRepository,
{
    let secret_json = repository
        .get_secret_json(instance.id, provider_secret_master_key)
        .await?
        .unwrap_or_else(empty_object);
    validate_required_fields(
        &package.provider.form_schema,
        &instance.config_json,
        &secret_json,
    )?;
    merge_json_object(&instance.config_json, &secret_json)
}

pub(crate) async fn maintain_provider_runtime_config<R, H>(
    repository: &R,
    runtime: &H,
    provider_secret_master_key: &str,
    package: &plugin_framework::provider_package::ProviderPackage,
    installation: &domain::LocalPluginInstallationRecord,
    instance: &domain::ModelProviderInstanceRecord,
) -> Result<Value>
where
    R: ModelProviderRepository,
    H: ProviderRuntimePort,
{
    if package.provider.auth.is_none() {
        return build_provider_runtime_config(
            repository,
            provider_secret_master_key,
            package,
            instance,
        )
        .await;
    }

    let (provider_config, result) = execute_provider_auth(
        repository,
        runtime,
        provider_secret_master_key,
        package,
        installation,
        instance,
        ProviderAuthOperation::Maintain,
    )
    .await?;
    if result.status != ProviderAuthStatus::Authorized {
        return Err(ControlPlaneError::Conflict("provider_authentication_required").into());
    }
    validate_required_fields(
        &package.provider.form_schema,
        &instance.config_json,
        &provider_config,
    )?;
    Ok(provider_config)
}

pub(crate) async fn execute_provider_auth<R, H>(
    repository: &R,
    runtime: &H,
    provider_secret_master_key: &str,
    package: &plugin_framework::provider_package::ProviderPackage,
    installation: &domain::LocalPluginInstallationRecord,
    instance: &domain::ModelProviderInstanceRecord,
    operation: ProviderAuthOperation,
) -> Result<(Value, ProviderAuthResult)>
where
    R: ModelProviderRepository,
    H: ProviderRuntimePort,
{
    let auth = package
        .provider
        .auth
        .as_ref()
        .ok_or(ControlPlaneError::Conflict(
            "provider_authentication_unsupported",
        ))?;
    let current_secret = repository
        .get_secret_json(instance.id, provider_secret_master_key)
        .await?
        .unwrap_or_else(empty_object);
    let current_secret_version = repository
        .get_secret_record(instance.id)
        .await?
        .map(|record| record.secret_version);
    let provider_config = merge_json_object(&instance.config_json, &current_secret)?;
    let result = runtime
        .authenticate_provider(installation, provider_config.clone(), operation)
        .await?;

    if result.managed_secret_patch.is_empty() {
        return Ok((provider_config, result));
    }

    let mut patched_secret = current_secret.as_object().cloned().unwrap_or_else(Map::new);
    for (key, value) in &result.managed_secret_patch {
        if !auth
            .managed_secret_keys
            .iter()
            .any(|allowed| allowed == key)
        {
            return Err(ControlPlaneError::InvalidInput("provider_auth_secret_key").into());
        }
        patched_secret.insert(key.clone(), value.clone());
    }
    let patched_secret = Value::Object(patched_secret);
    repository
        .patch_secret(&PatchModelProviderSecretInput {
            provider_instance_id: instance.id,
            expected_secret_version: current_secret_version,
            plaintext_secret_json: patched_secret.clone(),
            master_key: provider_secret_master_key.to_string(),
        })
        .await?;
    let provider_config = merge_json_object(&instance.config_json, &patched_secret)?;
    Ok((provider_config, result))
}

pub(super) async fn hydrate_instance_view<R>(
    repository: &R,
    provider_secret_master_key: &str,
    instance: domain::ModelProviderInstanceRecord,
    cache: Option<domain::ModelProviderCatalogCacheRecord>,
    form_schema: &[ProviderConfigField],
) -> Result<ModelProviderInstanceView>
where
    R: ModelProviderRepository,
{
    let secret_json = repository
        .get_secret_json(instance.id, provider_secret_master_key)
        .await?
        .unwrap_or_else(empty_object);
    let merged_config = mask_secret_config(&instance.config_json, &secret_json, form_schema)?;

    Ok(ModelProviderInstanceView {
        instance: domain::ModelProviderInstanceRecord {
            config_json: merged_config,
            ..instance
        },
        cache,
    })
}
