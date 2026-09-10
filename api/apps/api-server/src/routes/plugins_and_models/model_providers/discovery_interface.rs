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
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::union_schema(vec![
            mp::object_schema(&[
                ("variant", mp::tag_schema("Models")),
                ("id", mp::text_schema()),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("RefreshModels")),
                ("id", mp::text_schema()),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Options")),
                (
                    "query",
                    mp::object_schema(&[(
                        "locale",
                        serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                    )]),
                ),
                ("settings", serde_json::json!({"type":"boolean"})),
            ]),
        ]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(match self {
            Self::Models { id: _field_id, .. } => mp::object_value(&[
                ("variant", serde_json::Value::String("Models".to_owned())),
                ("id", mp::text(_field_id)?),
            ]),
            Self::RefreshModels { id: _field_id, .. } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("RefreshModels".to_owned()),
                ),
                ("id", mp::text(_field_id)?),
            ]),
            Self::Options {
                query: _field_query,
                settings: _field_settings,
                ..
            } => mp::object_value(&[
                ("variant", serde_json::Value::String("Options".to_owned())),
                (
                    "query",
                    mp::object_value(&[(
                        "locale",
                        match (&(_field_query).locale).as_ref() {
                            Some(item) => mp::text(item)?,
                            None => serde_json::Value::Null,
                        },
                    )]),
                ),
                ("settings", serde_json::Value::Bool(*(_field_settings))),
            ]),
        })
    }

    const CONTRACT_ID: &'static str = "console-provider-discovery-input";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) enum ProviderDiscoveryOutput {
    Models(ModelProviderModelCatalogResponse),
    Options(ModelProviderOptionsResponse),
}
impl InterfaceContract for ProviderDiscoveryOutput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::union_schema(vec![
            mp::object_schema(&[
                ("variant", mp::tag_schema("Models")),
                (
                    "0",
                    mp::object_schema(&[
                        ("provider_instance_id", mp::text_schema()),
                        (
                            "refresh_status",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "source",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "last_error_message",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "refreshed_at",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "models",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("model_id",mp::text_schema()), ("display_name",mp::object_schema(&[("byte_count",mp::count_schema())])), ("namespace",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("label_key",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("description_key",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("display_name_fallback",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("source",mp::object_schema(&[("byte_count",mp::count_schema())])), ("supports_streaming",serde_json::json!({"type":"boolean"})), ("supports_tool_call",serde_json::json!({"type":"boolean"})), ("supports_multimodal",serde_json::json!({"type":"boolean"})), ("context_window",serde_json::json!({"anyOf": [serde_json::json!({"type":"integer"}), {"type":"null"}]})), ("provider_metadata",mp::json_summary_schema())])}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Options")),
                (
                    "0",
                    mp::object_schema(&[
                        (
                            "locale_meta",
                            mp::object_schema(&[
                                (
                                    "requested_locale",
                                    serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                ),
                                (
                                    "resolved_locale",
                                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                                ),
                                (
                                    "source",
                                    mp::union_schema(vec![
                                        mp::object_schema(&[("variant", mp::tag_schema("Query"))]),
                                        mp::object_schema(&[(
                                            "variant",
                                            mp::tag_schema("ExplicitHeader"),
                                        )]),
                                        mp::object_schema(&[(
                                            "variant",
                                            mp::tag_schema("UserPreferredLocale"),
                                        )]),
                                        mp::object_schema(&[(
                                            "variant",
                                            mp::tag_schema("AcceptLanguage"),
                                        )]),
                                        mp::object_schema(&[(
                                            "variant",
                                            mp::tag_schema("Fallback"),
                                        )]),
                                    ]),
                                ),
                                (
                                    "fallback_locale",
                                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                                ),
                                (
                                    "supported_locales",
                                    mp::object_schema(&[("item_count", mp::count_schema())]),
                                ),
                            ]),
                        ),
                        ("i18n_catalog", mp::json_summary_schema()),
                        (
                            "providers",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("provider_code",mp::text_schema()), ("plugin_type",mp::text_schema()), ("namespace",mp::object_schema(&[("byte_count",mp::count_schema())])), ("label_key",mp::object_schema(&[("byte_count",mp::count_schema())])), ("description_key",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("protocol",mp::text_schema()), ("display_name",mp::object_schema(&[("byte_count",mp::count_schema())])), ("icon",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("parameter_form",serde_json::json!({"anyOf": [mp::object_schema(&[("schema_version",mp::text_schema()), ("title",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("description",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("fields",mp::object_schema(&[("item_count",mp::count_schema())]))]), {"type":"null"}]})), ("main_instance",mp::object_schema(&[("provider_code",mp::text_schema()), ("auto_include_new_instances",serde_json::json!({"type":"boolean"})), ("group_count",serde_json::json!({"type":"integer"})), ("model_count",serde_json::json!({"type":"integer"}))])), ("model_groups",mp::object_schema(&[("item_count",mp::count_schema())]))])}),
                        ),
                        (
                            "pricing_targets",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("provider_code",mp::text_schema()), ("upstream_model_id",mp::text_schema()), ("effective_from",mp::object_schema(&[("byte_count",mp::count_schema())])), ("effective_to",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("timezone",mp::object_schema(&[("byte_count",mp::count_schema())])), ("weekday_mask",serde_json::json!({"type":"integer"})), ("local_time_start",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("local_time_end",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("rating_policy_enabled",serde_json::json!({"type":"boolean"})), ("rating_policy",mp::json_summary_schema())])}),
                        ),
                    ]),
                ),
            ]),
        ]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(match self {Self::Models(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("Models".to_owned())), ("0",mp::object_value(&[("provider_instance_id",mp::text(&(_field_0).provider_instance_id)?), ("refresh_status",mp::object_value(&[("byte_count",serde_json::json!((&(_field_0).refresh_status).len()))])), ("source",mp::object_value(&[("byte_count",serde_json::json!((&(_field_0).source).len()))])), ("last_error_message",match (&(_field_0).last_error_message).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("refreshed_at",match (&(_field_0).refreshed_at).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("models",{ if (&(_field_0).models).len() > 32 { return None; } serde_json::Value::Array((&(_field_0).models).iter().map(|item| Some(mp::object_value(&[("model_id",mp::text(&(item).model_id)?), ("display_name",mp::object_value(&[("byte_count",serde_json::json!((&(item).display_name).len()))])), ("namespace",match (&(item).namespace).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("label_key",match (&(item).label_key).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("description_key",match (&(item).description_key).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("display_name_fallback",match (&(item).display_name_fallback).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("source",mp::object_value(&[("byte_count",serde_json::json!((&(item).source).len()))])), ("supports_streaming",serde_json::Value::Bool(*(&(item).supports_streaming))), ("supports_tool_call",serde_json::Value::Bool(*(&(item).supports_tool_call))), ("supports_multimodal",serde_json::Value::Bool(*(&(item).supports_multimodal))), ("context_window",match (&(item).context_window).as_ref() { Some(item) => serde_json::json!(*(item)), None => serde_json::Value::Null }), ("provider_metadata",mp::json_summary(&(item).provider_metadata))]))).collect::<Option<Vec<_>>>()?) })]))]), Self::Options(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("Options".to_owned())), ("0",mp::object_value(&[("locale_meta",mp::object_value(&[("requested_locale",match (&(&(_field_0).locale_meta).requested_locale).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("resolved_locale",mp::object_value(&[("byte_count",serde_json::json!((&(&(_field_0).locale_meta).resolved_locale).len()))])), ("source",match &(&(_field_0).locale_meta).source {crate::routes::settings_group::system::LocaleSourceResponse::Query => mp::object_value(&[("variant",serde_json::Value::String("Query".to_owned()))]), crate::routes::settings_group::system::LocaleSourceResponse::ExplicitHeader => mp::object_value(&[("variant",serde_json::Value::String("ExplicitHeader".to_owned()))]), crate::routes::settings_group::system::LocaleSourceResponse::UserPreferredLocale => mp::object_value(&[("variant",serde_json::Value::String("UserPreferredLocale".to_owned()))]), crate::routes::settings_group::system::LocaleSourceResponse::AcceptLanguage => mp::object_value(&[("variant",serde_json::Value::String("AcceptLanguage".to_owned()))]), crate::routes::settings_group::system::LocaleSourceResponse::Fallback => mp::object_value(&[("variant",serde_json::Value::String("Fallback".to_owned()))])}), ("fallback_locale",mp::object_value(&[("byte_count",serde_json::json!((&(&(_field_0).locale_meta).fallback_locale).len()))])), ("supported_locales",mp::object_value(&[("item_count",serde_json::json!((&(&(_field_0).locale_meta).supported_locales).len()))]))])), ("i18n_catalog",mp::json_summary(&(_field_0).i18n_catalog)), ("providers",{ if (&(_field_0).providers).len() > 32 { return None; } serde_json::Value::Array((&(_field_0).providers).iter().map(|item| Some(mp::object_value(&[("provider_code",mp::text(&(item).provider_code)?), ("plugin_type",mp::text(&(item).plugin_type)?), ("namespace",mp::object_value(&[("byte_count",serde_json::json!((&(item).namespace).len()))])), ("label_key",mp::object_value(&[("byte_count",serde_json::json!((&(item).label_key).len()))])), ("description_key",match (&(item).description_key).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("protocol",mp::text(&(item).protocol)?), ("display_name",mp::object_value(&[("byte_count",serde_json::json!((&(item).display_name).len()))])), ("icon",match (&(item).icon).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("parameter_form",match (&(item).parameter_form).as_ref() { Some(item) => mp::object_value(&[("schema_version",mp::text(&(item).schema_version)?), ("title",match (&(item).title).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("description",match (&(item).description).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("fields",mp::object_value(&[("item_count",serde_json::json!((&(item).fields).len()))]))]), None => serde_json::Value::Null }), ("main_instance",mp::object_value(&[("provider_code",mp::text(&(&(item).main_instance).provider_code)?), ("auto_include_new_instances",serde_json::Value::Bool(*(&(&(item).main_instance).auto_include_new_instances))), ("group_count",serde_json::json!(*(&(&(item).main_instance).group_count))), ("model_count",serde_json::json!(*(&(&(item).main_instance).model_count)))])), ("model_groups",mp::object_value(&[("item_count",serde_json::json!((&(item).model_groups).len()))]))]))).collect::<Option<Vec<_>>>()?) }), ("pricing_targets",{ if (&(_field_0).pricing_targets).len() > 32 { return None; } serde_json::Value::Array((&(_field_0).pricing_targets).iter().map(|item| Some(mp::object_value(&[("provider_code",mp::text(&(item).provider_code)?), ("upstream_model_id",mp::text(&(item).upstream_model_id)?), ("effective_from",mp::object_value(&[("byte_count",serde_json::json!((&(item).effective_from).len()))])), ("effective_to",match (&(item).effective_to).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("timezone",mp::object_value(&[("byte_count",serde_json::json!((&(item).timezone).len()))])), ("weekday_mask",serde_json::json!(*(&(item).weekday_mask))), ("local_time_start",match (&(item).local_time_start).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("local_time_end",match (&(item).local_time_end).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("rating_policy_enabled",serde_json::Value::Bool(*(&(item).rating_policy_enabled))), ("rating_policy",mp::json_summary(&(item).rating_policy))]))).collect::<Option<Vec<_>>>()?) })]))])})
    }

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
        interface_id: "model_providers.instances.models.view",
        binding_id: "http.console.model-providers.instances.models.view.v1",
        method: "GET",
        path: "/api/console/settings/model-providers/instances/:id/models",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "model_providers.instances.models.refresh",
        binding_id: "http.console.model-providers.instances.models.refresh.v1",
        method: "POST",
        path: "/api/console/settings/model-providers/instances/:id/models/refresh",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "model_providers.options.view",
        binding_id: "http.console.model-providers.options.view.v1",
        method: "GET",
        path: "/api/console/model-providers/options",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "model_providers.settings_options.view",
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
