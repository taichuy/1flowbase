use std::sync::Arc;

use control_plane::ports::CacheStore;
use control_plane::{
    application::ApplicationService,
    application_public_api::{
        mapping::{
            ApplicationApiMappingService, GetApplicationApiMappingCommand,
            ReplaceApplicationApiMappingCommand,
        },
        publications::{
            ApplicationPublicationService, LoadActiveApplicationPublicationCommand,
            PublishApplicationCommand, SetApplicationApiEnabledCommand,
            UnpublishApplicationCommand,
        },
    },
};
use interface_runtime::{InterfaceContract, UserPrincipal};
use storage_durable_postgres::MainDurableStore;
use uuid::Uuid;

use super::{
    map_publication_not_found, to_mapping_body, to_mapping_config, to_publication_response,
    ApplicationApiMappingBody, ApplicationApiStatusResponse, ApplicationPublicationResponse,
    PatchApplicationApiStatusBody, PublishApplicationApiBody, PUBLIC_RUNS_PATH,
};
use crate::{
    error_response::ApiError,
    routes::console_interface::{
        self, ConsoleInterfaceDeclaration, ConsoleInterfaceFuture, ConsoleInterfacePort,
        ConsoleInterfaceTargetError,
    },
};

pub(crate) enum ApplicationPublicationInput {
    GetMapping {
        application_id: Uuid,
    },
    ReplaceMapping {
        application_id: Uuid,
        body: ApplicationApiMappingBody,
    },
    GetPublication {
        application_id: Uuid,
    },
    Publish {
        application_id: Uuid,
        body: PublishApplicationApiBody,
    },
    Unpublish {
        application_id: Uuid,
    },
    SetEnabled {
        application_id: Uuid,
        body: PatchApplicationApiStatusBody,
    },
}

impl InterfaceContract for ApplicationPublicationInput {
    const CONTRACT_ID: &'static str = "console-application-publication-input";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) enum ApplicationPublicationOutput {
    Mapping(ApplicationApiMappingBody),
    Publication(ApplicationPublicationResponse),
    Status(ApplicationApiStatusResponse),
    NoContent,
}

impl ApplicationPublicationOutput {
    pub(super) fn into_mapping(self) -> Result<ApplicationApiMappingBody, ApiError> {
        match self {
            Self::Mapping(value) => Ok(value),
            _ => Err(output_error()),
        }
    }

    pub(super) fn into_publication(self) -> Result<ApplicationPublicationResponse, ApiError> {
        match self {
            Self::Publication(value) => Ok(value),
            _ => Err(output_error()),
        }
    }

    pub(super) fn into_status(self) -> Result<ApplicationApiStatusResponse, ApiError> {
        match self {
            Self::Status(value) => Ok(value),
            _ => Err(output_error()),
        }
    }
}

fn output_error() -> ApiError {
    control_plane::errors::ControlPlaneError::InvalidInput("application_publication_output").into()
}

impl InterfaceContract for ApplicationPublicationOutput {
    const CONTRACT_ID: &'static str = "console-application-publication-output";
    const CONTRACT_VERSION: &'static str = "1";
}

struct ApplicationPublicationAdapter {
    store: MainDurableStore,
    cache_store: Arc<dyn CacheStore>,
}

pub(crate) fn port(
    store: MainDurableStore,
    cache_store: Arc<dyn CacheStore>,
) -> Arc<dyn ConsoleInterfacePort<ApplicationPublicationInput, ApplicationPublicationOutput>> {
    Arc::new(ApplicationPublicationAdapter { store, cache_store })
}

impl ConsoleInterfacePort<ApplicationPublicationInput, ApplicationPublicationOutput>
    for ApplicationPublicationAdapter
{
    fn execute<'a>(
        &'a self,
        principal: &'a UserPrincipal,
        input: ApplicationPublicationInput,
    ) -> ConsoleInterfaceFuture<'a, ApplicationPublicationOutput> {
        Box::pin(async move {
            let result: Result<ApplicationPublicationOutput, ApiError> = async {
                let actor = principal.actor();
                let actor_store = self.store.for_actor(actor.clone());
                let output = match input {
                    ApplicationPublicationInput::GetMapping { application_id } => {
                        let draft = ApplicationApiMappingService::new(actor_store)
                            .get_mapping_draft(GetApplicationApiMappingCommand {
                                actor_user_id: actor.user_id,
                                application_id,
                            })
                            .await?;
                        ApplicationPublicationOutput::Mapping(to_mapping_body(draft.mapping))
                    }
                    ApplicationPublicationInput::ReplaceMapping {
                        application_id,
                        body,
                    } => {
                        let draft = ApplicationApiMappingService::new(actor_store)
                            .replace_mapping_draft(ReplaceApplicationApiMappingCommand {
                                actor_user_id: actor.user_id,
                                application_id,
                                mapping: to_mapping_config(body),
                            })
                            .await?;
                        ApplicationPublicationOutput::Mapping(to_mapping_body(draft.mapping))
                    }
                    ApplicationPublicationInput::GetPublication { application_id } => {
                        ApplicationService::new(actor_store.clone())
                            .get_application(actor.user_id, application_id)
                            .await?;
                        let publication = ApplicationPublicationService::new(actor_store)
                            .load_active_publication(LoadActiveApplicationPublicationCommand {
                                application_id,
                            })
                            .await
                            .map_err(map_publication_not_found)?;
                        ApplicationPublicationOutput::Publication(to_publication_response(
                            publication,
                        ))
                    }
                    ApplicationPublicationInput::Publish {
                        application_id,
                        body,
                    } => {
                        let publication = ApplicationPublicationService::new(actor_store)
                            .with_model_routing_cache_store(Arc::clone(&self.cache_store))
                            .publish_active_version(PublishApplicationCommand {
                                actor_user_id: actor.user_id,
                                application_id,
                                mapping: to_mapping_config(body.mapping),
                                api_enabled: body.api_enabled,
                            })
                            .await?;
                        ApplicationPublicationOutput::Publication(to_publication_response(
                            publication,
                        ))
                    }
                    ApplicationPublicationInput::Unpublish { application_id } => {
                        ApplicationPublicationService::new(actor_store)
                            .unpublish(UnpublishApplicationCommand {
                                actor_user_id: actor.user_id,
                                application_id,
                            })
                            .await
                            .map_err(map_publication_not_found)?;
                        ApplicationPublicationOutput::NoContent
                    }
                    ApplicationPublicationInput::SetEnabled {
                        application_id,
                        body,
                    } => {
                        ApplicationPublicationService::new(actor_store)
                            .set_api_enabled(SetApplicationApiEnabledCommand {
                                actor_user_id: actor.user_id,
                                application_id,
                                api_enabled: body.api_enabled,
                            })
                            .await?;
                        ApplicationPublicationOutput::Status(ApplicationApiStatusResponse {
                            application_id,
                            api_enabled: body.api_enabled,
                            public_url: PUBLIC_RUNS_PATH.to_string(),
                        })
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
        interface_id: "applications.api-mapping.get",
        binding_id: "http.console.applications.api-mapping.get.v1",
        method: "GET",
        path: "/api/console/applications/:application_id/api-mapping",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "applications.api-mapping.replace",
        binding_id: "http.console.applications.api-mapping.replace.v1",
        method: "PUT",
        path: "/api/console/applications/:application_id/api-mapping",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "applications.api-publication.get",
        binding_id: "http.console.applications.api-publication.get.v1",
        method: "GET",
        path: "/api/console/applications/:application_id/api-publication",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "applications.api-publication.publish",
        binding_id: "http.console.applications.api-publication.publish.v1",
        method: "POST",
        path: "/api/console/applications/:application_id/api-publications",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "applications.api-publication.unpublish",
        binding_id: "http.console.applications.api-publication.unpublish.v1",
        method: "DELETE",
        path: "/api/console/applications/:application_id/api-publication",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "applications.api-status.update",
        binding_id: "http.console.applications.api-status.update.v1",
        method: "PATCH",
        path: "/api/console/applications/:application_id/api-status",
        mutating: true,
    },
];

pub(crate) fn compile_registry(
    port: Arc<dyn ConsoleInterfacePort<ApplicationPublicationInput, ApplicationPublicationOutput>>,
) -> Result<
    Arc<interface_runtime::CompiledInterfaceRegistry>,
    interface_runtime::RegistryCompilationError,
> {
    console_interface::compile_registry(
        "api-server.console-application-publication",
        "api-server.console-application-publication.graph.v1",
        DECLARATIONS,
        port,
    )
}
