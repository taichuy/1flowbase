use super::*;

pub(super) async fn load_model_provider_routing_catalog<R>(
    repository: &R,
    workspace_id: Uuid,
    cache_store: Option<&dyn CacheStore>,
) -> Result<ModelProviderRoutingCatalogSnapshot>
where
    R: ModelProviderRepository + PluginRepository,
{
    let cache_key = model_routing_catalog_cache_key(workspace_id);
    if let Some(cache_store) = cache_store {
        match cache_store.get_json(&cache_key).await {
            Ok(Some(value)) => match serde_json::from_value(value) {
                Ok(snapshot) => return Ok(snapshot),
                Err(error) => {
                    tracing::warn!(%workspace_id, %error, "invalid cached model provider routing catalog");
                    invalidate_model_provider_routing_catalog(cache_store, workspace_id).await;
                }
            },
            Ok(None) => {}
            Err(error) => {
                tracing::warn!(%workspace_id, %error, "failed to read model provider routing catalog cache");
            }
        }
    }
    let snapshot = build_model_provider_routing_catalog(repository, workspace_id).await?;
    if let Some(cache_store) = cache_store {
        match serde_json::to_value(&snapshot) {
            Ok(value) => {
                if let Err(error) = cache_store
                    .set_json(&cache_key, value, Some(MODEL_ROUTING_CATALOG_TTL))
                    .await
                {
                    tracing::warn!(%workspace_id, %error, "failed to cache model provider routing catalog");
                }
            }
            Err(error) => {
                tracing::warn!(%workspace_id, %error, "failed to serialize model provider routing catalog");
            }
        }
    }
    Ok(snapshot)
}

pub(super) async fn build_model_provider_routing_catalog<R>(
    repository: &R,
    workspace_id: Uuid,
) -> Result<ModelProviderRoutingCatalogSnapshot>
where
    R: ModelProviderRepository + PluginRepository,
{
    let instances = repository.list_instances(workspace_id).await?;
    let assigned_installation_ids = repository
        .list_assignments(workspace_id)
        .await?
        .into_iter()
        .map(|assignment| assignment.installation_id)
        .collect::<BTreeSet<_>>();
    let mut provider_families = BTreeMap::new();
    let mut provider_instances = BTreeMap::new();
    let mut provider_codes = BTreeSet::new();

    for instance in instances {
        provider_codes.insert(instance.provider_code.clone());
        let available_models = available_models_for_instance(repository, &instance).await?;
        let allow_custom_models = allow_custom_models(&instance);
        let installation_runnable = installation_is_runnable(
            repository,
            instance.installation_id,
            assigned_installation_ids.contains(&instance.installation_id),
        )
        .await?;
        provider_instances.insert(
            instance.id.to_string(),
            orchestration_runtime::compiler::FlowCompileProviderInstance {
                provider_instance_id: instance.id.to_string(),
                display_name: instance.display_name.clone(),
                provider_code: instance.provider_code.clone(),
                protocol: instance.protocol.clone(),
                is_ready: instance.status == domain::ModelProviderInstanceStatus::Ready,
                is_runnable: installation_runnable,
                included_in_main: instance.included_in_main,
                available_models: available_models.clone(),
                allow_custom_models,
            },
        );

        if !instance.included_in_main || !installation_runnable {
            continue;
        }

        provider_families
            .entry(instance.provider_code.clone())
            .and_modify(
                |family: &mut orchestration_runtime::compiler::FlowCompileProviderFamily| {
                    family.is_ready |=
                        instance.status == domain::ModelProviderInstanceStatus::Ready;
                    family
                        .available_models
                        .extend(available_models.iter().cloned());
                    family.allow_custom_models |= allow_custom_models;
                },
            )
            .or_insert_with(
                || orchestration_runtime::compiler::FlowCompileProviderFamily {
                    provider_code: instance.provider_code.clone(),
                    protocol: instance.protocol.clone(),
                    is_ready: instance.status == domain::ModelProviderInstanceStatus::Ready,
                    available_models,
                    allow_custom_models,
                },
            );
    }

    let mut model_routing_policies = BTreeMap::new();
    for provider_code in provider_codes {
        let Some(main_instance) = repository
            .get_main_instance(workspace_id, &provider_code)
            .await?
        else {
            continue;
        };
        for policy in main_instance.model_routing_policies {
            model_routing_policies
                .entry(provider_code.clone())
                .or_insert_with(BTreeMap::new)
                .insert(
                    policy.model_id,
                    orchestration_runtime::compiler::FlowCompileModelRoutingPolicy {
                        distribution_rule: crate::orchestration_runtime::map_llm_distribution_rule(
                            policy.distribution_rule,
                        ),
                        provider_instance_ids: policy
                            .provider_instance_ids
                            .into_iter()
                            .map(|id| id.to_string())
                            .collect(),
                        excluded_provider_instance_ids: policy
                            .excluded_provider_instance_ids
                            .into_iter()
                            .map(|id| id.to_string())
                            .collect(),
                    },
                );
        }
    }

    Ok(ModelProviderRoutingCatalogSnapshot {
        provider_families,
        provider_instances,
        model_routing_policies,
    })
}
