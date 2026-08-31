use std::{collections::BTreeMap, sync::Arc};

use control_plane::ports::{BillingRepository, CacheStore, ListPricingRulesInput};
use interface_runtime::{InterfaceContract, UserPrincipal};
use storage_durable_postgres::MainDurableStore;

use super::*;
use crate::routes::console_interface::{
    self, ConsoleInterfaceDeclaration, ConsoleInterfaceFuture, ConsoleInterfacePort,
    ConsoleInterfaceTargetError,
};

pub(crate) enum ProviderDiscoveryInput {
    Models {
        id: String,
    },
    RefreshModels {
        id: String,
    },
    Options {
        query: ModelProviderCatalogQuery,
        locale: catalog_logs_interface::ProviderLocaleHints,
        settings: bool,
    },
}
impl InterfaceContract for ProviderDiscoveryInput {
    const CONTRACT_ID: &'static str = "console-provider-discovery-input";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) enum ProviderDiscoveryOutput {
    Models(ModelProviderModelCatalogResponse),
    Options(ModelProviderOptionsResponse),
}
impl InterfaceContract for ProviderDiscoveryOutput {
    const CONTRACT_ID: &'static str = "console-provider-discovery-output";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) struct ProviderDiscoveryDependencies {
    pub(crate) store: MainDurableStore,
    pub(crate) provider_runtime: Arc<crate::provider_runtime::ApiRuntimeServices>,
    pub(crate) secret_key: String,
    pub(crate) api_node_id: String,
    pub(crate) install_root: String,
    pub(crate) cache_store: Arc<dyn CacheStore>,
}
struct ProviderDiscoveryAdapter(ProviderDiscoveryDependencies);

impl ProviderDiscoveryAdapter {
    fn service(
        &self,
        actor: &domain::ActorContext,
        settings: bool,
        operation: &'static str,
    ) -> crate::app_state::ApiModelProviderService {
        let group = if settings {
            domain::ConsolePolicyGroup::settings_feature("system.model-providers")
                .expect("compiled model-provider settings group must be valid")
        } else {
            domain::ConsolePolicyGroup::other("other.model-providers")
                .expect("compiled model-provider group must be valid")
        };
        ModelProviderService::for_console_operation(
            self.0.store.for_actor(actor.clone()),
            ApiProviderRuntime::new(self.0.provider_runtime.clone()),
            self.0.secret_key.clone(),
            group,
            operation,
        )
        .with_node_artifact_context(self.0.api_node_id.clone(), self.0.install_root.clone())
        .with_routing_cache_store(self.0.cache_store.clone())
    }

    async fn pricing_targets(&self) -> Result<Vec<ModelProviderPricingTargetResponse>, ApiError> {
        let mut offset = 0;
        let mut rules = Vec::new();
        loop {
            let page = self
                .0
                .store
                .list_pricing_rules(&ListPricingRulesInput {
                    provider_code: None,
                    upstream_model_id: None,
                    enabled: Some(true),
                    source_kind: None,
                    page_size: 500,
                    offset,
                })
                .await?;
            let len = page.items.len();
            rules.extend(page.items);
            offset += len as i64;
            if len < 500 || offset >= page.total_count {
                break;
            }
        }
        let mut grouped = BTreeMap::<(String, String), Vec<PricingRule>>::new();
        for rule in rules {
            grouped
                .entry((rule.provider_code.clone(), rule.upstream_model_id.clone()))
                .or_default()
                .push(rule);
        }
        let now = time::OffsetDateTime::now_utc();
        let mut targets = Vec::new();
        for rules in grouped.into_values() {
            let Some(rule) = control_plane::billing::choose_pricing_rule(rules, now)? else {
                continue;
            };
            targets.push(ModelProviderPricingTargetResponse {
                provider_code: rule.provider_code,
                upstream_model_id: rule.upstream_model_id,
                input_token_unit_size: rule.input_token_unit_size,
                input_token_unit_price: rule.input_token_unit_price.to_string(),
                output_token_unit_size: rule.output_token_unit_size,
                output_token_unit_price: rule.output_token_unit_price.to_string(),
                cache_hit_token_unit_size: rule.cache_hit_token_unit_size,
                cache_hit_token_unit_price: rule.cache_hit_token_unit_price.to_string(),
                effective_from: format_time(rule.effective_from),
                effective_to: format_optional_time(rule.effective_to),
                timezone: rule.timezone,
                weekday_mask: rule.weekday_mask,
                local_time_start: rule.local_time_start.map(|value| value.to_string()),
                local_time_end: rule.local_time_end.map(|value| value.to_string()),
                rating_policy_enabled: rule.rating_policy_enabled,
                rating_policy: rule.rating_policy,
            });
        }
        targets.sort_by_key(|target| {
            (
                target.provider_code != domain::DEFAULT_MODEL_PRICING_PROVIDER_CODE,
                target.provider_code.clone(),
                target.upstream_model_id.clone(),
            )
        });
        Ok(targets)
    }

    async fn execute_inner(
        &self,
        principal: &UserPrincipal,
        input: ProviderDiscoveryInput,
    ) -> Result<ProviderDiscoveryOutput, ApiError> {
        let actor = principal.actor().clone();
        match input {
            ProviderDiscoveryInput::Models { id } => {
                Ok(ProviderDiscoveryOutput::Models(to_model_catalog_response(
                    self.service(&actor, true, "model_providers.instances.models.view")
                        .list_models(actor.user_id, parse_uuid(&id, "id")?)
                        .await?,
                )))
            }
            ProviderDiscoveryInput::RefreshModels { id } => {
                Ok(ProviderDiscoveryOutput::Models(to_model_catalog_response(
                    self.service(&actor, true, "model_providers.instances.models.refresh")
                        .refresh_models(actor.user_id, parse_uuid(&id, "id")?)
                        .await?,
                )))
            }
            ProviderDiscoveryInput::Options {
                query,
                locale,
                settings,
            } => {
                let preferred = self
                    .0
                    .store
                    .find_user_by_id(actor.user_id)
                    .await?
                    .ok_or(control_plane::errors::ControlPlaneError::NotAuthenticated)?
                    .preferred_locale;
                let locale_meta = locale.resolve(query.locale, preferred);
                let operation = if settings {
                    "model_providers.settings_options.view"
                } else {
                    "model_providers.options.view"
                };
                let options = self
                    .service(&actor, settings, operation)
                    .options(actor.user_id, requested_locales(&locale_meta))
                    .await?;
                Ok(ProviderDiscoveryOutput::Options(to_options_view_response(
                    locale_meta,
                    options,
                    self.pricing_targets().await?,
                )))
            }
        }
    }
}

impl ConsoleInterfacePort<ProviderDiscoveryInput, ProviderDiscoveryOutput>
    for ProviderDiscoveryAdapter
{
    fn execute<'a>(
        &'a self,
        principal: &'a UserPrincipal,
        input: ProviderDiscoveryInput,
    ) -> ConsoleInterfaceFuture<'a, ProviderDiscoveryOutput> {
        Box::pin(async move {
            self.execute_inner(principal, input)
                .await
                .map_err(ConsoleInterfaceTargetError)
        })
    }
}

const DECLARATIONS: &[ConsoleInterfaceDeclaration] = &[
    ConsoleInterfaceDeclaration {
        interface_id: "model-providers.instances.models.view",
        binding_id: "http.console.model-providers.instances.models.view.v1",
        method: "GET",
        path: "/api/console/settings/model-providers/instances/:id/models",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "model-providers.instances.models.refresh",
        binding_id: "http.console.model-providers.instances.models.refresh.v1",
        method: "POST",
        path: "/api/console/settings/model-providers/instances/:id/models/refresh",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "model-providers.options.view",
        binding_id: "http.console.model-providers.options.view.v1",
        method: "GET",
        path: "/api/console/model-providers/options",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "model-providers.settings-options.view",
        binding_id: "http.console.model-providers.settings-options.view.v1",
        method: "GET",
        path: "/api/console/settings/model-providers/options",
        mutating: false,
    },
];

pub(crate) fn compile_registry(
    dependencies: ProviderDiscoveryDependencies,
) -> Result<
    Arc<interface_runtime::CompiledInterfaceRegistry>,
    interface_runtime::RegistryCompilationError,
> {
    console_interface::compile_registry(
        "api-server.console-provider-discovery",
        "graph:console-provider-discovery-v1",
        DECLARATIONS,
        Arc::new(ProviderDiscoveryAdapter(dependencies)),
    )
}
