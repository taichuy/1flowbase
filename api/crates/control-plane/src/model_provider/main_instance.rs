use anyhow::Result;
use std::collections::HashSet;
use uuid::Uuid;

use crate::{
    model_provider::{ModelProviderMainInstanceView, UpdateModelProviderMainInstanceCommand},
    ports::{ModelProviderRepository, PluginRepository, UpsertModelProviderMainInstanceInput},
};

pub(super) async fn get_main_instance<R>(
    repository: &R,
    workspace_id: Uuid,
    provider_code: &str,
) -> Result<ModelProviderMainInstanceView>
where
    R: PluginRepository + ModelProviderRepository,
{
    super::routing::ensure_provider_exists(repository, workspace_id, provider_code).await?;
    Ok(to_view(
        provider_code,
        repository
            .get_main_instance(workspace_id, provider_code)
            .await?
            .as_ref(),
    ))
}

pub(super) async fn update_main_instance<R>(
    repository: &R,
    workspace_id: Uuid,
    command: &UpdateModelProviderMainInstanceCommand,
) -> Result<ModelProviderMainInstanceView>
where
    R: PluginRepository + ModelProviderRepository,
{
    super::routing::ensure_provider_exists(repository, workspace_id, &command.provider_code)
        .await?;
    validate_routing_policies(
        repository,
        workspace_id,
        &command.provider_code,
        command
            .model_routing_policies
            .as_deref()
            .unwrap_or_default(),
    )
    .await?;
    let record = repository
        .upsert_main_instance(&UpsertModelProviderMainInstanceInput {
            workspace_id,
            provider_code: command.provider_code.clone(),
            auto_include_new_instances: command.auto_include_new_instances,
            expected_revision: command.expected_revision,
            model_routing_policies: command.model_routing_policies.clone(),
            updated_by: command.actor_user_id,
        })
        .await?;
    let model_routing_policies = model_routing_policies(&record);
    Ok(ModelProviderMainInstanceView {
        provider_code: record.provider_code,
        auto_include_new_instances: record.auto_include_new_instances,
        revision: record.revision,
        model_routing_policies,
    })
}

pub(super) fn auto_include_new_instances(
    record: Option<&domain::ModelProviderMainInstanceRecord>,
) -> bool {
    record
        .map(|record| record.auto_include_new_instances)
        .unwrap_or(domain::DEFAULT_AUTO_INCLUDE_NEW_PROVIDER_INSTANCES)
}

fn to_view(
    provider_code: &str,
    record: Option<&domain::ModelProviderMainInstanceRecord>,
) -> ModelProviderMainInstanceView {
    ModelProviderMainInstanceView {
        provider_code: provider_code.to_string(),
        auto_include_new_instances: auto_include_new_instances(record),
        revision: record.map(|record| record.revision).unwrap_or(0),
        model_routing_policies: record.map(model_routing_policies).unwrap_or_default(),
    }
}

fn model_routing_policies(
    record: &domain::ModelProviderMainInstanceRecord,
) -> Vec<domain::ModelProviderMainModelRoutingPolicy> {
    record
        .model_routing_policies
        .iter()
        .map(|policy| domain::ModelProviderMainModelRoutingPolicy {
            model_id: policy.model_id.clone(),
            distribution_rule: policy.distribution_rule,
            provider_instance_ids: policy.provider_instance_ids.clone(),
        })
        .collect()
}

async fn validate_routing_policies<R>(
    repository: &R,
    workspace_id: Uuid,
    provider_code: &str,
    policies: &[domain::ModelProviderMainModelRoutingPolicy],
) -> Result<()>
where
    R: ModelProviderRepository,
{
    let provider_instance_ids = repository
        .list_instances(workspace_id)
        .await?
        .into_iter()
        .filter(|instance| instance.provider_code == provider_code)
        .map(|instance| instance.id)
        .collect::<HashSet<_>>();
    let mut model_ids = HashSet::new();
    for policy in policies {
        if policy.model_id.trim().is_empty() || !model_ids.insert(policy.model_id.as_str()) {
            return Err(
                crate::errors::ControlPlaneError::InvalidInput("model_routing_policy").into(),
            );
        }
        let mut ordered_ids = HashSet::new();
        if policy.provider_instance_ids.iter().any(|instance_id| {
            !provider_instance_ids.contains(instance_id) || !ordered_ids.insert(*instance_id)
        }) {
            return Err(
                crate::errors::ControlPlaneError::InvalidInput("provider_instance_ids").into(),
            );
        }
    }
    Ok(())
}
