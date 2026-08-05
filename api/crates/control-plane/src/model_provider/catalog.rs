use std::collections::{BTreeMap, HashMap, HashSet};

use anyhow::Result;
use plugin_framework::provider_contract::{ProviderModelDescriptor, ProviderModelSource};
use plugin_framework::provider_package::ProviderConfigField;
use serde_json::Value;
use uuid::Uuid;

use crate::{
    errors::ControlPlaneError,
    i18n::{
        merge_i18n_catalog, plugin_namespace, trim_json_bundles, trim_provider_bundles,
        RequestedLocales,
    },
    installed_provider_package::load_installed_provider_package,
    model_provider::{
        ModelProviderCatalogEntry, ModelProviderCatalogView, ModelProviderMainInstanceSummary,
        ModelProviderOptionEntry, ModelProviderOptionGroup, ModelProviderOptionTarget,
        ModelProviderOptionsView, ModelProviderUseCase,
    },
    ports::{AuthRepository, ModelProviderRepository, PluginRepository},
};

use super::shared::{
    ensure_model_provider_permission, load_actor_context_for_user, localized_model_descriptor,
    model_provider_installation_from_current_snapshot, ModelProviderNodeArtifactContext,
};

#[derive(Debug)]
struct ModelProviderCatalogProjectionView {
    display_name: String,
    help_url: Option<String>,
    default_base_url: Option<String>,
    model_discovery_mode: String,
    supports_model_fetch_without_credentials: bool,
    form_schema: Vec<ProviderConfigField>,
    predefined_models: Vec<ProviderModelDescriptor>,
    i18n_bundles: BTreeMap<String, Value>,
    catalog_refresh_status: String,
    catalog_last_error_message: Option<String>,
    catalog_refreshed_at: Option<time::OffsetDateTime>,
}

pub(super) async fn list_catalog<R>(
    repository: &R,
    actor_user_id: Uuid,
    locales: RequestedLocales,
    node_artifact_context: Option<ModelProviderNodeArtifactContext<'_>>,
    use_case: ModelProviderUseCase,
) -> Result<ModelProviderCatalogView>
where
    R: AuthRepository + PluginRepository,
{
    let actor = load_actor_context_for_user(repository, actor_user_id).await?;
    ensure_model_provider_permission(&actor, "view", &use_case).await?;

    let assignments = repository
        .list_assignments(actor.current_workspace_id)
        .await?
        .into_iter()
        .map(|assignment| assignment.installation_id)
        .collect::<HashSet<_>>();
    let installations = repository.list_installations().await?;
    let node_id = node_artifact_context
        .map(|context| context.node_id)
        .ok_or(ControlPlaneError::Conflict("plugin_node_context_required"))?;
    let artifacts = repository
        .list_artifact_instances(node_id)
        .await?
        .into_iter()
        .map(|artifact| (artifact.installation_id, artifact))
        .collect::<HashMap<_, _>>();
    let projections = repository
        .list_plugin_package_catalog_projections()
        .await?
        .into_iter()
        .map(|projection| (projection.installation_id, projection))
        .collect::<HashMap<_, _>>();
    let mut catalog = Vec::new();
    let mut i18n_catalog = BTreeMap::new();
    for installation in installations {
        if matches!(
            installation.desired_state,
            domain::PluginDesiredState::Disabled
        ) || !assignments.contains(&installation.id)
        {
            continue;
        }
        let Some(artifact) = artifacts.get(&installation.id) else {
            continue;
        };
        if artifact.availability_status != domain::PluginAvailabilityStatus::Available {
            continue;
        }
        let namespace = plugin_namespace(&installation.provider_code);
        let projection =
            model_provider_projection_view(&installation, projections.get(&installation.id));
        merge_i18n_catalog(
            &mut i18n_catalog,
            trim_json_bundles(&namespace, &projection.i18n_bundles, &locales),
        );
        catalog.push(ModelProviderCatalogEntry {
            installation_id: installation.id,
            provider_code: installation.provider_code,
            plugin_id: installation.plugin_id,
            plugin_version: installation.plugin_version,
            plugin_type: "model_provider".to_string(),
            namespace: namespace.clone(),
            label_key: "provider.label".to_string(),
            description_key: Some("provider.description".to_string()),
            display_name: projection.display_name,
            protocol: installation.protocol,
            help_url: projection.help_url,
            default_base_url: projection.default_base_url,
            model_discovery_mode: projection.model_discovery_mode,
            supports_model_fetch_without_credentials: projection
                .supports_model_fetch_without_credentials,
            desired_state: installation.desired_state.as_str().to_string(),
            availability_status: artifact.availability_status.as_str().to_string(),
            form_schema: projection.form_schema,
            predefined_models: projection
                .predefined_models
                .into_iter()
                .map(|model| localized_model_descriptor(&namespace, model))
                .collect(),
            catalog_refresh_status: projection.catalog_refresh_status,
            catalog_last_error_message: projection.catalog_last_error_message,
            catalog_refreshed_at: projection.catalog_refreshed_at,
        });
    }

    Ok(ModelProviderCatalogView {
        entries: catalog,
        i18n_catalog,
    })
}

pub(super) async fn options<R>(
    repository: &R,
    actor_user_id: Uuid,
    locales: RequestedLocales,
    node_artifact_context: Option<ModelProviderNodeArtifactContext<'_>>,
    use_case: ModelProviderUseCase,
) -> Result<ModelProviderOptionsView>
where
    R: AuthRepository + PluginRepository + ModelProviderRepository,
{
    let actor = load_actor_context_for_user(repository, actor_user_id).await?;
    ensure_model_provider_permission(&actor, "view", &use_case).await?;
    let mut installation_map = HashMap::new();
    for installation in repository.list_installations().await? {
        let Some(installation) = model_provider_installation_from_current_snapshot(
            repository,
            node_artifact_context,
            installation,
        )
        .await?
        else {
            continue;
        };
        installation_map.insert(installation.id, installation);
    }
    let mut instances_by_provider =
        HashMap::<String, Vec<domain::ModelProviderInstanceRecord>>::new();
    for instance in repository
        .list_instances(actor.current_workspace_id)
        .await?
    {
        if instance.status != domain::ModelProviderInstanceStatus::Ready
            || !instance.included_in_main
        {
            continue;
        }
        instances_by_provider
            .entry(instance.provider_code.clone())
            .or_default()
            .push(instance);
    }
    let mut provider_codes = instances_by_provider.keys().cloned().collect::<Vec<_>>();
    provider_codes.sort();

    let mut options = Vec::new();
    let mut i18n_catalog = BTreeMap::new();
    for provider_code in provider_codes {
        let Some(mut instances) = instances_by_provider.remove(&provider_code) else {
            continue;
        };
        instances.sort_by(|left, right| {
            left.display_name
                .cmp(&right.display_name)
                .then(left.id.cmp(&right.id))
        });
        let Some(first_instance) = instances.first() else {
            continue;
        };
        let installation_id = first_instance.installation_id;
        let protocol = first_instance.protocol.clone();
        let Some(installation) = installation_map.get(&installation_id) else {
            continue;
        };
        if installation.availability_status() != domain::PluginAvailabilityStatus::Available {
            continue;
        }
        let package = load_installed_provider_package(installation)?;
        let namespace = plugin_namespace(&provider_code);
        merge_i18n_catalog(
            &mut i18n_catalog,
            trim_provider_bundles(&namespace, &package.i18n, &locales),
        );

        let main_instance = repository
            .get_main_instance(actor.current_workspace_id, &provider_code)
            .await?;
        let routing_policies = model_routing_policies_by_id(main_instance.as_ref());
        let mut model_groups: Vec<ModelProviderOptionGroup> = Vec::new();
        let mut model_group_indexes: HashMap<String, usize> = HashMap::new();
        for instance in instances {
            let candidate_models = match repository.get_catalog_cache(instance.id).await? {
                Some(cache) => serde_json::from_value(cache.models_json).unwrap_or_default(),
                None => package.predefined_models.clone(),
            };
            let models = expose_enabled_models(
                &namespace,
                candidate_models,
                &instance.configured_models,
                &instance.enabled_model_ids,
            );
            for model in models {
                let model_id = model.descriptor.model_id.clone();
                let routing_enabled = !routing_policies.get(&model_id).is_some_and(|policy| {
                    policy.excluded_provider_instance_ids.contains(&instance.id)
                });
                let target = ModelProviderOptionTarget {
                    source_instance_id: instance.id,
                    source_instance_display_name: instance.display_name.clone(),
                    routing_enabled,
                    model: model.clone(),
                };
                if let Some(index) = model_group_indexes.get(&model_id).copied() {
                    model_groups[index].targets.push(target);
                    continue;
                }

                model_group_indexes.insert(model_id.clone(), model_groups.len());
                model_groups.push(ModelProviderOptionGroup {
                    distribution_rule: routing_policies
                        .get(&model_id)
                        .map(|policy| policy.distribution_rule)
                        .unwrap_or_default(),
                    model_id,
                    model,
                    targets: vec![target],
                });
            }
        }
        for group in &mut model_groups {
            let configured_positions = routing_policies
                .get(&group.model_id)
                .map(|policy| {
                    policy
                        .provider_instance_ids
                        .iter()
                        .enumerate()
                        .map(|(position, id)| (*id, position))
                        .collect::<HashMap<_, _>>()
                })
                .unwrap_or_default();
            group.targets.sort_by(|left, right| {
                configured_positions
                    .get(&left.source_instance_id)
                    .copied()
                    .unwrap_or(usize::MAX)
                    .cmp(
                        &configured_positions
                            .get(&right.source_instance_id)
                            .copied()
                            .unwrap_or(usize::MAX),
                    )
                    .then(left.source_instance_id.cmp(&right.source_instance_id))
            });
        }
        let model_count = model_groups.len();
        options.push(ModelProviderOptionEntry {
            provider_code: provider_code.clone(),
            plugin_type: "model_provider".to_string(),
            namespace: namespace.clone(),
            label_key: "provider.label".to_string(),
            description_key: Some("provider.description".to_string()),
            protocol,
            display_name: package.provider.display_name.clone(),
            icon: package.manifest.icon.clone(),
            parameter_form: package.provider.parameter_form.clone(),
            main_instance: ModelProviderMainInstanceSummary {
                provider_code: provider_code.clone(),
                auto_include_new_instances: super::main_instance::auto_include_new_instances(
                    main_instance.as_ref(),
                ),
                group_count: model_groups.len(),
                model_count,
            },
            model_groups,
        });
    }
    Ok(ModelProviderOptionsView {
        providers: options,
        i18n_catalog,
    })
}

fn model_routing_policies_by_id(
    main_instance: Option<&domain::ModelProviderMainInstanceRecord>,
) -> HashMap<String, domain::ModelProviderMainModelRoutingPolicy> {
    main_instance
        .map(|record| {
            record
                .model_routing_policies
                .iter()
                .map(|policy| {
                    (
                        policy.model_id.clone(),
                        domain::ModelProviderMainModelRoutingPolicy {
                            model_id: policy.model_id.clone(),
                            distribution_rule: policy.distribution_rule,
                            provider_instance_ids: policy.provider_instance_ids.clone(),
                            excluded_provider_instance_ids: policy
                                .excluded_provider_instance_ids
                                .clone(),
                        },
                    )
                })
                .collect()
        })
        .unwrap_or_default()
}

fn model_provider_projection_view(
    installation: &domain::PluginInstallationRecord,
    projection: Option<&domain::PluginPackageCatalogProjectionRecord>,
) -> ModelProviderCatalogProjectionView {
    let Some(projection) = projection else {
        return ModelProviderCatalogProjectionView {
            display_name: installation.display_name.clone(),
            help_url: None,
            default_base_url: None,
            model_discovery_mode: "unknown".to_string(),
            supports_model_fetch_without_credentials: false,
            form_schema: Vec::new(),
            predefined_models: Vec::new(),
            i18n_bundles: BTreeMap::new(),
            catalog_refresh_status: domain::PluginPackageCatalogProjectionStatus::Missing
                .as_str()
                .to_string(),
            catalog_last_error_message: None,
            catalog_refreshed_at: None,
        };
    };

    let snapshot = &projection.catalog_snapshot_json;
    ModelProviderCatalogProjectionView {
        display_name: projection_provider_string(snapshot, "display_name")
            .unwrap_or_else(|| installation.display_name.clone()),
        help_url: projection_provider_string(snapshot, "help_url"),
        default_base_url: projection_provider_string(snapshot, "default_base_url"),
        model_discovery_mode: projection_provider_string(snapshot, "model_discovery_mode")
            .unwrap_or_else(|| "unknown".to_string()),
        supports_model_fetch_without_credentials: snapshot
            .pointer("/provider/supports_model_fetch_without_credentials")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        form_schema: projection_provider_array(snapshot, "form_schema"),
        predefined_models: projection_provider_array(snapshot, "predefined_models"),
        i18n_bundles: projection_i18n_bundles(snapshot),
        catalog_refresh_status: projection.projection_status.as_str().to_string(),
        catalog_last_error_message: projection.last_error_message.clone(),
        catalog_refreshed_at: projection.refreshed_at,
    }
}

fn projection_provider_string(snapshot: &Value, field: &str) -> Option<String> {
    snapshot
        .get("provider")?
        .get(field)?
        .as_str()
        .map(str::to_string)
}

fn projection_provider_array<T>(snapshot: &Value, field: &str) -> Vec<T>
where
    T: serde::de::DeserializeOwned,
{
    snapshot
        .get("provider")
        .and_then(|provider| provider.get(field))
        .cloned()
        .and_then(|value| serde_json::from_value(value).ok())
        .unwrap_or_default()
}

fn projection_i18n_bundles(snapshot: &Value) -> BTreeMap<String, Value> {
    snapshot
        .pointer("/i18n/bundles")
        .cloned()
        .and_then(|value| serde_json::from_value(value).ok())
        .unwrap_or_default()
}

fn expose_enabled_models(
    namespace: &str,
    models: Vec<ProviderModelDescriptor>,
    configured_models: &[domain::ModelProviderConfiguredModel],
    enabled_model_ids: &[String],
) -> Vec<crate::model_provider::LocalizedProviderModelDescriptor> {
    let configured_models_by_id = configured_models
        .iter()
        .map(|model| (model.model_id.as_str(), model))
        .collect::<HashMap<_, _>>();
    let localized_models = models
        .into_iter()
        .map(|model| {
            let model_id = model.model_id.clone();
            let configured_model = configured_models_by_id.get(model_id.as_str()).copied();
            (
                model_id,
                localized_model_descriptor(
                    namespace,
                    apply_context_override(model, configured_model),
                ),
            )
        })
        .collect::<HashMap<_, _>>();

    enabled_model_ids
        .iter()
        .map(|model_id| {
            let configured_model = configured_models_by_id.get(model_id.as_str()).copied();
            localized_models
                .get(model_id)
                .cloned()
                .unwrap_or_else(|| fallback_enabled_model_descriptor(model_id, configured_model))
        })
        .collect()
}

fn apply_context_override(
    mut model: ProviderModelDescriptor,
    configured_model: Option<&domain::ModelProviderConfiguredModel>,
) -> ProviderModelDescriptor {
    if let Some(override_tokens) = configured_model
        .and_then(|configured_model| configured_model.context_window_override_tokens)
    {
        model.context_window = Some(override_tokens);
    }
    if let Some(supports_multimodal) =
        configured_model.and_then(|configured_model| configured_model.supports_multimodal)
    {
        model.supports_multimodal = supports_multimodal;
    }

    model
}

fn fallback_enabled_model_descriptor(
    model_id: &str,
    configured_model: Option<&domain::ModelProviderConfiguredModel>,
) -> crate::model_provider::LocalizedProviderModelDescriptor {
    crate::model_provider::LocalizedProviderModelDescriptor {
        descriptor: ProviderModelDescriptor {
            model_id: model_id.to_string(),
            display_name: model_id.to_string(),
            source: ProviderModelSource::Dynamic,
            supports_streaming: false,
            supports_tool_call: false,
            supports_multimodal: configured_model
                .and_then(|configured_model| configured_model.supports_multimodal)
                .unwrap_or(false),
            context_window: configured_model
                .and_then(|configured_model| configured_model.context_window_override_tokens),
            max_output_tokens: None,
            provider_metadata: serde_json::json!({}),
        },
        namespace: None,
        label_key: None,
        description_key: None,
        display_name_fallback: Some(model_id.to_string()),
    }
}
