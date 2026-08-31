use std::sync::Arc;

use control_plane::{
    application::ApplicationService,
    application_public_api::publications::{
        ApplicationPublicationService, LoadActiveApplicationPublicationCommand,
    },
};
use interface_runtime::{InterfaceContract, UserPrincipal};
use serde_json::Value;
use storage_durable_postgres::MainDurableStore;
use uuid::Uuid;

use super::ApplicationApiDocsQuery;
use crate::{
    application_public_docs::{
        build_application_public_docs_catalog, build_application_public_docs_category_operations,
        build_application_public_docs_category_spec, build_application_public_docs_operation_spec,
        ApplicationPublicDocsContext, ApplicationSessionOperation,
    },
    error_response::ApiError,
    openapi_docs::{
        filter_category_operations, paginate_category_operations, ApiDocsRegistry, DocsCatalog,
        DocsCatalogCategoryOperationsPage,
    },
    routes::console_interface::{
        self, ConsoleInterfaceDeclaration, ConsoleInterfaceFuture, ConsoleInterfacePort,
        ConsoleInterfaceTargetError, ConsoleLocaleHints,
    },
};

pub(crate) enum ApplicationDocsInput {
    Catalog {
        application_id: Uuid,
        query_locale: Option<String>,
        locale: ConsoleLocaleHints,
    },
    CategoryOperations {
        application_id: Uuid,
        category_id: String,
        query: ApplicationApiDocsQuery,
        locale: ConsoleLocaleHints,
    },
    CategoryOpenApi {
        application_id: Uuid,
        category_id: String,
        query_locale: Option<String>,
        locale: ConsoleLocaleHints,
    },
    OperationOpenApi {
        application_id: Uuid,
        operation_id: String,
        query_locale: Option<String>,
        locale: ConsoleLocaleHints,
    },
}

impl InterfaceContract for ApplicationDocsInput {
    const CONTRACT_ID: &'static str = "console-application-docs-input";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) enum ApplicationDocsOutput {
    Catalog(DocsCatalog),
    CategoryOperations(DocsCatalogCategoryOperationsPage),
    OpenApi(Value),
}

impl ApplicationDocsOutput {
    pub(super) fn into_catalog(self) -> Result<DocsCatalog, ApiError> {
        match self {
            Self::Catalog(value) => Ok(value),
            _ => Err(output_error()),
        }
    }

    pub(super) fn into_category_operations(
        self,
    ) -> Result<DocsCatalogCategoryOperationsPage, ApiError> {
        match self {
            Self::CategoryOperations(value) => Ok(value),
            _ => Err(output_error()),
        }
    }

    pub(super) fn into_openapi(self) -> Result<Value, ApiError> {
        match self {
            Self::OpenApi(value) => Ok(value),
            _ => Err(output_error()),
        }
    }
}

fn output_error() -> ApiError {
    control_plane::errors::ControlPlaneError::InvalidInput("application_docs_output").into()
}

impl InterfaceContract for ApplicationDocsOutput {
    const CONTRACT_ID: &'static str = "console-application-docs-output";
    const CONTRACT_VERSION: &'static str = "1";
}

struct ApplicationDocsAdapter {
    store: MainDurableStore,
    api_docs: Arc<ApiDocsRegistry>,
}

pub(crate) fn port(
    store: MainDurableStore,
    api_docs: Arc<ApiDocsRegistry>,
) -> Arc<dyn ConsoleInterfacePort<ApplicationDocsInput, ApplicationDocsOutput>> {
    Arc::new(ApplicationDocsAdapter { store, api_docs })
}

impl ApplicationDocsAdapter {
    async fn context(
        &self,
        principal: &UserPrincipal,
        application_id: Uuid,
        query_locale: Option<String>,
        locale: ConsoleLocaleHints,
    ) -> Result<ApplicationPublicDocsContext, ApiError> {
        let actor = principal.actor();
        let preferred_locale = self
            .store
            .find_user_by_id(actor.user_id)
            .await?
            .ok_or(control_plane::errors::ControlPlaneError::NotAuthenticated)?
            .preferred_locale;
        let locale = locale.resolve_with_query(query_locale, preferred_locale);
        let actor_store = self.store.for_actor(actor.clone());
        let application = ApplicationService::new(actor_store.clone())
            .get_application(actor.user_id, application_id)
            .await?;
        let active_publication = ApplicationPublicationService::new(actor_store)
            .load_active_publication(LoadActiveApplicationPublicationCommand { application_id })
            .await
            .ok();
        Ok(ApplicationPublicDocsContext {
            application,
            active_publication,
            locale: locale.as_str().to_string(),
            assistant_operations: [
                "assistant_start_run_stream",
                "assistant_create_websocket_ticket",
                "assistant_runs_websocket",
            ]
            .into_iter()
            .filter_map(|operation_id| {
                Some(ApplicationSessionOperation {
                    operation: self.api_docs.operation(operation_id)?,
                    spec: self.api_docs.operation_spec(operation_id)?.clone(),
                })
            })
            .collect(),
        })
    }
}

impl ConsoleInterfacePort<ApplicationDocsInput, ApplicationDocsOutput> for ApplicationDocsAdapter {
    fn execute<'a>(
        &'a self,
        principal: &'a UserPrincipal,
        input: ApplicationDocsInput,
    ) -> ConsoleInterfaceFuture<'a, ApplicationDocsOutput> {
        Box::pin(async move {
            let result: Result<ApplicationDocsOutput, ApiError> = async {
                let output = match input {
                    ApplicationDocsInput::Catalog {
                        application_id,
                        query_locale,
                        locale,
                    } => {
                        let context = self
                            .context(principal, application_id, query_locale, locale)
                            .await?;
                        ApplicationDocsOutput::Catalog(build_application_public_docs_catalog(
                            &context,
                        ))
                    }
                    ApplicationDocsInput::CategoryOperations {
                        application_id,
                        category_id,
                        query,
                        locale,
                    } => {
                        let context = self
                            .context(principal, application_id, query.locale.clone(), locale)
                            .await?;
                        let operations = build_application_public_docs_category_operations(
                            &context,
                            &category_id,
                        )
                        .ok_or(
                            control_plane::errors::ControlPlaneError::NotFound(
                                "application_api_docs_category",
                            ),
                        )?;
                        let filtered =
                            filter_category_operations(&operations, query.search_query());
                        ApplicationDocsOutput::CategoryOperations(paginate_category_operations(
                            &filtered,
                            query.offset(),
                            query.limit(),
                        ))
                    }
                    ApplicationDocsInput::CategoryOpenApi {
                        application_id,
                        category_id,
                        query_locale,
                        locale,
                    } => {
                        let context = self
                            .context(principal, application_id, query_locale, locale)
                            .await?;
                        ApplicationDocsOutput::OpenApi(
                            build_application_public_docs_category_spec(&context, &category_id)
                                .ok_or(control_plane::errors::ControlPlaneError::NotFound(
                                    "application_api_docs_category",
                                ))?,
                        )
                    }
                    ApplicationDocsInput::OperationOpenApi {
                        application_id,
                        operation_id,
                        query_locale,
                        locale,
                    } => {
                        let context = self
                            .context(principal, application_id, query_locale, locale)
                            .await?;
                        ApplicationDocsOutput::OpenApi(
                            build_application_public_docs_operation_spec(&context, &operation_id)
                                .ok_or(control_plane::errors::ControlPlaneError::NotFound(
                                "application_api_docs_operation",
                            ))?,
                        )
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
        interface_id: "applications.api-docs.catalog",
        binding_id: "http.console.applications.api-docs.catalog.v1",
        method: "GET",
        path: "/api/console/applications/:application_id/api-docs/catalog",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "applications.api-docs.category-operations",
        binding_id: "http.console.applications.api-docs.category-operations.v1",
        method: "GET",
        path: "/api/console/applications/:application_id/api-docs/categories/:category_id/operations",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "applications.api-docs.category-openapi",
        binding_id: "http.console.applications.api-docs.category-openapi.v1",
        method: "GET",
        path: "/api/console/applications/:application_id/api-docs/categories/:category_id/openapi.json",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "applications.api-docs.operation-openapi",
        binding_id: "http.console.applications.api-docs.operation-openapi.v1",
        method: "GET",
        path: "/api/console/applications/:application_id/api-docs/operations/:operation_id/openapi.json",
        mutating: false,
    },
];

pub(crate) fn compile_registry(
    port: Arc<dyn ConsoleInterfacePort<ApplicationDocsInput, ApplicationDocsOutput>>,
) -> Result<
    Arc<interface_runtime::CompiledInterfaceRegistry>,
    interface_runtime::RegistryCompilationError,
> {
    console_interface::compile_registry(
        "api-server.console-application-docs",
        "api-server.console-application-docs.graph.v1",
        DECLARATIONS,
        port,
    )
}
