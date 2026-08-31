use std::sync::Arc;

use control_plane::ports::CacheStore;
use interface_runtime::{InterfaceContract, UserPrincipal};
use storage_durable_postgres::MainDurableStore;

use super::*;
use crate::routes::console_interface::{
    self, ConsoleInterfaceDeclaration, ConsoleInterfaceFuture, ConsoleInterfacePort,
    ConsoleInterfaceTargetError,
};

pub(crate) enum ProviderInstanceLifecycleInput {
    List,
    Create(CreateModelProviderBody),
    Update {
        id: String,
        body: UpdateModelProviderBody,
    },
    Validate {
        id: String,
    },
    Delete {
        id: String,
    },
}

impl InterfaceContract for ProviderInstanceLifecycleInput {
    const CONTRACT_ID: &'static str = "console-provider-instance-lifecycle-input";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) enum ProviderInstanceLifecycleOutput {
    Instances(Vec<ModelProviderInstanceResponse>),
    Instance(ModelProviderInstanceResponse),
    Validation(ValidateModelProviderResponse),
    Deleted(DeletedResponse),
}

impl InterfaceContract for ProviderInstanceLifecycleOutput {
    const CONTRACT_ID: &'static str = "console-provider-instance-lifecycle-output";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) struct ProviderInstanceLifecycleDependencies {
    pub(crate) store: MainDurableStore,
    pub(crate) provider_runtime: Arc<crate::provider_runtime::ApiRuntimeServices>,
    pub(crate) secret_key: String,
    pub(crate) api_node_id: String,
    pub(crate) install_root: String,
    pub(crate) cache_store: Arc<dyn CacheStore>,
}

struct ProviderInstanceLifecycleAdapter(ProviderInstanceLifecycleDependencies);

impl ProviderInstanceLifecycleAdapter {
    fn service(
        &self,
        actor: &domain::ActorContext,
        operation_id: &'static str,
    ) -> crate::app_state::ApiModelProviderService {
        ModelProviderService::for_console_operation(
            self.0.store.for_actor(actor.clone()),
            ApiProviderRuntime::new(self.0.provider_runtime.clone()),
            self.0.secret_key.clone(),
            domain::ConsolePolicyGroup::settings_feature("system.model-providers")
                .expect("compiled model-provider settings group must be valid"),
            operation_id,
        )
        .with_node_artifact_context(self.0.api_node_id.clone(), self.0.install_root.clone())
        .with_routing_cache_store(self.0.cache_store.clone())
    }

    async fn execute_inner(
        &self,
        principal: &UserPrincipal,
        input: ProviderInstanceLifecycleInput,
    ) -> Result<ProviderInstanceLifecycleOutput, ApiError> {
        let actor = principal.actor().clone();
        let user_id = actor.user_id;
        match input {
            ProviderInstanceLifecycleInput::List => {
                let instances = self
                    .service(&actor, "model_providers.instances.view")
                    .list_instances(user_id)
                    .await?;
                Ok(ProviderInstanceLifecycleOutput::Instances(
                    instances.into_iter().map(to_instance_response).collect(),
                ))
            }
            ProviderInstanceLifecycleInput::Create(body) => {
                let created = self
                    .service(&actor, "model_providers.instances.create")
                    .create_instance(CreateModelProviderInstanceCommand {
                        actor_user_id: user_id,
                        installation_id: parse_uuid(&body.installation_id, "installation_id")?,
                        display_name: body.display_name,
                        config_json: body.config,
                        configured_models: configured_models(body.configured_models),
                        enabled_model_ids: body.enabled_model_ids,
                        included_in_main: body.included_in_main,
                        preview_token: body
                            .preview_token
                            .as_deref()
                            .map(|raw| parse_uuid(raw, "preview_token"))
                            .transpose()?,
                    })
                    .await?;
                Ok(ProviderInstanceLifecycleOutput::Instance(
                    to_instance_response(created),
                ))
            }
            ProviderInstanceLifecycleInput::Update { id, body } => {
                let updated = self
                    .service(&actor, "model_providers.instances.update")
                    .update_instance(UpdateModelProviderInstanceCommand {
                        actor_user_id: user_id,
                        instance_id: parse_uuid(&id, "id")?,
                        display_name: body.display_name,
                        config_json: body.config,
                        configured_models: configured_models(body.configured_models),
                        enabled_model_ids: body.enabled_model_ids,
                        included_in_main: body.included_in_main,
                        preview_token: body
                            .preview_token
                            .as_deref()
                            .map(|raw| parse_uuid(raw, "preview_token"))
                            .transpose()?,
                    })
                    .await?;
                Ok(ProviderInstanceLifecycleOutput::Instance(
                    to_instance_response(updated),
                ))
            }
            ProviderInstanceLifecycleInput::Validate { id } => {
                let result = self
                    .service(&actor, "model_providers.instances.validate")
                    .validate_instance(user_id, parse_uuid(&id, "id")?)
                    .await?;
                Ok(ProviderInstanceLifecycleOutput::Validation(
                    to_validate_response(result),
                ))
            }
            ProviderInstanceLifecycleInput::Delete { id } => {
                self.service(&actor, "model_providers.instances.delete")
                    .delete_instance(DeleteModelProviderInstanceCommand {
                        actor_user_id: user_id,
                        instance_id: parse_uuid(&id, "id")?,
                    })
                    .await?;
                Ok(ProviderInstanceLifecycleOutput::Deleted(DeletedResponse {
                    deleted: true,
                }))
            }
        }
    }
}

fn configured_models(
    models: Vec<ConfiguredModelBody>,
) -> Vec<domain::ModelProviderConfiguredModel> {
    models
        .into_iter()
        .map(|model| domain::ModelProviderConfiguredModel {
            model_id: model.model_id,
            enabled: model.enabled,
            context_window_override_tokens: model.context_window_override_tokens,
            supports_multimodal: model.supports_multimodal,
            pricing_provider_code: model.pricing_provider_code,
            pricing_model_id: model.pricing_model_id,
        })
        .collect()
}

impl ConsoleInterfacePort<ProviderInstanceLifecycleInput, ProviderInstanceLifecycleOutput>
    for ProviderInstanceLifecycleAdapter
{
    fn execute<'a>(
        &'a self,
        principal: &'a UserPrincipal,
        input: ProviderInstanceLifecycleInput,
    ) -> ConsoleInterfaceFuture<'a, ProviderInstanceLifecycleOutput> {
        Box::pin(async move {
            self.execute_inner(principal, input)
                .await
                .map_err(ConsoleInterfaceTargetError)
        })
    }
}

const DECLARATIONS: &[ConsoleInterfaceDeclaration] = &[
    ConsoleInterfaceDeclaration {
        interface_id: "model_providers.instances.view",
        binding_id: "http.console.model-providers.instances.view.v1",
        method: "GET",
        path: "/api/console/settings/model-providers/instances",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "model_providers.instances.create",
        binding_id: "http.console.model-providers.instances.create.v1",
        method: "POST",
        path: "/api/console/settings/model-providers/instances",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "model_providers.instances.update",
        binding_id: "http.console.model-providers.instances.update.v1",
        method: "PATCH",
        path: "/api/console/settings/model-providers/instances/:id",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "model_providers.instances.validate",
        binding_id: "http.console.model-providers.instances.validate.v1",
        method: "POST",
        path: "/api/console/settings/model-providers/instances/:id/validate",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "model_providers.instances.delete",
        binding_id: "http.console.model-providers.instances.delete.v1",
        method: "DELETE",
        path: "/api/console/settings/model-providers/instances/:id",
        mutating: true,
    },
];

pub(crate) fn compile_registry(
    dependencies: ProviderInstanceLifecycleDependencies,
) -> Result<
    Arc<interface_runtime::CompiledInterfaceRegistry>,
    interface_runtime::RegistryCompilationError,
> {
    console_interface::compile_registry(
        "api-server.console-provider-instance-lifecycle",
        "graph:console-provider-instance-lifecycle-v1",
        DECLARATIONS,
        Arc::new(ProviderInstanceLifecycleAdapter(dependencies)),
    )
}
