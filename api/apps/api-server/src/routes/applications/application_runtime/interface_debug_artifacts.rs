use std::{collections::HashSet, sync::Arc};

use control_plane::{
    application::ApplicationService, errors::ControlPlaneError,
    ports::OrchestrationRuntimeRepository,
};
use interface_runtime::{InterfaceContract, UserPrincipal};
use storage_durable_postgres::MainDurableStore;
use uuid::Uuid;

use super::{
    load_runtime_debug_artifact_content, load_runtime_debug_artifact_json_value_with_dependencies,
    offload_application_run_detail_artifacts_with_dependencies, to_application_run_detail_response,
    to_context_snapshot_response, ApplicationRunDetailResponse, ResolveRuntimeDebugArtifactsBody,
    ResolveRuntimeDebugArtifactsResponse, RuntimeDebugArtifactContent,
    RuntimeDebugArtifactReadDependencies, RuntimeDebugArtifactValueResponse,
    RUNTIME_DEBUG_ARTIFACT_RESOLVE_MAX_REFS,
};
use crate::{
    error_response::ApiError,
    routes::console_interface::{
        self, ConsoleInterfaceDeclaration, ConsoleInterfaceFuture, ConsoleInterfacePort,
        ConsoleInterfaceTargetError,
    },
};

pub(crate) enum ApplicationRuntimeDebugArtifactsInput {
    Get {
        application_id: Uuid,
        artifact_id: Uuid,
    },
    Resolve {
        application_id: Uuid,
        body: ResolveRuntimeDebugArtifactsBody,
    },
    Snapshot {
        application_id: Uuid,
        run_id: Uuid,
    },
}

impl InterfaceContract for ApplicationRuntimeDebugArtifactsInput {
    const CONTRACT_ID: &'static str = "console-application-runtime-debug-artifacts-input";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) enum ApplicationRuntimeDebugArtifactsOutput {
    Content(RuntimeDebugArtifactContent),
    Resolved(ResolveRuntimeDebugArtifactsResponse),
    Snapshot(ApplicationRunDetailResponse),
}

impl InterfaceContract for ApplicationRuntimeDebugArtifactsOutput {
    const CONTRACT_ID: &'static str = "console-application-runtime-debug-artifacts-output";
    const CONTRACT_VERSION: &'static str = "1";
}

struct ApplicationRuntimeDebugArtifactsAdapter {
    store: MainDurableStore,
    reads: RuntimeDebugArtifactReadDependencies,
    file_storage_registry: Arc<storage_object::FileStorageDriverRegistry>,
}

impl ApplicationRuntimeDebugArtifactsAdapter {
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

    async fn execute_inner(
        &self,
        principal: &UserPrincipal,
        input: ApplicationRuntimeDebugArtifactsInput,
    ) -> Result<ApplicationRuntimeDebugArtifactsOutput, ApiError> {
        let actor = principal.actor();
        match input {
            ApplicationRuntimeDebugArtifactsInput::Get {
                application_id,
                artifact_id,
            } => {
                self.visible_application(actor, application_id).await?;
                Ok(ApplicationRuntimeDebugArtifactsOutput::Content(
                    load_runtime_debug_artifact_content(
                        &self.reads,
                        actor.current_workspace_id,
                        application_id,
                        artifact_id,
                    )
                    .await?,
                ))
            }
            ApplicationRuntimeDebugArtifactsInput::Resolve {
                application_id,
                body,
            } => {
                self.visible_application(actor, application_id).await?;
                if body.artifact_refs.len() > RUNTIME_DEBUG_ARTIFACT_RESOLVE_MAX_REFS {
                    return Err(ControlPlaneError::InvalidInput("artifact_refs").into());
                }
                let mut seen = HashSet::new();
                let mut artifacts = Vec::new();
                for artifact_id in body.artifact_refs {
                    if !seen.insert(artifact_id) {
                        continue;
                    }
                    artifacts.push(RuntimeDebugArtifactValueResponse {
                        artifact_ref: artifact_id.to_string(),
                        content_type: "application/json".to_string(),
                        value: load_runtime_debug_artifact_json_value_with_dependencies(
                            &self.reads,
                            actor.current_workspace_id,
                            application_id,
                            artifact_id,
                        )
                        .await?,
                    });
                }
                Ok(ApplicationRuntimeDebugArtifactsOutput::Resolved(
                    ResolveRuntimeDebugArtifactsResponse { artifacts },
                ))
            }
            ApplicationRuntimeDebugArtifactsInput::Snapshot {
                application_id,
                run_id,
            } => {
                let application = ApplicationService::new(self.store.for_actor(actor.clone()))
                    .get_application(actor.user_id, application_id)
                    .await?;
                let detail = <_ as OrchestrationRuntimeRepository>::get_application_run_detail(
                    &self.store,
                    application_id,
                    run_id,
                )
                .await?
                .ok_or(ControlPlaneError::NotFound("flow_run"))?;
                if detail.flow_run.created_by != actor.user_id {
                    return Err(ControlPlaneError::NotFound("flow_run").into());
                }
                let runtime_events = <_ as OrchestrationRuntimeRepository>::list_runtime_events(
                    &self.store,
                    run_id,
                    0,
                )
                .await?;
                let detail = offload_application_run_detail_artifacts_with_dependencies(
                    self.store.clone(),
                    self.file_storage_registry.clone(),
                    actor.current_workspace_id,
                    application_id,
                    detail,
                )
                .await?;
                let mut response = to_application_run_detail_response(&application, detail);
                response.context_snapshot = to_context_snapshot_response(&runtime_events);
                Ok(ApplicationRuntimeDebugArtifactsOutput::Snapshot(response))
            }
        }
    }
}

impl
    ConsoleInterfacePort<
        ApplicationRuntimeDebugArtifactsInput,
        ApplicationRuntimeDebugArtifactsOutput,
    > for ApplicationRuntimeDebugArtifactsAdapter
{
    fn execute<'a>(
        &'a self,
        principal: &'a UserPrincipal,
        input: ApplicationRuntimeDebugArtifactsInput,
    ) -> ConsoleInterfaceFuture<'a, ApplicationRuntimeDebugArtifactsOutput> {
        Box::pin(async move {
            self.execute_inner(principal, input)
                .await
                .map_err(ConsoleInterfaceTargetError)
        })
    }
}

pub(crate) const DECLARATIONS: &[ConsoleInterfaceDeclaration] = &[
    ConsoleInterfaceDeclaration {
        interface_id: "applications.runtime.debug-artifact.get",
        binding_id: "http.console.applications.runtime.debug-artifact.get.v1",
        method: "GET",
        path: "/api/console/applications/:id/orchestration/debug-artifacts/:artifact_id",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "applications.runtime.debug-artifacts.resolve",
        binding_id: "http.console.applications.runtime.debug-artifacts.resolve.v1",
        method: "POST",
        path: "/api/console/applications/:id/orchestration/debug-artifacts/resolve",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "applications.runtime.debug-snapshot.get",
        binding_id: "http.console.applications.runtime.debug-snapshot.get.v1",
        method: "GET",
        path: "/api/console/applications/:id/orchestration/runs/:run_id/debug-snapshot",
        mutating: false,
    },
];

pub(crate) fn compile_registry(
    store: MainDurableStore,
    file_storage_registry: Arc<storage_object::FileStorageDriverRegistry>,
) -> Result<
    Arc<interface_runtime::CompiledInterfaceRegistry>,
    interface_runtime::RegistryCompilationError,
> {
    let reads =
        RuntimeDebugArtifactReadDependencies::new(store.clone(), file_storage_registry.clone());
    console_interface::compile_registry(
        "api-server.console-application-runtime-debug-artifacts",
        "graph:console-application-runtime-debug-artifacts-v1",
        DECLARATIONS,
        Arc::new(ApplicationRuntimeDebugArtifactsAdapter {
            store,
            reads,
            file_storage_registry,
        }),
    )
}

#[cfg(test)]
mod tests {
    use interface_runtime::BindingId;

    use super::*;

    #[test]
    fn f09r3_registry_freezes_debug_artifact_bindings() {
        struct Unavailable;
        impl
            ConsoleInterfacePort<
                ApplicationRuntimeDebugArtifactsInput,
                ApplicationRuntimeDebugArtifactsOutput,
            > for Unavailable
        {
            fn execute<'a>(
                &'a self,
                _principal: &'a UserPrincipal,
                _input: ApplicationRuntimeDebugArtifactsInput,
            ) -> ConsoleInterfaceFuture<'a, ApplicationRuntimeDebugArtifactsOutput> {
                Box::pin(async {
                    Err(ConsoleInterfaceTargetError(
                        anyhow::anyhow!("fixture unavailable").into(),
                    ))
                })
            }
        }

        let registry = console_interface::compile_registry(
            "api-server.console-application-runtime-debug-artifacts",
            "graph:console-application-runtime-debug-artifacts-v1",
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
