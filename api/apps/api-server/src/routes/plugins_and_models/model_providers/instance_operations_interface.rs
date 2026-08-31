use std::sync::Arc;

use control_plane::ports::CacheStore;
use interface_runtime::{InterfaceContract, UserPrincipal};
use storage_durable_postgres::MainDurableStore;

use super::*;
use crate::routes::console_interface::{
    self, ConsoleInterfaceDeclaration, ConsoleInterfaceFuture, ConsoleInterfacePort,
    ConsoleInterfaceTargetError,
};

pub(crate) enum ProviderInstanceOperationsInput {
    Authenticate {
        id: String,
        body: AuthenticateModelProviderInstanceBody,
    },
    Usage {
        id: String,
    },
    ResetCredits {
        id: String,
    },
    ConsumeResetCredit {
        id: String,
        body: ConsumeModelProviderResetCreditBody,
    },
    Balance {
        id: String,
    },
    Preview(PreviewModelProviderModelsBody),
    Reveal {
        id: String,
        body: RevealModelProviderSecretBody,
    },
}

impl InterfaceContract for ProviderInstanceOperationsInput {
    const CONTRACT_ID: &'static str = "console-provider-instance-operations-input";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) enum ProviderInstanceOperationsOutput {
    Authentication(AuthenticateModelProviderInstanceResponse),
    Usage(ModelProviderUsageWindowsResponse),
    ResetCredits(ModelProviderResetCreditCountResponse),
    Consumed(ConsumeModelProviderResetCreditResponse),
    Balance(ModelProviderBalanceResponse),
    Preview(PreviewModelProviderModelsResponse),
    Secret(RevealModelProviderSecretResponse),
}

impl InterfaceContract for ProviderInstanceOperationsOutput {
    const CONTRACT_ID: &'static str = "console-provider-instance-operations-output";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) struct ProviderInstanceOperationsDependencies {
    pub(crate) store: MainDurableStore,
    pub(crate) provider_runtime: Arc<crate::provider_runtime::ApiRuntimeServices>,
    pub(crate) secret_key: String,
    pub(crate) api_node_id: String,
    pub(crate) install_root: String,
    pub(crate) cache_store: Arc<dyn CacheStore>,
}

struct ProviderInstanceOperationsAdapter(ProviderInstanceOperationsDependencies);

impl ProviderInstanceOperationsAdapter {
    fn service(
        &self,
        actor: &domain::ActorContext,
        group: &'static str,
        operation: &'static str,
    ) -> crate::app_state::ApiModelProviderService {
        ModelProviderService::for_console_operation(
            self.0.store.for_actor(actor.clone()),
            ApiProviderRuntime::new(self.0.provider_runtime.clone()),
            self.0.secret_key.clone(),
            if group == "settings" {
                domain::ConsolePolicyGroup::settings_feature("system.model-providers")
                    .expect("compiled model-provider settings group must be valid")
            } else {
                domain::ConsolePolicyGroup::other("other.model-providers")
                    .expect("compiled model-provider other group must be valid")
            },
            operation,
        )
        .with_node_artifact_context(self.0.api_node_id.clone(), self.0.install_root.clone())
        .with_routing_cache_store(self.0.cache_store.clone())
    }

    async fn execute_inner(
        &self,
        principal: &UserPrincipal,
        input: ProviderInstanceOperationsInput,
    ) -> Result<ProviderInstanceOperationsOutput, ApiError> {
        let actor = principal.actor().clone();
        let user_id = actor.user_id;
        match input {
            ProviderInstanceOperationsInput::Authenticate { id, body } => {
                let operation = serde_json::from_value::<ProviderAuthOperation>(body.operation)
                    .map_err(|_| {
                        control_plane::errors::ControlPlaneError::InvalidInput(
                            "provider_auth_operation",
                        )
                    })?;
                let result = self
                    .service(&actor, "settings", "model_providers.instances.authenticate")
                    .authenticate_instance(AuthenticateModelProviderInstanceCommand {
                        actor_user_id: user_id,
                        instance_id: parse_uuid(&id, "id")?,
                        operation,
                    })
                    .await?;
                Ok(ProviderInstanceOperationsOutput::Authentication(
                    to_authenticate_response(result),
                ))
            }
            ProviderInstanceOperationsInput::Usage { id } => {
                let result = self
                    .service(&actor, "settings", "model_providers.instances.usage.view")
                    .get_usage_windows(user_id, parse_uuid(&id, "id")?)
                    .await?;
                Ok(ProviderInstanceOperationsOutput::Usage(
                    to_usage_windows_response(result),
                ))
            }
            ProviderInstanceOperationsInput::ResetCredits { id } => {
                let result = self
                    .service(
                        &actor,
                        "settings",
                        "model_providers.instances.reset_credits.view",
                    )
                    .count_reset_credits(user_id, parse_uuid(&id, "id")?)
                    .await?;
                Ok(ProviderInstanceOperationsOutput::ResetCredits(
                    to_reset_credit_count_response(result),
                ))
            }
            ProviderInstanceOperationsInput::ConsumeResetCredit { id, body } => {
                let result = self
                    .service(
                        &actor,
                        "settings",
                        "model_providers.instances.reset_credits.consume",
                    )
                    .consume_reset_credit(ConsumeModelProviderResetCreditCommand {
                        actor_user_id: user_id,
                        instance_id: parse_uuid(&id, "id")?,
                        idempotency_key: body.idempotency_key,
                    })
                    .await?;
                Ok(ProviderInstanceOperationsOutput::Consumed(
                    to_consume_reset_credit_response(result),
                ))
            }
            ProviderInstanceOperationsInput::Balance { id } => {
                let result = self
                    .service(&actor, "other", "model_providers.balance.view")
                    .get_balance(user_id, parse_uuid(&id, "id")?)
                    .await?;
                Ok(ProviderInstanceOperationsOutput::Balance(
                    to_balance_response(result),
                ))
            }
            ProviderInstanceOperationsInput::Preview(body) => {
                let preview = self
                    .service(&actor, "settings", "model_providers.preview.view")
                    .preview_models(PreviewModelProviderModelsCommand {
                        actor_user_id: user_id,
                        installation_id: body
                            .installation_id
                            .as_deref()
                            .map(|raw| parse_uuid(raw, "installation_id"))
                            .transpose()?,
                        instance_id: body
                            .instance_id
                            .as_deref()
                            .map(|raw| parse_uuid(raw, "instance_id"))
                            .transpose()?,
                        config_json: body.config,
                    })
                    .await?;
                Ok(ProviderInstanceOperationsOutput::Preview(
                    PreviewModelProviderModelsResponse {
                        models: preview
                            .models
                            .into_iter()
                            .map(to_runtime_model_descriptor_response)
                            .collect(),
                        preview_token: preview.preview_token.to_string(),
                        expires_at: format_optional_time(Some(preview.expires_at))
                            .unwrap_or_default(),
                    },
                ))
            }
            ProviderInstanceOperationsInput::Reveal { id, body } => {
                let value = self
                    .service(
                        &actor,
                        "settings",
                        "model_providers.instances.secrets.reveal",
                    )
                    .reveal_secret(user_id, parse_uuid(&id, "id")?, &body.key)
                    .await?;
                Ok(ProviderInstanceOperationsOutput::Secret(
                    RevealModelProviderSecretResponse {
                        key: body.key,
                        value,
                    },
                ))
            }
        }
    }
}

impl ConsoleInterfacePort<ProviderInstanceOperationsInput, ProviderInstanceOperationsOutput>
    for ProviderInstanceOperationsAdapter
{
    fn execute<'a>(
        &'a self,
        principal: &'a UserPrincipal,
        input: ProviderInstanceOperationsInput,
    ) -> ConsoleInterfaceFuture<'a, ProviderInstanceOperationsOutput> {
        Box::pin(async move {
            self.execute_inner(principal, input)
                .await
                .map_err(ConsoleInterfaceTargetError)
        })
    }
}

const DECLARATIONS: &[ConsoleInterfaceDeclaration] = &[
    ConsoleInterfaceDeclaration {
        interface_id: "model_providers.instances.authenticate",
        binding_id: "http.console.model-providers.instances.authenticate.v1",
        method: "POST",
        path: "/api/console/settings/model-providers/instances/:id/authenticate",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "model_providers.instances.usage.view",
        binding_id: "http.console.model-providers.instances.usage.view.v1",
        method: "GET",
        path: "/api/console/settings/model-providers/instances/:id/usage",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "model_providers.instances.reset_credits.view",
        binding_id: "http.console.model-providers.instances.reset-credits.view.v1",
        method: "GET",
        path: "/api/console/settings/model-providers/instances/:id/reset-credits",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "model_providers.instances.reset_credits.consume",
        binding_id: "http.console.model-providers.instances.reset-credits.consume.v1",
        method: "POST",
        path: "/api/console/settings/model-providers/instances/:id/reset-credits/consume",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "model_providers.balance.view",
        binding_id: "http.console.model-providers.balance.view.v1",
        method: "GET",
        path: "/api/console/model-providers/:id/balance",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "model_providers.preview.view",
        binding_id: "http.console.model-providers.preview.view.v1",
        method: "POST",
        path: "/api/console/settings/model-providers/preview-models",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "model_providers.instances.secrets.reveal",
        binding_id: "http.console.model-providers.instances.secrets.reveal.v1",
        method: "POST",
        path: "/api/console/settings/model-providers/instances/:id/secrets/reveal",
        mutating: true,
    },
];

pub(crate) fn compile_registry(
    dependencies: ProviderInstanceOperationsDependencies,
) -> Result<
    Arc<interface_runtime::CompiledInterfaceRegistry>,
    interface_runtime::RegistryCompilationError,
> {
    console_interface::compile_registry(
        "api-server.console-provider-instance-operations",
        "graph:console-provider-instance-operations-v1",
        DECLARATIONS,
        Arc::new(ProviderInstanceOperationsAdapter(dependencies)),
    )
}
