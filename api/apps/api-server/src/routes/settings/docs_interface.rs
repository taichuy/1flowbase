use std::sync::Arc;

use control_plane::errors::ControlPlaneError;
use interface_runtime::{InterfaceContract, UserPrincipal};
use serde_json::Value;
use storage_durable_postgres::MainDurableStore;

use super::DocsCategoryOperationsQuery;
use crate::{
    error_response::ApiError,
    openapi_docs::{
        ApiDocsRegistry, DocsCatalog, DocsCatalogCategoryOperationsPage,
        build_api_docs_registry_with_cookie_name, filter_category_operations,
        paginate_category_operations,
    },
    routes::console_interface::{
        self, ConsoleInterfaceDeclaration, ConsoleInterfaceFuture, ConsoleInterfacePort,
        ConsoleInterfaceTargetError,
    },
    runtime_data_model_docs,
};

pub(crate) enum DocsInput {
    Catalog,
    CategoryOperations {
        category_id: String,
        query: DocsCategoryOperationsQuery,
    },
    CategoryOpenApi {
        category_id: String,
    },
    OperationOpenApi {
        operation_id: String,
    },
}

impl InterfaceContract for DocsInput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::union_schema(vec![
            mp::object_schema(&[("variant", mp::tag_schema("Catalog"))]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("CategoryOperations")),
                ("category_id", mp::text_schema()),
                (
                    "query",
                    mp::object_schema(&[
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
                ("category_id", mp::text_schema()),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("OperationOpenApi")),
                ("operation_id", mp::text_schema()),
            ]),
        ]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(match self {
            Self::Catalog => {
                mp::object_value(&[("variant", serde_json::Value::String("Catalog".to_owned()))])
            }
            Self::CategoryOperations {
                category_id: _field_category_id,
                query: _field_query,
                ..
            } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("CategoryOperations".to_owned()),
                ),
                ("category_id", mp::text(_field_category_id)?),
                (
                    "query",
                    mp::object_value(&[
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
                category_id: _field_category_id,
                ..
            } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("CategoryOpenApi".to_owned()),
                ),
                ("category_id", mp::text(_field_category_id)?),
            ]),
            Self::OperationOpenApi {
                operation_id: _field_operation_id,
                ..
            } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("OperationOpenApi".to_owned()),
                ),
                ("operation_id", mp::text(_field_operation_id)?),
            ]),
        })
    }

    const CONTRACT_ID: &'static str = "console-docs-input";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) enum DocsOutput {
    Catalog(DocsCatalog),
    CategoryOperations(DocsCatalogCategoryOperationsPage),
    OpenApi(Value),
}

impl InterfaceContract for DocsOutput {
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

    const CONTRACT_ID: &'static str = "console-docs-output";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) struct DocsDependencies {
    pub(crate) store: MainDurableStore,
    pub(crate) api_docs: Arc<ApiDocsRegistry>,
    pub(crate) template_catalog:
        runtime_core::data_model_template_registry::DataModelTemplateCatalog,
    pub(crate) cookie_name: String,
}

struct DocsAdapter(DocsDependencies);

pub(crate) fn port(
    dependencies: DocsDependencies,
) -> Arc<dyn ConsoleInterfacePort<DocsInput, DocsOutput>> {
    Arc::new(DocsAdapter(dependencies))
}

impl DocsAdapter {
    async fn extension_docs(&self) -> Result<ApiDocsRegistry, ApiError> {
        let document = crate::openapi::workflow_extension_openapi_document_with(
            self.0.store.clone(),
            &self.0.cookie_name,
            &self.0.template_catalog,
        )
        .await?;
        Ok(build_api_docs_registry_with_cookie_name(
            document,
            &self.0.cookie_name,
        )?)
    }

    async fn ready_models(
        &self,
        principal: &UserPrincipal,
    ) -> Result<Vec<domain::ModelDefinitionRecord>, ApiError> {
        runtime_data_model_docs::ready_models_with(self.0.store.clone(), principal.actor().user_id)
            .await
    }

    async fn execute_inner(
        &self,
        principal: &UserPrincipal,
        input: DocsInput,
    ) -> Result<DocsOutput, ApiError> {
        match input {
            DocsInput::Catalog => {
                let extension_docs = self.extension_docs().await?;
                let mut catalog = self.0.api_docs.catalog().clone();
                catalog
                    .categories
                    .extend(extension_docs.catalog().categories.iter().cloned());
                let models = self.ready_models(principal).await?;
                if let Some(category) =
                    runtime_data_model_docs::build_category(&models, &self.0.template_catalog)
                {
                    catalog.categories.push(category);
                }
                Ok(DocsOutput::Catalog(catalog))
            }
            DocsInput::CategoryOperations { category_id, query } => {
                if category_id == runtime_data_model_docs::DATA_MODEL_DOCS_CATEGORY_ID {
                    let models = self.ready_models(principal).await?;
                    if models.is_empty() {
                        return Err(ControlPlaneError::NotFound("category_id").into());
                    }
                    let operations = runtime_data_model_docs::build_category_operations(
                        &models,
                        &self.0.template_catalog,
                    );
                    let filtered = filter_category_operations(&operations, query.search_query());
                    return Ok(DocsOutput::CategoryOperations(
                        paginate_category_operations(&filtered, query.offset(), query.limit()),
                    ));
                }
                if let Some(operations) = self.0.api_docs.category_operations(&category_id) {
                    let filtered = filter_category_operations(operations, query.search_query());
                    return Ok(DocsOutput::CategoryOperations(
                        paginate_category_operations(&filtered, query.offset(), query.limit()),
                    ));
                }
                let extension_docs = self.extension_docs().await?;
                let operations = extension_docs
                    .category_operations(&category_id)
                    .ok_or(ControlPlaneError::NotFound("category_id"))?;
                let filtered = filter_category_operations(operations, query.search_query());
                Ok(DocsOutput::CategoryOperations(
                    paginate_category_operations(&filtered, query.offset(), query.limit()),
                ))
            }
            DocsInput::CategoryOpenApi { category_id } => {
                if category_id == runtime_data_model_docs::DATA_MODEL_DOCS_CATEGORY_ID {
                    let models = self.ready_models(principal).await?;
                    if models.is_empty() {
                        return Err(ControlPlaneError::NotFound("category_id").into());
                    }
                    return Ok(DocsOutput::OpenApi(
                        runtime_data_model_docs::build_category_openapi(
                            &models,
                            &self.0.template_catalog,
                        ),
                    ));
                }
                if let Some(spec) = self.0.api_docs.category_spec(&category_id) {
                    return Ok(DocsOutput::OpenApi(spec.clone()));
                }
                let extension_docs = self.extension_docs().await?;
                let spec = extension_docs
                    .category_spec(&category_id)
                    .cloned()
                    .ok_or(ControlPlaneError::NotFound("category_id"))?;
                Ok(DocsOutput::OpenApi(spec))
            }
            DocsInput::OperationOpenApi { operation_id } => {
                let parsed = runtime_data_model_docs::parse_operation_id(&operation_id)
                    .map_err(|_| ControlPlaneError::InvalidInput("operation_id"))?;
                if let Some((model_id, operation_code)) = parsed {
                    let Some(model) = runtime_data_model_docs::ready_model_with(
                        self.0.store.clone(),
                        principal.actor().user_id,
                        model_id,
                    )
                    .await?
                    else {
                        return Err(ControlPlaneError::NotFound("operation_id").into());
                    };
                    let spec = runtime_data_model_docs::build_operation_openapi(
                        &model,
                        &operation_code,
                        &self.0.template_catalog,
                    )
                    .ok_or(ControlPlaneError::NotFound("operation_id"))?;
                    return Ok(DocsOutput::OpenApi(spec));
                }
                if let Some(spec) = self.0.api_docs.operation_spec(&operation_id) {
                    return Ok(DocsOutput::OpenApi(spec.clone()));
                }
                let extension_docs = self.extension_docs().await?;
                let spec = extension_docs
                    .operation_spec(&operation_id)
                    .cloned()
                    .ok_or(ControlPlaneError::NotFound("operation_id"))?;
                Ok(DocsOutput::OpenApi(spec))
            }
        }
    }
}

impl ConsoleInterfacePort<DocsInput, DocsOutput> for DocsAdapter {
    fn execute<'a>(
        &'a self,
        principal: &'a UserPrincipal,
        input: DocsInput,
    ) -> ConsoleInterfaceFuture<'a, DocsOutput> {
        Box::pin(async move {
            self.execute_inner(principal, input)
                .await
                .map_err(ConsoleInterfaceTargetError)
        })
    }
}

pub(crate) const DECLARATIONS: &[ConsoleInterfaceDeclaration] = &[
    ConsoleInterfaceDeclaration {
        interface_id: "docs.catalog.view",
        binding_id: "http.console.docs.catalog.get.v1",
        method: "GET",
        path: "/api/console/docs/catalog",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "docs.categories.operations.list",
        binding_id: "http.console.docs.category-operations.get.v1",
        method: "GET",
        path: "/api/console/docs/categories/:category_id/operations",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "docs.categories.openapi.view",
        binding_id: "http.console.docs.category-openapi.get.v1",
        method: "GET",
        path: "/api/console/docs/categories/:category_id/openapi.json",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "docs.operations.openapi.view",
        binding_id: "http.console.docs.operation-openapi.get.v1",
        method: "GET",
        path: "/api/console/docs/operations/:operation_id/openapi.json",
        mutating: false,
    },
];

pub(crate) fn compile_registry(
    port: Arc<dyn ConsoleInterfacePort<DocsInput, DocsOutput>>,
) -> Result<
    Arc<interface_runtime::CompiledInterfaceRegistry>,
    interface_runtime::RegistryCompilationError,
> {
    console_interface::compile_registry(
        "api-server.console-docs",
        "graph:console-docs-v1",
        DECLARATIONS,
        port,
    )
}

#[cfg(test)]
struct UnavailableDocsPort;

#[cfg(test)]
impl ConsoleInterfacePort<DocsInput, DocsOutput> for UnavailableDocsPort {
    fn execute<'a>(
        &'a self,
        _principal: &'a UserPrincipal,
        _input: DocsInput,
    ) -> ConsoleInterfaceFuture<'a, DocsOutput> {
        Box::pin(async {
            Err(ConsoleInterfaceTargetError(
                anyhow::anyhow!("docs fixture unavailable").into(),
            ))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn f13a2_registry_freezes_docs_bindings() {
        let registry = compile_registry(Arc::new(UnavailableDocsPort)).unwrap();
        for declaration in DECLARATIONS {
            let binding = registry
                .binding(&interface_runtime::BindingId::new(declaration.binding_id).unwrap())
                .expect("declared docs binding must be frozen");
            let route = binding.projection().http_route().unwrap();
            assert_eq!(route.method(), declaration.method);
            assert_eq!(route.path(), declaration.path);
        }
        assert_eq!(registry.bindings().count(), DECLARATIONS.len());
    }
}
