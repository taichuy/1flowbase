use anyhow::Result;
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
    let record = repository
        .upsert_main_instance(&UpsertModelProviderMainInstanceInput {
            workspace_id,
            provider_code: command.provider_code.clone(),
            auto_include_new_instances: command.auto_include_new_instances,
            model_distribution_rules: command.model_distribution_rules.clone(),
            updated_by: command.actor_user_id,
        })
        .await?;
    let model_distribution_rules = model_distribution_rules(&record);
    Ok(ModelProviderMainInstanceView {
        provider_code: record.provider_code,
        auto_include_new_instances: record.auto_include_new_instances,
        model_distribution_rules,
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
        model_distribution_rules: record.map(model_distribution_rules).unwrap_or_default(),
    }
}

fn model_distribution_rules(
    record: &domain::ModelProviderMainInstanceRecord,
) -> Vec<domain::ModelProviderMainModelDistributionRule> {
    record
        .model_distribution_rules
        .iter()
        .map(|rule| domain::ModelProviderMainModelDistributionRule {
            model_id: rule.model_id.clone(),
            distribution_rule: rule.distribution_rule,
        })
        .collect()
}
