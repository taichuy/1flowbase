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
    const CONTRACT_ID: &'static str = "console-provider-routing-input";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) struct ProviderRoutingOutput(pub(crate) ModelProviderMainInstanceResponse);
impl InterfaceContract for ProviderRoutingOutput {
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
