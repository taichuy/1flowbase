use std::sync::Arc;

use control_plane::{
    application::ApplicationService,
    errors::ControlPlaneError,
    flow::FlowService,
    ports::{
        DebugVariableCacheKey, DeleteDebugVariableCacheEntriesInput,
        OrchestrationRuntimeRepository, UpsertDebugVariableCacheEntryInput,
    },
};
use interface_runtime::{InterfaceContract, UserPrincipal};
use storage_durable_postgres::MainDurableStore;
use uuid::Uuid;

use super::{
    debug_variable_cache::{
        DeleteDebugVariableCacheEntriesBody, UpsertDebugVariableCacheEntryBody,
    },
    debug_variable_snapshot::{build_debug_variable_snapshot, DebugVariableSnapshotResponse},
};
use crate::{
    error_response::ApiError,
    routes::console_interface::{
        self, ConsoleInterfaceDeclaration, ConsoleInterfaceFuture, ConsoleInterfacePort,
        ConsoleInterfaceTargetError,
    },
};

pub(crate) enum ApplicationRuntimeDebugVariablesInput {
    Snapshot {
        application_id: Uuid,
    },
    Upsert {
        application_id: Uuid,
        body: UpsertDebugVariableCacheEntryBody,
    },
    Delete {
        application_id: Uuid,
        body: DeleteDebugVariableCacheEntriesBody,
    },
}

impl InterfaceContract for ApplicationRuntimeDebugVariablesInput {
    const CONTRACT_ID: &'static str = "console-application-runtime-debug-variables-input";
    const CONTRACT_VERSION: &'static str = "1";
}

#[expect(
    clippy::large_enum_variant,
    reason = "the typed debug output is projected immediately into the console response"
)]
pub(crate) enum ApplicationRuntimeDebugVariablesOutput {
    Snapshot(DebugVariableSnapshotResponse),
    Updated,
}

impl InterfaceContract for ApplicationRuntimeDebugVariablesOutput {
    const CONTRACT_ID: &'static str = "console-application-runtime-debug-variables-output";
    const CONTRACT_VERSION: &'static str = "1";
}

struct ApplicationRuntimeDebugVariablesAdapter {
    store: MainDurableStore,
}

impl ApplicationRuntimeDebugVariablesAdapter {
    async fn visible_application(
        &self,
        actor: &domain::ActorContext,
        application_id: Uuid,
    ) -> Result<(), ApiError> {
        ApplicationService::new(self.store.for_actor(actor.clone()))
            .get_application(actor.user_id, application_id)
            .await?;
        Ok(())
    }

    async fn editor_state(
        &self,
        actor: &domain::ActorContext,
        application_id: Uuid,
    ) -> Result<domain::FlowEditorState, ApiError> {
        Ok(FlowService::new(self.store.for_actor(actor.clone()))
            .get_or_create_editor_state(actor.user_id, application_id)
            .await?)
    }

    async fn execute_inner(
        &self,
        principal: &UserPrincipal,
        input: ApplicationRuntimeDebugVariablesInput,
    ) -> Result<ApplicationRuntimeDebugVariablesOutput, ApiError> {
        let actor = principal.actor();
        match input {
            ApplicationRuntimeDebugVariablesInput::Snapshot { application_id } => {
                self.visible_application(actor, application_id).await?;
                let editor_state = self.editor_state(actor, application_id).await?;
                Ok(ApplicationRuntimeDebugVariablesOutput::Snapshot(
                    build_debug_variable_snapshot(
                        &self.store,
                        application_id,
                        actor.current_workspace_id,
                        actor.user_id,
                        &editor_state,
                    )
                    .await?,
                ))
            }
            ApplicationRuntimeDebugVariablesInput::Upsert {
                application_id,
                body,
            } => {
                self.visible_application(actor, application_id).await?;
                let editor_state = self.editor_state(actor, application_id).await?;
                let node_id = body.node_id.trim().to_string();
                let variable_key = body.variable_key.trim().to_string();
                if node_id.is_empty() || variable_key.is_empty() {
                    return Err(ControlPlaneError::InvalidInput("debug_variable_cache_key").into());
                }
                <_ as OrchestrationRuntimeRepository>::upsert_debug_variable_cache_entry(
                    &self.store,
                    &UpsertDebugVariableCacheEntryInput {
                        workspace_id: actor.current_workspace_id,
                        application_id,
                        draft_id: editor_state.draft.id,
                        actor_user_id: actor.user_id,
                        node_id,
                        variable_key,
                        value: body.value,
                    },
                )
                .await?;
                Ok(ApplicationRuntimeDebugVariablesOutput::Updated)
            }
            ApplicationRuntimeDebugVariablesInput::Delete {
                application_id,
                body,
            } => {
                self.visible_application(actor, application_id).await?;
                let editor_state = self.editor_state(actor, application_id).await?;
                let keys = body.keys.map(|keys| {
                    keys.into_iter()
                        .filter_map(|key| {
                            let node_id = key.node_id.trim().to_string();
                            let variable_key = key.variable_key.trim().to_string();
                            (!node_id.is_empty() && !variable_key.is_empty()).then_some(
                                DebugVariableCacheKey {
                                    node_id,
                                    variable_key,
                                },
                            )
                        })
                        .collect()
                });
                <_ as OrchestrationRuntimeRepository>::delete_debug_variable_cache_entries(
                    &self.store,
                    &DeleteDebugVariableCacheEntriesInput {
                        application_id,
                        draft_id: editor_state.draft.id,
                        actor_user_id: actor.user_id,
                        keys,
                    },
                )
                .await?;
                Ok(ApplicationRuntimeDebugVariablesOutput::Updated)
            }
        }
    }
}

impl
    ConsoleInterfacePort<
        ApplicationRuntimeDebugVariablesInput,
        ApplicationRuntimeDebugVariablesOutput,
    > for ApplicationRuntimeDebugVariablesAdapter
{
    fn execute<'a>(
        &'a self,
        principal: &'a UserPrincipal,
        input: ApplicationRuntimeDebugVariablesInput,
    ) -> ConsoleInterfaceFuture<'a, ApplicationRuntimeDebugVariablesOutput> {
        Box::pin(async move {
            self.execute_inner(principal, input)
                .await
                .map_err(ConsoleInterfaceTargetError)
        })
    }
}

pub(crate) const DECLARATIONS: &[ConsoleInterfaceDeclaration] = &[
    ConsoleInterfaceDeclaration {
        interface_id: "applications.runtime.debug-variables.snapshot.get",
        binding_id: "http.console.applications.runtime.debug-variables.snapshot.get.v1",
        method: "GET",
        path: "/api/console/applications/:id/orchestration/debug-variable-snapshot",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "applications.runtime.debug-variables.cache.upsert",
        binding_id: "http.console.applications.runtime.debug-variables.cache.upsert.v1",
        method: "PUT",
        path: "/api/console/applications/:id/orchestration/debug-variable-cache",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "applications.runtime.debug-variables.cache.delete",
        binding_id: "http.console.applications.runtime.debug-variables.cache.delete.v1",
        method: "DELETE",
        path: "/api/console/applications/:id/orchestration/debug-variable-cache",
        mutating: true,
    },
];

pub(crate) fn compile_registry(
    store: MainDurableStore,
) -> Result<
    Arc<interface_runtime::CompiledInterfaceRegistry>,
    interface_runtime::RegistryCompilationError,
> {
    console_interface::compile_registry(
        "api-server.console-application-runtime-debug-variables",
        "graph:console-application-runtime-debug-variables-v1",
        DECLARATIONS,
        Arc::new(ApplicationRuntimeDebugVariablesAdapter { store }),
    )
}

#[cfg(test)]
mod tests {
    use interface_runtime::BindingId;

    use super::*;

    #[test]
    fn f09r3_registry_freezes_debug_variable_bindings() {
        struct Unavailable;
        impl
            ConsoleInterfacePort<
                ApplicationRuntimeDebugVariablesInput,
                ApplicationRuntimeDebugVariablesOutput,
            > for Unavailable
        {
            fn execute<'a>(
                &'a self,
                _principal: &'a UserPrincipal,
                _input: ApplicationRuntimeDebugVariablesInput,
            ) -> ConsoleInterfaceFuture<'a, ApplicationRuntimeDebugVariablesOutput> {
                Box::pin(async {
                    Err(ConsoleInterfaceTargetError(
                        anyhow::anyhow!("fixture unavailable").into(),
                    ))
                })
            }
        }

        let registry = console_interface::compile_registry(
            "api-server.console-application-runtime-debug-variables",
            "graph:console-application-runtime-debug-variables-v1",
            DECLARATIONS,
            Arc::new(Unavailable),
        )
        .unwrap();
        for declaration in DECLARATIONS {
            assert!(registry
                .binding(&BindingId::new(declaration.binding_id).unwrap())
                .is_some());
        }
        assert_eq!(registry.bindings().count(), DECLARATIONS.len());
    }
}
