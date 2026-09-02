use std::sync::Arc;

use control_plane::{
    application::{
        ApplicationService, CreateApplicationCommand, CreateApplicationTagCommand,
        DeleteApplicationCommand, ReplaceApplicationEnvironmentVariablesCommand,
        UpdateApplicationCommand,
    },
    errors::ControlPlaneError,
    js_dependency::{
        ApplicationJsDependencyService, ReplaceApplicationJsDependencySelectionCommand,
    },
    ports::{ApplicationEnvironmentVariableInput, CreateWorkflowTriggerConfig},
};
use interface_runtime::{InterfaceContract, UserPrincipal};
use storage_durable_postgres::MainDurableStore;
use uuid::Uuid;

use super::{
    ApplicationCatalogResponse, ApplicationDetailResponse, ApplicationEnvironmentVariableResponse,
    ApplicationJsDependencySelectionResponse, ApplicationSummaryResponse,
    ApplicationTagCatalogResponse, ApplicationTypeDto, ApplicationTypeOptionResponse,
    CreateApplicationBody, CreateApplicationTagBody, CreateWorkflowTriggerConfigBody,
    PatchApplicationBody, ReplaceApplicationEnvironmentVariablesBody,
    ReplaceApplicationJsDependencySelectionBody, WorkflowTriggerTypeDto,
    WorkflowTriggerTypeOptionResponse,
};
use crate::routes::application_api::{
    WorkflowExtensionHttpMethodBody, WorkflowExtensionResponseModeBody,
};
use crate::{
    app_state::resolve_request_text_with,
    error_response::ApiError,
    routes::console_interface::{
        self, ConsoleInterfaceDeclaration, ConsoleInterfaceFuture, ConsoleInterfacePort,
        ConsoleInterfaceTargetError, ConsoleLocaleHints,
    },
};

pub(crate) enum ApplicationsInput {
    List,
    Catalog {
        locale: ConsoleLocaleHints,
    },
    Create(CreateApplicationBody),
    CreateTag(CreateApplicationTagBody),
    Get {
        application_id: Uuid,
    },
    Patch {
        application_id: Uuid,
        body: PatchApplicationBody,
    },
    Delete {
        application_id: Uuid,
    },
    ListEnvironmentVariables {
        application_id: Uuid,
    },
    ReplaceEnvironmentVariables {
        application_id: Uuid,
        body: ReplaceApplicationEnvironmentVariablesBody,
    },
    ListJsDependencies {
        application_id: Uuid,
    },
    ReplaceJsDependency {
        application_id: Uuid,
        body: ReplaceApplicationJsDependencySelectionBody,
    },
}

impl InterfaceContract for ApplicationsInput {
    const CONTRACT_ID: &'static str = "console-applications-input";
    const CONTRACT_VERSION: &'static str = "1";
}

#[expect(
    clippy::large_enum_variant,
    reason = "the typed application output is projected immediately into the console response"
)]
pub(crate) enum ApplicationsOutput {
    Applications(Vec<ApplicationSummaryResponse>),
    Catalog(ApplicationCatalogResponse),
    Application(ApplicationDetailResponse),
    Tag(ApplicationTagCatalogResponse),
    EnvironmentVariables(Vec<ApplicationEnvironmentVariableResponse>),
    JsDependencies(Vec<ApplicationJsDependencySelectionResponse>),
    JsDependency(ApplicationJsDependencySelectionResponse),
    NoContent,
}

impl InterfaceContract for ApplicationsOutput {
    const CONTRACT_ID: &'static str = "console-applications-output";
    const CONTRACT_VERSION: &'static str = "1";
}

struct ApplicationsAdapter {
    store: MainDurableStore,
    bootstrap_workspace_id: Uuid,
}

pub(crate) fn applications_port(
    store: MainDurableStore,
    bootstrap_workspace_id: Uuid,
) -> Arc<dyn ConsoleInterfacePort<ApplicationsInput, ApplicationsOutput>> {
    Arc::new(ApplicationsAdapter {
        store,
        bootstrap_workspace_id,
    })
}

impl ApplicationsAdapter {
    async fn catalog(
        &self,
        principal: &UserPrincipal,
        hints: ConsoleLocaleHints,
    ) -> Result<ApplicationCatalogResponse, ApiError> {
        let preferred_locale = self
            .store
            .find_user_by_id(principal.actor().user_id)
            .await?
            .ok_or(ControlPlaneError::NotAuthenticated)?
            .preferred_locale;
        let locale = hints.resolve(preferred_locale);
        let tags = ApplicationService::new(self.store.for_actor(principal.actor().clone()))
            .list_application_tags(principal.actor().user_id)
            .await?;
        Ok(ApplicationCatalogResponse {
            types: application_type_catalog(&self.store, self.bootstrap_workspace_id, &locale)
                .await?,
            workflow_triggers: workflow_trigger_type_catalog(
                &self.store,
                self.bootstrap_workspace_id,
                &locale,
            )
            .await?,
            tags: tags
                .into_iter()
                .map(super::to_application_tag_catalog_entry)
                .collect(),
        })
    }

    async fn execute_inner(
        &self,
        principal: &UserPrincipal,
        input: ApplicationsInput,
    ) -> Result<ApplicationsOutput, ApiError> {
        let actor = principal.actor();
        let service = ApplicationService::new(self.store.for_actor(actor.clone()));
        match input {
            ApplicationsInput::List => Ok(ApplicationsOutput::Applications(
                service
                    .list_applications(actor.user_id)
                    .await?
                    .into_iter()
                    .map(super::to_application_summary)
                    .collect(),
            )),
            ApplicationsInput::Catalog { locale } => Ok(ApplicationsOutput::Catalog(
                self.catalog(principal, locale).await?,
            )),
            ApplicationsInput::Create(body) => {
                let application_type = body.application_type.into_domain();
                let workflow_trigger_type =
                    parse_workflow_trigger_type(application_type, body.workflow_trigger_type)?;
                let created = service
                    .create_application(CreateApplicationCommand {
                        actor_user_id: actor.user_id,
                        application_type,
                        workflow_trigger_type,
                        workflow_trigger_config: parse_create_workflow_trigger_config(
                            workflow_trigger_type,
                            body.workflow_trigger_config,
                        )?,
                        name: body.name,
                        description: body.description,
                        icon: body.icon,
                        icon_type: body.icon_type,
                        icon_background: body.icon_background,
                    })
                    .await?;
                Ok(ApplicationsOutput::Application(
                    super::to_application_detail(created),
                ))
            }
            ApplicationsInput::CreateTag(body) => {
                let created = service
                    .create_application_tag(CreateApplicationTagCommand {
                        actor_user_id: actor.user_id,
                        name: body.name,
                    })
                    .await?;
                Ok(ApplicationsOutput::Tag(
                    super::to_application_tag_catalog_entry(created),
                ))
            }
            ApplicationsInput::Get { application_id } => {
                let application = service
                    .get_application(actor.user_id, application_id)
                    .await?;
                Ok(ApplicationsOutput::Application(
                    super::to_application_detail(application),
                ))
            }
            ApplicationsInput::Patch {
                application_id,
                body,
            } => {
                let tag_ids = body
                    .tag_ids
                    .into_iter()
                    .map(|value| {
                        value
                            .parse::<Uuid>()
                            .map_err(|_| ControlPlaneError::InvalidInput("tag_ids"))
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let updated = service
                    .update_application(UpdateApplicationCommand {
                        actor_user_id: actor.user_id,
                        application_id,
                        name: body.name,
                        description: body.description,
                        tag_ids,
                        icon: body.icon,
                        icon_type: body.icon_type,
                        icon_background: body.icon_background,
                    })
                    .await?;
                Ok(ApplicationsOutput::Application(
                    super::to_application_detail(updated),
                ))
            }
            ApplicationsInput::Delete { application_id } => {
                service
                    .delete_application(DeleteApplicationCommand {
                        actor_user_id: actor.user_id,
                        application_id,
                    })
                    .await?;
                Ok(ApplicationsOutput::NoContent)
            }
            ApplicationsInput::ListEnvironmentVariables { application_id } => {
                Ok(ApplicationsOutput::EnvironmentVariables(
                    service
                        .list_application_environment_variables(actor.user_id, application_id)
                        .await?
                        .into_iter()
                        .map(super::to_application_environment_variable)
                        .collect(),
                ))
            }
            ApplicationsInput::ReplaceEnvironmentVariables {
                application_id,
                body,
            } => {
                let variables = body
                    .variables
                    .into_iter()
                    .map(|variable| ApplicationEnvironmentVariableInput {
                        name: variable.name,
                        value_type: variable.value_type,
                        value: variable.value,
                        description: variable.description,
                    })
                    .collect();
                let replaced = service
                    .replace_application_environment_variables(
                        ReplaceApplicationEnvironmentVariablesCommand {
                            actor_user_id: actor.user_id,
                            application_id,
                            variables,
                        },
                    )
                    .await?;
                Ok(ApplicationsOutput::EnvironmentVariables(
                    replaced
                        .into_iter()
                        .map(super::to_application_environment_variable)
                        .collect(),
                ))
            }
            ApplicationsInput::ListJsDependencies { application_id } => {
                Ok(ApplicationsOutput::JsDependencies(
                    ApplicationJsDependencyService::new(self.store.clone())
                        .list_application_js_dependency_selections(actor.user_id, application_id)
                        .await?
                        .into_iter()
                        .map(super::to_application_js_dependency_selection)
                        .collect(),
                ))
            }
            ApplicationsInput::ReplaceJsDependency {
                application_id,
                body,
            } => {
                let installation_id = body
                    .installation_id
                    .parse::<Uuid>()
                    .map_err(|_| ControlPlaneError::InvalidInput("installation_id"))?;
                let selection = ApplicationJsDependencyService::new(self.store.clone())
                    .replace_application_js_dependency_selection(
                        ReplaceApplicationJsDependencySelectionCommand {
                            actor_user_id: actor.user_id,
                            application_id,
                            installation_id,
                            alias: body.alias,
                            target: body.target,
                        },
                    )
                    .await?;
                Ok(ApplicationsOutput::JsDependency(
                    super::to_application_js_dependency_selection(selection),
                ))
            }
        }
    }
}

impl ConsoleInterfacePort<ApplicationsInput, ApplicationsOutput> for ApplicationsAdapter {
    fn execute<'a>(
        &'a self,
        principal: &'a UserPrincipal,
        input: ApplicationsInput,
    ) -> ConsoleInterfaceFuture<'a, ApplicationsOutput> {
        Box::pin(async move {
            self.execute_inner(principal, input)
                .await
                .map_err(ConsoleInterfaceTargetError)
        })
    }
}

pub(crate) const DECLARATIONS: &[ConsoleInterfaceDeclaration] = &[
    ConsoleInterfaceDeclaration {
        interface_id: "applications.list",
        binding_id: "http.console.applications.list.v1",
        method: "GET",
        path: "/api/console/applications",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "applications.create",
        binding_id: "http.console.applications.create.v1",
        method: "POST",
        path: "/api/console/applications",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "applications.get",
        binding_id: "http.console.applications.get.v1",
        method: "GET",
        path: "/api/console/applications/:id",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "applications.update",
        binding_id: "http.console.applications.update.v1",
        method: "PATCH",
        path: "/api/console/applications/:id",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "applications.delete",
        binding_id: "http.console.applications.delete.v1",
        method: "DELETE",
        path: "/api/console/applications/:id",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "applications.catalog.get",
        binding_id: "http.console.applications.catalog.get.v1",
        method: "GET",
        path: "/api/console/applications/catalog",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "applications.tags.create",
        binding_id: "http.console.applications.tags.create.v1",
        method: "POST",
        path: "/api/console/applications/tags",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "applications.environment-variables.list",
        binding_id: "http.console.applications.environment-variables.list.v1",
        method: "GET",
        path: "/api/console/applications/:id/environment-variables",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "applications.environment-variables.replace",
        binding_id: "http.console.applications.environment-variables.replace.v1",
        method: "PUT",
        path: "/api/console/applications/:id/environment-variables",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "applications.js-dependencies.list",
        binding_id: "http.console.applications.js-dependencies.list.v1",
        method: "GET",
        path: "/api/console/applications/:id/js-dependencies",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "applications.js-dependencies.replace",
        binding_id: "http.console.applications.js-dependencies.replace.v1",
        method: "PUT",
        path: "/api/console/applications/:id/js-dependencies",
        mutating: true,
    },
];

pub(crate) fn compile_registry(
    port: Arc<dyn ConsoleInterfacePort<ApplicationsInput, ApplicationsOutput>>,
) -> Result<
    Arc<interface_runtime::CompiledInterfaceRegistry>,
    interface_runtime::RegistryCompilationError,
> {
    console_interface::compile_registry(
        "api-server.console-applications",
        "graph:console-applications-v1",
        DECLARATIONS,
        port,
    )
}

async fn application_type_catalog(
    store: &MainDurableStore,
    bootstrap_workspace_id: Uuid,
    locale: &domain::CatalogLocale,
) -> Result<Vec<ApplicationTypeOptionResponse>, ApiError> {
    let agent_flow =
        resolve_request_text_with(store, bootstrap_workspace_id, locale, "Agent Flow").await?;
    let workflow =
        resolve_request_text_with(store, bootstrap_workspace_id, locale, "Workflow").await?;
    Ok(vec![
        ApplicationTypeOptionResponse {
            value: ApplicationTypeDto::AgentFlow,
            label: agent_flow,
        },
        ApplicationTypeOptionResponse {
            value: ApplicationTypeDto::Workflow,
            label: workflow,
        },
    ])
}

async fn workflow_trigger_type_catalog(
    store: &MainDurableStore,
    bootstrap_workspace_id: Uuid,
    locale: &domain::CatalogLocale,
) -> Result<Vec<WorkflowTriggerTypeOptionResponse>, ApiError> {
    let extension =
        resolve_request_text_with(store, bootstrap_workspace_id, locale, "Extension").await?;
    let schedule =
        resolve_request_text_with(store, bootstrap_workspace_id, locale, "Schedule").await?;
    Ok(vec![
        WorkflowTriggerTypeOptionResponse {
            value: WorkflowTriggerTypeDto::Extension,
            label: extension,
        },
        WorkflowTriggerTypeOptionResponse {
            value: WorkflowTriggerTypeDto::Schedule,
            label: schedule,
        },
    ])
}

fn parse_create_workflow_trigger_config(
    trigger_type: Option<domain::WorkflowTriggerType>,
    config: Option<CreateWorkflowTriggerConfigBody>,
) -> Result<Option<CreateWorkflowTriggerConfig>, ApiError> {
    match (trigger_type, config) {
        (None, None) => Ok(None),
        (None, Some(_)) => Err(ControlPlaneError::InvalidInput("workflow_trigger_config").into()),
        (Some(domain::WorkflowTriggerType::Schedule), Some(config)) => {
            let cron = config
                .cron
                .filter(|value| !value.trim().is_empty())
                .ok_or(ControlPlaneError::InvalidInput("cron"))?;
            let timezone = config
                .timezone
                .filter(|value| !value.trim().is_empty())
                .ok_or(ControlPlaneError::InvalidInput("timezone"))?;
            Ok(Some(CreateWorkflowTriggerConfig::Schedule {
                cron,
                timezone,
                input_payload: config
                    .input_payload
                    .unwrap_or_else(|| serde_json::json!({})),
            }))
        }
        (Some(domain::WorkflowTriggerType::Extension), Some(config)) => {
            let subpath = config
                .subpath
                .filter(|value| !value.trim().is_empty())
                .ok_or(ControlPlaneError::InvalidInput("subpath"))?;
            let http_method = match config
                .http_method
                .unwrap_or(WorkflowExtensionHttpMethodBody::Post)
            {
                WorkflowExtensionHttpMethodBody::Get => "GET",
                WorkflowExtensionHttpMethodBody::Post => "POST",
                WorkflowExtensionHttpMethodBody::Put => "PUT",
                WorkflowExtensionHttpMethodBody::Patch => "PATCH",
                WorkflowExtensionHttpMethodBody::Delete => "DELETE",
                WorkflowExtensionHttpMethodBody::Head => "HEAD",
                WorkflowExtensionHttpMethodBody::Options => "OPTIONS",
            }
            .to_string();
            let response_mode = match config
                .response_mode
                .unwrap_or(WorkflowExtensionResponseModeBody::Sync)
            {
                WorkflowExtensionResponseModeBody::Sync => "sync",
                WorkflowExtensionResponseModeBody::Async => "async",
            }
            .to_string();
            Ok(Some(CreateWorkflowTriggerConfig::Extension {
                subpath,
                http_method,
                response_mode,
            }))
        }
        (Some(_), None) => Ok(None),
    }
}

fn parse_workflow_trigger_type(
    application_type: domain::ApplicationType,
    value: Option<WorkflowTriggerTypeDto>,
) -> Result<Option<domain::WorkflowTriggerType>, ApiError> {
    match application_type {
        domain::ApplicationType::AgentFlow if value.is_none() => Ok(None),
        domain::ApplicationType::AgentFlow => {
            Err(ControlPlaneError::InvalidInput("workflow_trigger_type").into())
        }
        domain::ApplicationType::Workflow => Ok(Some(
            value
                .unwrap_or(WorkflowTriggerTypeDto::Extension)
                .into_domain(),
        )),
    }
}

#[cfg(test)]
struct UnavailableApplicationsPort;

#[cfg(test)]
impl ConsoleInterfacePort<ApplicationsInput, ApplicationsOutput> for UnavailableApplicationsPort {
    fn execute<'a>(
        &'a self,
        _principal: &'a UserPrincipal,
        _input: ApplicationsInput,
    ) -> ConsoleInterfaceFuture<'a, ApplicationsOutput> {
        Box::pin(async {
            Err(ConsoleInterfaceTargetError(
                anyhow::anyhow!("applications fixture unavailable").into(),
            ))
        })
    }
}

#[cfg(test)]
mod tests {
    use interface_runtime::BindingId;

    use super::*;

    #[test]
    fn f09a_registry_freezes_all_console_application_bindings() {
        let registry = compile_registry(Arc::new(UnavailableApplicationsPort)).unwrap();
        for declaration in DECLARATIONS {
            assert!(registry
                .binding(&BindingId::new(declaration.binding_id).unwrap())
                .is_some());
        }
        assert_eq!(registry.bindings().count(), DECLARATIONS.len());
    }
}
