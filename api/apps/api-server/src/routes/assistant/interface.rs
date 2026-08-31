use std::sync::Arc;

use control_plane::profile::{ProfileService, UpdateMeMetaCommand};
use interface_runtime::{InterfaceContract, UserPrincipal};
use serde_json::json;
use storage_durable_postgres::MainDurableStore;

use super::{
    assistant_run_capabilities, available_targets, read_preference, validate_preference,
    AssistantPreferenceBody, AssistantSettingsResponse, ASSISTANT_META_KEY,
};
use crate::{
    error_response::ApiError,
    routes::console_interface::{
        self, ConsoleInterfaceDeclaration, ConsoleInterfaceFuture, ConsoleInterfacePort,
        ConsoleInterfaceTargetError,
    },
};

pub(crate) enum AssistantSettingsInput {
    Get,
    Update(AssistantPreferenceBody),
}

impl InterfaceContract for AssistantSettingsInput {
    const CONTRACT_ID: &'static str = "console-assistant-settings-input";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) enum AssistantSettingsOutput {
    Settings(AssistantSettingsResponse),
}

impl InterfaceContract for AssistantSettingsOutput {
    const CONTRACT_ID: &'static str = "console-assistant-settings-output";
    const CONTRACT_VERSION: &'static str = "1";
}

struct AssistantSettingsAdapter {
    store: MainDurableStore,
}

pub(crate) fn settings_port(
    store: MainDurableStore,
) -> Arc<dyn ConsoleInterfacePort<AssistantSettingsInput, AssistantSettingsOutput>> {
    Arc::new(AssistantSettingsAdapter { store })
}

impl AssistantSettingsAdapter {
    async fn settings_response(
        &self,
        actor: &domain::ActorContext,
        preference: AssistantPreferenceBody,
    ) -> Result<AssistantSettingsResponse, ApiError> {
        let (published_agent_flows, enabled_mcp_instances) =
            available_targets(&self.store, actor).await?;
        let run_capabilities =
            assistant_run_capabilities(&self.store, actor, preference.application_id).await?;
        Ok(AssistantSettingsResponse {
            preference,
            published_agent_flows,
            enabled_mcp_instances,
            page_reference_max_bytes:
                control_plane::application_public_api::run_service::ASSISTANT_PAGE_REFERENCE_MAX_BYTES,
            page_reference_max_count:
                control_plane::application_public_api::run_service::ASSISTANT_PAGE_REFERENCE_MAX_COUNT,
            page_reference_max_total_bytes:
                control_plane::application_public_api::run_service::ASSISTANT_PAGE_REFERENCE_MAX_TOTAL_BYTES,
            run_capabilities,
        })
    }

    async fn execute_inner(
        &self,
        principal: &UserPrincipal,
        input: AssistantSettingsInput,
    ) -> Result<AssistantSettingsOutput, ApiError> {
        let actor = principal.actor();
        let user = self
            .store
            .find_user_by_id(actor.user_id)
            .await?
            .ok_or(control_plane::errors::ControlPlaneError::NotFound("user"))?;
        let current_preference = read_preference(&user.meta, actor.current_workspace_id);
        let preference = match input {
            AssistantSettingsInput::Get => current_preference,
            AssistantSettingsInput::Update(preference) => {
                let preference = if current_preference.application_id != preference.application_id {
                    AssistantPreferenceBody {
                        model: None,
                        reasoning_effort: None,
                        ..preference
                    }
                } else {
                    preference
                };
                validate_preference(&self.store, actor, &preference).await?;
                let workspace_id = actor.current_workspace_id;
                let meta_patch = json!({
                    ASSISTANT_META_KEY: { "workspaces": { workspace_id.to_string(): preference } }
                });
                ProfileService::new(self.store.clone())
                    .update_me_meta(UpdateMeMetaCommand {
                        actor_user_id: actor.user_id,
                        tenant_id: actor.tenant_id,
                        workspace_id,
                        meta_patch,
                    })
                    .await?;
                preference
            }
        };
        Ok(AssistantSettingsOutput::Settings(
            self.settings_response(actor, preference).await?,
        ))
    }
}

impl ConsoleInterfacePort<AssistantSettingsInput, AssistantSettingsOutput>
    for AssistantSettingsAdapter
{
    fn execute<'a>(
        &'a self,
        principal: &'a UserPrincipal,
        input: AssistantSettingsInput,
    ) -> ConsoleInterfaceFuture<'a, AssistantSettingsOutput> {
        Box::pin(async move {
            self.execute_inner(principal, input)
                .await
                .map_err(ConsoleInterfaceTargetError)
        })
    }
}

pub(crate) const DECLARATIONS: &[ConsoleInterfaceDeclaration] = &[
    ConsoleInterfaceDeclaration {
        interface_id: "assistant.settings.get",
        binding_id: "http.console.assistant.settings.get.v1",
        method: "GET",
        path: "/api/console/assistant/settings",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "assistant.settings.update",
        binding_id: "http.console.assistant.settings.update.v1",
        method: "PATCH",
        path: "/api/console/assistant/settings",
        mutating: true,
    },
];

pub(crate) fn compile_registry(
    store: MainDurableStore,
) -> Result<
    Arc<interface_runtime::CompiledInterfaceRegistry>,
    interface_runtime::RegistryCompilationError,
> {
    compile_registry_with_port(settings_port(store))
}

fn compile_registry_with_port(
    port: Arc<dyn ConsoleInterfacePort<AssistantSettingsInput, AssistantSettingsOutput>>,
) -> Result<
    Arc<interface_runtime::CompiledInterfaceRegistry>,
    interface_runtime::RegistryCompilationError,
> {
    console_interface::compile_registry(
        "api-server.console-assistant-settings",
        "graph:console-assistant-settings-v1",
        DECLARATIONS,
        port,
    )
}

#[cfg(test)]
struct UnavailableAssistantSettingsPort;

#[cfg(test)]
impl ConsoleInterfacePort<AssistantSettingsInput, AssistantSettingsOutput>
    for UnavailableAssistantSettingsPort
{
    fn execute<'a>(
        &'a self,
        _principal: &'a UserPrincipal,
        _input: AssistantSettingsInput,
    ) -> ConsoleInterfaceFuture<'a, AssistantSettingsOutput> {
        Box::pin(async {
            Err(ConsoleInterfaceTargetError(
                anyhow::anyhow!("assistant settings fixture unavailable").into(),
            ))
        })
    }
}

#[cfg(test)]
mod tests {
    use interface_runtime::BindingId;

    use super::*;

    #[test]
    fn f09b1a_registry_freezes_assistant_settings_bindings() {
        let registry =
            compile_registry_with_port(Arc::new(UnavailableAssistantSettingsPort)).unwrap();
        for declaration in DECLARATIONS {
            assert!(registry
                .binding(&BindingId::new(declaration.binding_id).unwrap())
                .is_some());
        }
        assert_eq!(registry.bindings().count(), DECLARATIONS.len());
    }
}
