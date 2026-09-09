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
        ApplicationPublicDocsContext, ApplicationSessionOperation,
        build_application_public_docs_catalog, build_application_public_docs_category_operations,
        build_application_public_docs_category_spec, build_application_public_docs_operation_spec,
    },
    error_response::ApiError,
    openapi_docs::{
        ApiDocsRegistry, DocsCatalog, DocsCatalogCategoryOperationsPage,
        filter_category_operations, paginate_category_operations,
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
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::union_schema(vec![
            mp::object_schema(&[
                ("variant", mp::tag_schema("Catalog")),
                ("application_id", mp::text_schema()),
                (
                    "query_locale",
                    serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("CategoryOperations")),
                ("application_id", mp::text_schema()),
                ("category_id", mp::text_schema()),
                (
                    "query",
                    mp::object_schema(&[
                        (
                            "locale",
                            serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                        ),
                        (
                            "offset",
                            serde_json::json!({"anyOf": [serde_json::json!({"type":"integer"}), {"type":"null"}]}),
                        ),
                        (
                            "limit",
                            serde_json::json!({"anyOf": [serde_json::json!({"type":"integer"}), {"type":"null"}]}),
                        ),
                        (
                            "q",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("CategoryOpenApi")),
                ("application_id", mp::text_schema()),
                ("category_id", mp::text_schema()),
                (
                    "query_locale",
                    serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("OperationOpenApi")),
                ("application_id", mp::text_schema()),
                ("operation_id", mp::text_schema()),
                (
                    "query_locale",
                    serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                ),
            ]),
        ]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(match self {
            Self::Catalog {
                application_id: _field_application_id,
                query_locale: _field_query_locale,
                ..
            } => mp::object_value(&[
                ("variant", serde_json::Value::String("Catalog".to_owned())),
                (
                    "application_id",
                    serde_json::Value::String((_field_application_id).to_string()),
                ),
                (
                    "query_locale",
                    match (_field_query_locale).as_ref() {
                        Some(item) => mp::text(item)?,
                        None => serde_json::Value::Null,
                    },
                ),
            ]),
            Self::CategoryOperations {
                application_id: _field_application_id,
                category_id: _field_category_id,
                query: _field_query,
                ..
            } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("CategoryOperations".to_owned()),
                ),
                (
                    "application_id",
                    serde_json::Value::String((_field_application_id).to_string()),
                ),
                ("category_id", mp::text(_field_category_id)?),
                (
                    "query",
                    mp::object_value(&[
                        (
                            "locale",
                            match (&(_field_query).locale).as_ref() {
                                Some(item) => mp::text(item)?,
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "offset",
                            match (&(_field_query).offset).as_ref() {
                                Some(item) => serde_json::json!(*(item)),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "limit",
                            match (&(_field_query).limit).as_ref() {
                                Some(item) => serde_json::json!(*(item)),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "q",
                            match (&(_field_query).q).as_ref() {
                                Some(item) => mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
                        ),
                    ]),
                ),
            ]),
            Self::CategoryOpenApi {
                application_id: _field_application_id,
                category_id: _field_category_id,
                query_locale: _field_query_locale,
                ..
            } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("CategoryOpenApi".to_owned()),
                ),
                (
                    "application_id",
                    serde_json::Value::String((_field_application_id).to_string()),
                ),
                ("category_id", mp::text(_field_category_id)?),
                (
                    "query_locale",
                    match (_field_query_locale).as_ref() {
                        Some(item) => mp::text(item)?,
                        None => serde_json::Value::Null,
                    },
                ),
            ]),
            Self::OperationOpenApi {
                application_id: _field_application_id,
                operation_id: _field_operation_id,
                query_locale: _field_query_locale,
                ..
            } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("OperationOpenApi".to_owned()),
                ),
                (
                    "application_id",
                    serde_json::Value::String((_field_application_id).to_string()),
                ),
                ("operation_id", mp::text(_field_operation_id)?),
                (
                    "query_locale",
                    match (_field_query_locale).as_ref() {
                        Some(item) => mp::text(item)?,
                        None => serde_json::Value::Null,
                    },
                ),
            ]),
        })
    }

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
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::union_schema(vec![
            mp::object_schema(&[
                ("variant", mp::tag_schema("Catalog")),
                (
                    "0",
                    mp::object_schema(&[
                        (
                            "title",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        ("version", mp::text_schema()),
                        (
                            "categories",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("id",mp::text_schema()), ("label",mp::object_schema(&[("byte_count",mp::count_schema())])), ("operation_count",serde_json::json!({"type":"integer"}))])}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("CategoryOperations")),
                (
                    "0",
                    mp::object_schema(&[
                        ("id", mp::text_schema()),
                        (
                            "label",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "operations",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("id",mp::text_schema()), ("method",mp::text_schema()), ("path",mp::object_schema(&[("byte_count",mp::count_schema())])), ("summary",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("description",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("tags",mp::object_schema(&[("item_count",mp::count_schema())])), ("group",mp::object_schema(&[("byte_count",mp::count_schema())])), ("deprecated",serde_json::json!({"type":"boolean"}))])}),
                        ),
                        ("total", serde_json::json!({"type":"integer"})),
                        ("offset", serde_json::json!({"type":"integer"})),
                        ("limit", serde_json::json!({"type":"integer"})),
                        ("has_more", serde_json::json!({"type":"boolean"})),
                        (
                            "next_offset",
                            serde_json::json!({"anyOf": [serde_json::json!({"type":"integer"}), {"type":"null"}]}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("OpenApi")),
                ("0", mp::json_summary_schema()),
            ]),
        ]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(match self {
            Self::Catalog(_field_0) => mp::object_value(&[
                ("variant", serde_json::Value::String("Catalog".to_owned())),
                (
                    "0",
                    mp::object_value(&[
                        (
                            "title",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_0).title).len()),
                            )]),
                        ),
                        ("version", mp::text(&(_field_0).version)?),
                        ("categories", {
                            if (&(_field_0).categories).len() > 32 {
                                return None;
                            }
                            serde_json::Value::Array(
                                (&(_field_0).categories)
                                    .iter()
                                    .map(|item| {
                                        Some(mp::object_value(&[
                                            ("id", mp::text(&(item).id)?),
                                            (
                                                "label",
                                                mp::object_value(&[(
                                                    "byte_count",
                                                    serde_json::json!((&(item).label).len()),
                                                )]),
                                            ),
                                            (
                                                "operation_count",
                                                serde_json::json!(*(&(item).operation_count)),
                                            ),
                                        ]))
                                    })
                                    .collect::<Option<Vec<_>>>()?,
                            )
                        }),
                    ]),
                ),
            ]),
            Self::CategoryOperations(_field_0) => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("CategoryOperations".to_owned()),
                ),
                (
                    "0",
                    mp::object_value(&[
                        ("id", mp::text(&(_field_0).id)?),
                        (
                            "label",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_0).label).len()),
                            )]),
                        ),
                        ("operations", {
                            if (&(_field_0).operations).len() > 32 {
                                return None;
                            }
                            serde_json::Value::Array(
                                (&(_field_0).operations)
                                    .iter()
                                    .map(|item| {
                                        Some(mp::object_value(&[
                                            ("id", mp::text(&(item).id)?),
                                            ("method", mp::text(&(item).method)?),
                                            (
                                                "path",
                                                mp::object_value(&[(
                                                    "byte_count",
                                                    serde_json::json!((&(item).path).len()),
                                                )]),
                                            ),
                                            (
                                                "summary",
                                                match (&(item).summary).as_ref() {
                                                    Some(item) => mp::object_value(&[(
                                                        "byte_count",
                                                        serde_json::json!((item).len()),
                                                    )]),
                                                    None => serde_json::Value::Null,
                                                },
                                            ),
                                            (
                                                "description",
                                                match (&(item).description).as_ref() {
                                                    Some(item) => mp::object_value(&[(
                                                        "byte_count",
                                                        serde_json::json!((item).len()),
                                                    )]),
                                                    None => serde_json::Value::Null,
                                                },
                                            ),
                                            (
                                                "tags",
                                                mp::object_value(&[(
                                                    "item_count",
                                                    serde_json::json!((&(item).tags).len()),
                                                )]),
                                            ),
                                            (
                                                "group",
                                                mp::object_value(&[(
                                                    "byte_count",
                                                    serde_json::json!((&(item).group).len()),
                                                )]),
                                            ),
                                            (
                                                "deprecated",
                                                serde_json::Value::Bool(*(&(item).deprecated)),
                                            ),
                                        ]))
                                    })
                                    .collect::<Option<Vec<_>>>()?,
                            )
                        }),
                        ("total", serde_json::json!(*(&(_field_0).total))),
                        ("offset", serde_json::json!(*(&(_field_0).offset))),
                        ("limit", serde_json::json!(*(&(_field_0).limit))),
                        ("has_more", serde_json::Value::Bool(*(&(_field_0).has_more))),
                        (
                            "next_offset",
                            match (&(_field_0).next_offset).as_ref() {
                                Some(item) => serde_json::json!(*(item)),
                                None => serde_json::Value::Null,
                            },
                        ),
                    ]),
                ),
            ]),
            Self::OpenApi(_field_0) => mp::object_value(&[
                ("variant", serde_json::Value::String("OpenApi".to_owned())),
                ("0", mp::json_summary(_field_0)),
            ]),
        })
    }

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
