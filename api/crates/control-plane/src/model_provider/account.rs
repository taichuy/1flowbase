use anyhow::Result;
use plugin_framework::provider_contract::{
    ProviderResetCreditOperation, ProviderResetCreditResult,
};
use uuid::Uuid;

use crate::{
    audit::audit_log,
    errors::ControlPlaneError,
    installed_provider_package::load_installed_provider_package,
    model_provider::{
        ConsumeModelProviderResetCreditCommand, ConsumeModelProviderResetCreditResult,
        ModelProviderResetCreditCount, ModelProviderUsageWindowsResult, ModelProviderUseCase,
    },
    ports::{AuthRepository, ModelProviderRepository, PluginRepository, ProviderRuntimePort},
};

use super::{
    instances::maintain_provider_runtime_config,
    shared::{
        ensure_model_provider_permission, load_actor_context_for_user,
        ready_model_provider_installation, ModelProviderNodeArtifactContext,
    },
};

struct ProviderAccountRuntimeTarget {
    workspace_id: Uuid,
    instance: domain::ModelProviderInstanceRecord,
    installation: domain::LocalPluginInstallationRecord,
    provider_config: serde_json::Value,
}

async fn resolve_provider_account_runtime_target<R, H>(
    repository: &R,
    runtime: &H,
    provider_secret_master_key: &str,
    actor_user_id: Uuid,
    instance_id: Uuid,
    node_artifact_context: Option<ModelProviderNodeArtifactContext<'_>>,
    use_case: ModelProviderUseCase,
) -> Result<ProviderAccountRuntimeTarget>
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
        return Err(ControlPlaneError::PluginUnavailable.into());
    }
    let package = load_installed_provider_package(&installation)?;
    let provider_config = maintain_provider_runtime_config(
        repository,
        runtime,
        provider_secret_master_key,
        &package,
        &installation,
        &instance,
    )
    .await?;

    Ok(ProviderAccountRuntimeTarget {
        workspace_id: actor.current_workspace_id,
        instance,
        installation,
        provider_config,
    })
}

pub(super) async fn get_usage_windows<R, H>(
    repository: &R,
    runtime: &H,
    provider_secret_master_key: &str,
    actor_user_id: Uuid,
    instance_id: Uuid,
    node_artifact_context: Option<ModelProviderNodeArtifactContext<'_>>,
    use_case: ModelProviderUseCase,
) -> Result<ModelProviderUsageWindowsResult>
where
    R: AuthRepository + PluginRepository + ModelProviderRepository,
    H: ProviderRuntimePort,
{
    let target = resolve_provider_account_runtime_target(
        repository,
        runtime,
        provider_secret_master_key,
        actor_user_id,
        instance_id,
        node_artifact_context,
        use_case,
    )
    .await?;
    runtime
        .get_usage_windows(&target.installation, target.provider_config)
        .await
}

pub(super) async fn count_reset_credits<R, H>(
    repository: &R,
    runtime: &H,
    provider_secret_master_key: &str,
    actor_user_id: Uuid,
    instance_id: Uuid,
    node_artifact_context: Option<ModelProviderNodeArtifactContext<'_>>,
    use_case: ModelProviderUseCase,
) -> Result<ModelProviderResetCreditCount>
where
    R: AuthRepository + PluginRepository + ModelProviderRepository,
    H: ProviderRuntimePort,
{
    let target = resolve_provider_account_runtime_target(
        repository,
        runtime,
        provider_secret_master_key,
        actor_user_id,
        instance_id,
        node_artifact_context,
        use_case,
    )
    .await?;
    match runtime
        .reset_credit(
            &target.installation,
            target.provider_config,
            ProviderResetCreditOperation::Count,
        )
        .await?
    {
        ProviderResetCreditResult::Count { available_count } => {
            Ok(ModelProviderResetCreditCount { available_count })
        }
        ProviderResetCreditResult::Consumed => {
            Err(ControlPlaneError::InvalidInput("provider_reset_credit_result").into())
        }
    }
}

pub(super) async fn consume_reset_credit<R, H>(
    repository: &R,
    runtime: &H,
    provider_secret_master_key: &str,
    command: ConsumeModelProviderResetCreditCommand,
    node_artifact_context: Option<ModelProviderNodeArtifactContext<'_>>,
    use_case: ModelProviderUseCase,
) -> Result<ConsumeModelProviderResetCreditResult>
where
    R: AuthRepository + PluginRepository + ModelProviderRepository,
    H: ProviderRuntimePort,
{
    if command.idempotency_key.trim().is_empty() {
        return Err(ControlPlaneError::InvalidInput("idempotency_key").into());
    }
    let target = resolve_provider_account_runtime_target(
        repository,
        runtime,
        provider_secret_master_key,
        command.actor_user_id,
        command.instance_id,
        node_artifact_context,
        use_case,
    )
    .await?;

    let result = runtime
        .reset_credit(
            &target.installation,
            target.provider_config,
            ProviderResetCreditOperation::Consume {
                idempotency_key: command.idempotency_key,
            },
        )
        .await;
    match result {
        Ok(ProviderResetCreditResult::Consumed) => {
            repository
                .append_audit_log(&audit_log(
                    Some(target.workspace_id),
                    Some(command.actor_user_id),
                    "model_provider_instance",
                    Some(target.instance.id),
                    "model_provider.reset_credit_consumed",
                    serde_json::json!({
                        "provider_code": target.instance.provider_code,
                    }),
                ))
                .await?;
            Ok(ConsumeModelProviderResetCreditResult { consumed: true })
        }
        Ok(ProviderResetCreditResult::Count { .. }) => {
            Err(ControlPlaneError::InvalidInput("provider_reset_credit_result").into())
        }
        Err(error) => {
            let _ = repository
                .append_audit_log(&audit_log(
                    Some(target.workspace_id),
                    Some(command.actor_user_id),
                    "model_provider_instance",
                    Some(target.instance.id),
                    "model_provider.reset_credit_failed",
                    serde_json::json!({
                        "provider_code": target.instance.provider_code,
                        "message": error.to_string(),
                    }),
                ))
                .await;
            Err(error)
        }
    }
}
