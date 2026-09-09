use std::sync::Arc;

use control_plane::ports::CacheStore;
use interface_runtime::{InterfaceContract, UserPrincipal};
use storage_durable_postgres::MainDurableStore;

use super::*;
use crate::routes::console_interface::{
    self, ConsoleInterfaceDeclaration, ConsoleInterfaceFuture, ConsoleInterfacePort,
    ConsoleInterfaceTargetError,
};

pub(crate) enum ProviderRoutingInput {
    Get {
        provider_code: String,
    },
    Update {
        provider_code: String,
        body: UpdateModelProviderMainInstanceBody,
    },
}
impl InterfaceContract for ProviderRoutingInput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::union_schema(vec![
            mp::object_schema(&[
                ("variant", mp::tag_schema("Get")),
                ("provider_code", mp::text_schema()),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Update")),
                ("provider_code", mp::text_schema()),
                (
                    "body",
                    mp::object_schema(&[
                        (
                            "auto_include_new_instances",
                            serde_json::json!({"type":"boolean"}),
                        ),
                        ("expected_revision", serde_json::json!({"type":"integer"})),
                        (
                            "model_routing_policies",
                            serde_json::json!({"anyOf": [serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("model_id",mp::text_schema()), ("distribution_rule",mp::object_schema(&[("byte_count",mp::count_schema())])), ("distribution_rule_contract_version",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("distribution_rule_config",mp::object_schema(&[("item_count",mp::count_schema())])), ("provider_instance_ids",serde_json::json!({"type":"array","maxItems":32,"items":mp::text_schema()})), ("excluded_provider_instance_ids",serde_json::json!({"type":"array","maxItems":32,"items":mp::text_schema()}))])}), {"type":"null"}]}),
                        ),
                    ]),
                ),
            ]),
        ]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(match self {
            Self::Get {
                provider_code: _field_provider_code,
                ..
            } => mp::object_value(&[
                ("variant", serde_json::Value::String("Get".to_owned())),
                ("provider_code", mp::text(_field_provider_code)?),
            ]),
            Self::Update {
                provider_code: _field_provider_code,
                body: _field_body,
                ..
            } => {
                mp::object_value(&[
                    ("variant", serde_json::Value::String("Update".to_owned())),
                    ("provider_code", mp::text(_field_provider_code)?),
                    (
                        "body",
                        mp::object_value(&[
                            (
                                "auto_include_new_instances",
                                serde_json::Value::Bool(
                                    *(&(_field_body).auto_include_new_instances),
                                ),
                            ),
                            (
                                "expected_revision",
                                serde_json::json!(*(&(_field_body).expected_revision)),
                            ),
                            (
                                "model_routing_policies",
                                match (&(_field_body).model_routing_policies).as_ref() {
                                    Some(item) => {
                                        if (item).len() > 32 {
                                            return None;
                                        }
                                        serde_json::Value::Array((item).iter().map(|item| Some(mp::object_value(&[("model_id",mp::text(&(item).model_id)?), ("distribution_rule",mp::object_value(&[("byte_count",serde_json::json!((&(item).distribution_rule).len()))])), ("distribution_rule_contract_version",match (&(item).distribution_rule_contract_version).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("distribution_rule_config",mp::object_value(&[("item_count",serde_json::json!((&(item).distribution_rule_config).len()))])), ("provider_instance_ids",{ if (&(item).provider_instance_ids).len() > 32 { return None; } serde_json::Value::Array((&(item).provider_instance_ids).iter().map(|item| Some(serde_json::Value::String((item).to_string()))).collect::<Option<Vec<_>>>()?) }), ("excluded_provider_instance_ids",{ if (&(item).excluded_provider_instance_ids).len() > 32 { return None; } serde_json::Value::Array((&(item).excluded_provider_instance_ids).iter().map(|item| Some(serde_json::Value::String((item).to_string()))).collect::<Option<Vec<_>>>()?) })]))).collect::<Option<Vec<_>>>()?)
                                    }
                                    None => serde_json::Value::Null,
                                },
                            ),
                        ]),
                    ),
                ])
            }
        })
    }

    const CONTRACT_ID: &'static str = "console-provider-routing-input";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) struct ProviderRoutingOutput(pub(crate) ModelProviderMainInstanceResponse);
impl InterfaceContract for ProviderRoutingOutput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::object_schema(&[(
            "0",
            mp::object_schema(&[
                ("provider_code", mp::text_schema()),
                (
                    "auto_include_new_instances",
                    serde_json::json!({"type":"boolean"}),
                ),
                ("revision", serde_json::json!({"type":"integer"})),
                (
                    "model_routing_policies",
                    serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("model_id",mp::text_schema()), ("distribution_rule",mp::object_schema(&[("byte_count",mp::count_schema())])), ("distribution_rule_id",mp::text_schema()), ("distribution_rule_contract_version",mp::text_schema()), ("distribution_rule_config",mp::object_schema(&[("item_count",mp::count_schema())])), ("provider_instance_ids",serde_json::json!({"type":"array","maxItems":32,"items":mp::text_schema()})), ("excluded_provider_instance_ids",serde_json::json!({"type":"array","maxItems":32,"items":mp::text_schema()}))])}),
                ),
                (
                    "distribution_rules",
                    serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("value",mp::object_schema(&[("byte_count",mp::count_schema())])), ("rule_id",mp::text_schema()), ("rule_version",mp::text_schema()), ("contract_version",mp::text_schema()), ("display_name",mp::object_schema(&[("byte_count",mp::count_schema())])), ("config_fields",mp::object_schema(&[("item_count",mp::count_schema())]))])}),
                ),
            ]),
        )]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::object_value(&[(
            "0",
            mp::object_value(&[
                ("provider_code", mp::text(&(&(self).0).provider_code)?),
                (
                    "auto_include_new_instances",
                    serde_json::Value::Bool(*(&(&(self).0).auto_include_new_instances)),
                ),
                ("revision", serde_json::json!(*(&(&(self).0).revision))),
                ("model_routing_policies", {
                    if (&(&(self).0).model_routing_policies).len() > 32 {
                        return None;
                    }
                    serde_json::Value::Array(
                        (&(&(self).0).model_routing_policies)
                            .iter()
                            .map(|item| {
                                Some(mp::object_value(&[
                                    ("model_id", mp::text(&(item).model_id)?),
                                    (
                                        "distribution_rule",
                                        mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((&(item).distribution_rule).len()),
                                        )]),
                                    ),
                                    (
                                        "distribution_rule_id",
                                        mp::text(&(item).distribution_rule_id)?,
                                    ),
                                    (
                                        "distribution_rule_contract_version",
                                        mp::text(&(item).distribution_rule_contract_version)?,
                                    ),
                                    (
                                        "distribution_rule_config",
                                        mp::object_value(&[(
                                            "item_count",
                                            serde_json::json!(
                                                (&(item).distribution_rule_config).len()
                                            ),
                                        )]),
                                    ),
                                    ("provider_instance_ids", {
                                        if (&(item).provider_instance_ids).len() > 32 {
                                            return None;
                                        }
                                        serde_json::Value::Array(
                                            (&(item).provider_instance_ids)
                                                .iter()
                                                .map(|item| {
                                                    Some(serde_json::Value::String(
                                                        (item).to_string(),
                                                    ))
                                                })
                                                .collect::<Option<Vec<_>>>()?,
                                        )
                                    }),
                                    ("excluded_provider_instance_ids", {
                                        if (&(item).excluded_provider_instance_ids).len() > 32 {
                                            return None;
                                        }
                                        serde_json::Value::Array(
                                            (&(item).excluded_provider_instance_ids)
                                                .iter()
                                                .map(|item| {
                                                    Some(serde_json::Value::String(
                                                        (item).to_string(),
                                                    ))
                                                })
                                                .collect::<Option<Vec<_>>>()?,
                                        )
                                    }),
                                ]))
                            })
                            .collect::<Option<Vec<_>>>()?,
                    )
                }),
                ("distribution_rules", {
                    if (&(&(self).0).distribution_rules).len() > 32 {
                        return None;
                    }
                    serde_json::Value::Array(
                        (&(&(self).0).distribution_rules)
                            .iter()
                            .map(|item| {
                                Some(mp::object_value(&[
                                    (
                                        "value",
                                        mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((&(item).value).len()),
                                        )]),
                                    ),
                                    ("rule_id", mp::text(&(item).rule_id)?),
                                    ("rule_version", mp::text(&(item).rule_version)?),
                                    ("contract_version", mp::text(&(item).contract_version)?),
                                    (
                                        "display_name",
                                        mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((&(item).display_name).len()),
                                        )]),
                                    ),
                                    (
                                        "config_fields",
                                        mp::object_value(&[(
                                            "item_count",
                                            serde_json::json!((&(item).config_fields).len()),
                                        )]),
                                    ),
                                ]))
                            })
                            .collect::<Option<Vec<_>>>()?,
                    )
                }),
            ]),
        )]))
    }

    const CONTRACT_ID: &'static str = "console-provider-routing-output";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) struct ProviderRoutingDependencies {
    pub(crate) store: MainDurableStore,
    pub(crate) provider_runtime: Arc<crate::provider_runtime::ApiRuntimeServices>,
    pub(crate) secret_key: String,
    pub(crate) api_node_id: String,
    pub(crate) install_root: String,
    pub(crate) cache_store: Arc<dyn CacheStore>,
}

struct ProviderRoutingAdapter(ProviderRoutingDependencies);
impl ProviderRoutingAdapter {
    fn service(
        &self,
        actor: &domain::ActorContext,
        operation: &'static str,
    ) -> crate::app_state::ApiModelProviderService {
        ModelProviderService::for_console_operation(
            self.0.store.for_actor(actor.clone()),
            ApiProviderRuntime::new(self.0.provider_runtime.clone()),
            self.0.secret_key.clone(),
            domain::ConsolePolicyGroup::settings_feature("system.model-providers")
                .expect("compiled model-provider settings group must be valid"),
            operation,
        )
        .with_node_artifact_context(self.0.api_node_id.clone(), self.0.install_root.clone())
        .with_routing_cache_store(self.0.cache_store.clone())
    }

    async fn execute_inner(
        &self,
        principal: &UserPrincipal,
        input: ProviderRoutingInput,
    ) -> Result<ProviderRoutingOutput, ApiError> {
        let actor = principal.actor().clone();
        let view = match input {
            ProviderRoutingInput::Get { provider_code } => {
                self.service(&actor, "model_providers.main_instance.view")
                    .get_main_instance(actor.user_id, &provider_code)
                    .await?
            }
            ProviderRoutingInput::Update {
                provider_code,
                body,
            } => {
                let model_routing_policies = if let Some(policies) = body.model_routing_policies {
                    let mut compiled = Vec::with_capacity(policies.len());
                    for policy in policies {
                        let rule_version = if !matches!(
                            policy.distribution_rule.as_str(),
                            "none"
                                | "builtin.none"
                                | "round_robin"
                                | "builtin.round_robin"
                                | "retry_round_robin"
                                | "builtin.retry_round_robin"
                        ) {
                            let contract_version = policy
                                .distribution_rule_contract_version
                                .as_deref()
                                .filter(|value| !value.trim().is_empty())
                                .ok_or(control_plane::errors::ControlPlaneError::InvalidInput(
                                    "distribution_rule_contract_version",
                                ))?;
                            let config = policy.distribution_rule_config.iter().map(|(key, value)| {
                                let value = match value {
                                    ModelProviderDistributionConfigValueBody::String(value) => extension_contracts::ProviderDistributionConfigValue::String(value.clone()),
                                    ModelProviderDistributionConfigValueBody::Integer(value) => extension_contracts::ProviderDistributionConfigValue::Integer(*value),
                                    ModelProviderDistributionConfigValueBody::Boolean(value) => extension_contracts::ProviderDistributionConfigValue::Boolean(*value),
                                };
                                (key.clone(), value)
                            }).collect();
                            self.0
                                .provider_runtime
                                .validate_provider_distribution_rule(
                                    &policy.distribution_rule,
                                    contract_version,
                                    &config,
                                )
                                .await
                                .map_err(|_| {
                                    control_plane::errors::ControlPlaneError::InvalidInput(
                                        "distribution_rule",
                                    )
                                })?
                                .into()
                        } else {
                            None
                        };
                        compiled.push(to_main_model_routing_policy(policy, rule_version)?);
                    }
                    Some(compiled)
                } else {
                    None
                };
                self.service(&actor, "model_providers.main_instance.update")
                    .update_main_instance(UpdateModelProviderMainInstanceCommand {
                        actor_user_id: actor.user_id,
                        provider_code,
                        auto_include_new_instances: body.auto_include_new_instances,
                        expected_revision: body.expected_revision,
                        model_routing_policies,
                    })
                    .await?
            }
        };
        let definitions = self
            .0
            .provider_runtime
            .provider_distribution_definitions()
            .await;
        Ok(ProviderRoutingOutput(to_main_instance_response(
            view,
            definitions,
        )))
    }
}

impl ConsoleInterfacePort<ProviderRoutingInput, ProviderRoutingOutput> for ProviderRoutingAdapter {
    fn execute<'a>(
        &'a self,
        principal: &'a UserPrincipal,
        input: ProviderRoutingInput,
    ) -> ConsoleInterfaceFuture<'a, ProviderRoutingOutput> {
        Box::pin(async move {
            self.execute_inner(principal, input)
                .await
                .map_err(ConsoleInterfaceTargetError)
        })
    }
}

const DECLARATIONS: &[ConsoleInterfaceDeclaration] = &[
    ConsoleInterfaceDeclaration {
        interface_id: "model_providers.main_instance.view",
        binding_id: "http.console.model-providers.main-instance.view.v1",
        method: "GET",
        path: "/api/console/settings/model-providers/providers/:provider_code/main-instance",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "model_providers.main_instance.update",
        binding_id: "http.console.model-providers.main-instance.update.v1",
        method: "PUT",
        path: "/api/console/settings/model-providers/providers/:provider_code/main-instance",
        mutating: true,
    },
];

pub(crate) fn compile_registry(
    dependencies: ProviderRoutingDependencies,
) -> Result<
    Arc<interface_runtime::CompiledInterfaceRegistry>,
    interface_runtime::RegistryCompilationError,
> {
    console_interface::compile_registry(
        "api-server.console-provider-routing",
        "graph:console-provider-routing-v1",
        DECLARATIONS,
        Arc::new(ProviderRoutingAdapter(dependencies)),
    )
}
