use std::sync::Arc;

use control_plane::flow::{FlowService, SaveFlowDraftCommand, UpdateFlowVersionMetadataCommand};
use interface_runtime::{InterfaceContract, UserPrincipal};
use storage_durable_postgres::MainDurableStore;
use uuid::Uuid;

use super::{
    parse_change_kind, to_response_with, OrchestrationStateResponse, SaveDraftBody,
    UpdateVersionBody,
};
use crate::{
    error_response::ApiError,
    routes::console_interface::{
        self, ConsoleInterfaceDeclaration, ConsoleInterfaceFuture, ConsoleInterfacePort,
        ConsoleInterfaceTargetError, ConsoleLocaleHints,
    },
};

pub(crate) enum ApplicationOrchestrationInput {
    Get {
        application_id: Uuid,
        locale: ConsoleLocaleHints,
    },
    SaveDraft {
        application_id: Uuid,
        body: SaveDraftBody,
        locale: ConsoleLocaleHints,
    },
    RestoreVersion {
        application_id: Uuid,
        version_id: Uuid,
        locale: ConsoleLocaleHints,
    },
    UpdateVersion {
        application_id: Uuid,
        version_id: Uuid,
        body: UpdateVersionBody,
        locale: ConsoleLocaleHints,
    },
}

impl InterfaceContract for ApplicationOrchestrationInput {
    const CONTRACT_ID: &'static str = "console-application-orchestration-input";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) enum ApplicationOrchestrationOutput {
    State(OrchestrationStateResponse),
}

impl ApplicationOrchestrationOutput {
    pub(super) fn into_state(self) -> Result<OrchestrationStateResponse, ApiError> {
        match self {
            Self::State(state) => Ok(state),
        }
    }
}

impl InterfaceContract for ApplicationOrchestrationOutput {
    const CONTRACT_ID: &'static str = "console-application-orchestration-output";
    const CONTRACT_VERSION: &'static str = "1";
}

struct ApplicationOrchestrationAdapter {
    store: MainDurableStore,
    bootstrap_workspace_id: Uuid,
}

pub(crate) fn port(
    store: MainDurableStore,
    bootstrap_workspace_id: Uuid,
) -> Arc<dyn ConsoleInterfacePort<ApplicationOrchestrationInput, ApplicationOrchestrationOutput>> {
    Arc::new(ApplicationOrchestrationAdapter {
        store,
        bootstrap_workspace_id,
    })
}

impl ApplicationOrchestrationAdapter {
    async fn preferred_locale(
        &self,
        principal: &UserPrincipal,
    ) -> Result<Option<String>, ApiError> {
        Ok(self
            .store
            .find_user_by_id(principal.actor().user_id)
            .await?
            .ok_or(control_plane::errors::ControlPlaneError::NotAuthenticated)?
            .preferred_locale)
    }

    async fn execute_inner(
        &self,
        principal: &UserPrincipal,
        input: ApplicationOrchestrationInput,
    ) -> Result<ApplicationOrchestrationOutput, ApiError> {
        let actor = principal.actor();
        let user_id = actor.user_id;
        let service = FlowService::new(self.store.for_actor(actor.clone()));
        let (state, locale) = match input {
            ApplicationOrchestrationInput::Get {
                application_id,
                locale,
            } => (
                service
                    .get_or_create_editor_state(user_id, application_id)
                    .await?,
                locale,
            ),
            ApplicationOrchestrationInput::SaveDraft {
                application_id,
                body,
                locale,
            } => (
                service
                    .save_draft(SaveFlowDraftCommand {
                        actor_user_id: user_id,
                        application_id,
                        document: body.document,
                        change_kind: parse_change_kind(&body.change_kind)?,
                        summary: body.summary,
                    })
                    .await?,
                locale,
            ),
            ApplicationOrchestrationInput::RestoreVersion {
                application_id,
                version_id,
                locale,
            } => (
                service
                    .restore_version(user_id, application_id, version_id)
                    .await?,
                locale,
            ),
            ApplicationOrchestrationInput::UpdateVersion {
                application_id,
                version_id,
                body,
                locale,
            } => (
                service
                    .update_version_metadata(UpdateFlowVersionMetadataCommand {
                        actor_user_id: user_id,
                        application_id,
                        version_id,
                        summary: body.summary,
                        summary_is_custom: body.summary_is_custom,
                        is_user_protected: body.is_user_protected,
                    })
                    .await?,
                locale,
            ),
        };
        let locale = locale.resolve(self.preferred_locale(principal).await?);
        Ok(ApplicationOrchestrationOutput::State(
            to_response_with(&self.store, self.bootstrap_workspace_id, &locale, state).await?,
        ))
    }
}

impl ConsoleInterfacePort<ApplicationOrchestrationInput, ApplicationOrchestrationOutput>
    for ApplicationOrchestrationAdapter
{
    fn execute<'a>(
        &'a self,
        principal: &'a UserPrincipal,
        input: ApplicationOrchestrationInput,
    ) -> ConsoleInterfaceFuture<'a, ApplicationOrchestrationOutput> {
        Box::pin(async move {
            self.execute_inner(principal, input)
                .await
                .map_err(ConsoleInterfaceTargetError)
        })
    }
}

pub(crate) const DECLARATIONS: &[ConsoleInterfaceDeclaration] = &[
    ConsoleInterfaceDeclaration {
        interface_id: "applications.orchestration.get",
        binding_id: "http.console.applications.orchestration.get.v1",
        method: "GET",
        path: "/api/console/applications/:id/orchestration",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "applications.orchestration.draft.save",
        binding_id: "http.console.applications.orchestration.draft.save.v1",
        method: "PUT",
        path: "/api/console/applications/:id/orchestration/draft",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "applications.orchestration.version.restore",
        binding_id: "http.console.applications.orchestration.version.restore.v1",
        method: "POST",
        path: "/api/console/applications/:id/orchestration/versions/:version_id/restore",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "applications.orchestration.version.update",
        binding_id: "http.console.applications.orchestration.version.update.v1",
        method: "PATCH",
        path: "/api/console/applications/:id/orchestration/versions/:version_id",
        mutating: true,
    },
];

pub(crate) fn compile_registry(
    port: Arc<
        dyn ConsoleInterfacePort<ApplicationOrchestrationInput, ApplicationOrchestrationOutput>,
    >,
) -> Result<
    Arc<interface_runtime::CompiledInterfaceRegistry>,
    interface_runtime::RegistryCompilationError,
> {
    console_interface::compile_registry(
        "api-server.console-application-orchestration",
        "api-server.console-application-orchestration.graph.v1",
        DECLARATIONS,
        port,
    )
}
