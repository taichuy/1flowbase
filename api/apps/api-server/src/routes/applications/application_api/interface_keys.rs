use std::sync::Arc;

use control_plane::application_public_api::api_keys::{
    ApplicationApiKeyService, CreateApplicationApiKeyCommand, ListApplicationApiKeysCommand,
    RevokeApplicationApiKeyCommand,
};
use interface_runtime::{InterfaceContract, UserPrincipal};
use storage_durable_postgres::MainDurableStore;
use uuid::Uuid;

use super::{
    map_application_api_key_not_found, parse_expires_at, to_api_key_response,
    to_created_api_key_response, ApplicationApiKeyResponse, CreateApplicationApiKeyBody,
    CreatedApplicationApiKeyResponse,
};
use crate::{
    error_response::ApiError,
    routes::console_interface::{
        self, ConsoleInterfaceDeclaration, ConsoleInterfaceFuture, ConsoleInterfacePort,
        ConsoleInterfaceTargetError,
    },
};

pub(crate) enum ApplicationApiKeyInput {
    List {
        application_id: Uuid,
    },
    Create {
        application_id: Uuid,
        body: CreateApplicationApiKeyBody,
    },
    Revoke {
        application_id: Uuid,
        key_id: Uuid,
    },
}

impl InterfaceContract for ApplicationApiKeyInput {
    const CONTRACT_ID: &'static str = "console-application-api-key-input";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) enum ApplicationApiKeyOutput {
    List(Vec<ApplicationApiKeyResponse>),
    Created(CreatedApplicationApiKeyResponse),
    NoContent,
}

impl ApplicationApiKeyOutput {
    pub(super) fn into_list(self) -> Result<Vec<ApplicationApiKeyResponse>, ApiError> {
        match self {
            Self::List(value) => Ok(value),
            _ => Err(control_plane::errors::ControlPlaneError::InvalidInput(
                "application_api_key_output",
            )
            .into()),
        }
    }

    pub(super) fn into_created(self) -> Result<CreatedApplicationApiKeyResponse, ApiError> {
        match self {
            Self::Created(value) => Ok(value),
            _ => Err(control_plane::errors::ControlPlaneError::InvalidInput(
                "application_api_key_output",
            )
            .into()),
        }
    }
}

impl InterfaceContract for ApplicationApiKeyOutput {
    const CONTRACT_ID: &'static str = "console-application-api-key-output";
    const CONTRACT_VERSION: &'static str = "1";
}

struct ApplicationApiKeyAdapter {
    store: MainDurableStore,
}

pub(crate) fn port(
    store: MainDurableStore,
) -> Arc<dyn ConsoleInterfacePort<ApplicationApiKeyInput, ApplicationApiKeyOutput>> {
    Arc::new(ApplicationApiKeyAdapter { store })
}

impl ConsoleInterfacePort<ApplicationApiKeyInput, ApplicationApiKeyOutput>
    for ApplicationApiKeyAdapter
{
    fn execute<'a>(
        &'a self,
        principal: &'a UserPrincipal,
        input: ApplicationApiKeyInput,
    ) -> ConsoleInterfaceFuture<'a, ApplicationApiKeyOutput> {
        Box::pin(async move {
            let result: Result<ApplicationApiKeyOutput, ApiError> = async {
                let actor = principal.actor();
                let service = ApplicationApiKeyService::new(self.store.for_actor(actor.clone()));
                let output = match input {
                    ApplicationApiKeyInput::List { application_id } => {
                        let values = service
                            .list_api_keys(ListApplicationApiKeysCommand {
                                actor_user_id: actor.user_id,
                                application_id,
                            })
                            .await?;
                        ApplicationApiKeyOutput::List(
                            values.into_iter().map(to_api_key_response).collect(),
                        )
                    }
                    ApplicationApiKeyInput::Create {
                        application_id,
                        body,
                    } => {
                        let result = service
                            .create_api_key(CreateApplicationApiKeyCommand {
                                actor_user_id: actor.user_id,
                                application_id,
                                name: body.name,
                                expires_at: parse_expires_at(body.expires_at)?,
                            })
                            .await?;
                        ApplicationApiKeyOutput::Created(to_created_api_key_response(
                            result.api_key,
                            result.token,
                        ))
                    }
                    ApplicationApiKeyInput::Revoke {
                        application_id,
                        key_id,
                    } => {
                        service
                            .revoke_api_key(RevokeApplicationApiKeyCommand {
                                actor_user_id: actor.user_id,
                                application_id,
                                api_key_id: key_id,
                            })
                            .await
                            .map_err(map_application_api_key_not_found)?;
                        ApplicationApiKeyOutput::NoContent
                    }
                };
                Ok(output)
            }
            .await;
            result.map_err(ConsoleInterfaceTargetError)
        })
    }
}

pub(crate) const DECLARATIONS: &[ConsoleInterfaceDeclaration] = &[
    ConsoleInterfaceDeclaration {
        interface_id: "applications.api-keys.list",
        binding_id: "http.console.applications.api-keys.list.v1",
        method: "GET",
        path: "/api/console/applications/:application_id/api-keys",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "applications.api-keys.create",
        binding_id: "http.console.applications.api-keys.create.v1",
        method: "POST",
        path: "/api/console/applications/:application_id/api-keys",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "applications.api-keys.revoke",
        binding_id: "http.console.applications.api-keys.revoke.v1",
        method: "DELETE",
        path: "/api/console/applications/:application_id/api-keys/:key_id",
        mutating: true,
    },
];

pub(crate) fn compile_registry(
    port: Arc<dyn ConsoleInterfacePort<ApplicationApiKeyInput, ApplicationApiKeyOutput>>,
) -> Result<
    Arc<interface_runtime::CompiledInterfaceRegistry>,
    interface_runtime::RegistryCompilationError,
> {
    console_interface::compile_registry(
        "api-server.console-application-api-keys",
        "api-server.console-application-api-keys.graph.v1",
        DECLARATIONS,
        port,
    )
}
