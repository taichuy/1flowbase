use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use control_plane::{
    application_public_api::{
        mapping::WorkflowExtensionHttpMethod,
        publications::ApplicationPublicationVersionRecord,
        published_workflow_operation::{
            build_published_workflow_operations, validate_published_workflow_contract,
            workflow_route_shapes_conflict, PublishedWorkflowOperation,
        },
    },
    ports::{ApplicationPublicationRepository, ModelDefinitionRepository},
};
use serde_json::{json, Value};

use crate::{
    app_state::ApiState, error_response::ApiError, openapi_docs::DocsCatalogOperation,
    runtime_data_model_docs,
};

use super::{catalog_entry_from_operation, OpenApiInterfaceCatalogEntry};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpenApiCapabilitySource {
    StaticApiDocs,
    ActivatedInterfaceOperation,
    BuiltinDataModelCrud,
    WorkspaceDataModelCrud,
    PublishedWorkflow,
}

impl OpenApiCapabilitySource {
    pub fn adapter_id(self) -> &'static str {
        match self {
            Self::StaticApiDocs | Self::ActivatedInterfaceOperation | Self::PublishedWorkflow => {
                "console_openapi"
            }
            Self::BuiltinDataModelCrud | Self::WorkspaceDataModelCrud => "runtime_data_model",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ActivatedInterfaceOperationProjection {
    pub operation_id: String,
    pub input_contract_id: String,
    pub input_contract_version: String,
    pub output_contract_id: String,
    pub output_contract_version: String,
    pub required_core_permission: String,
    pub auth_policy: interface_runtime::InterfaceAuthenticationPolicy,
    pub audit_policy: interface_runtime::InterfaceAuditPolicy,
    pub error_policy: interface_runtime::InterfaceErrorPolicy,
    pub graph_fingerprint: String,
    pub registry_fingerprint: String,
    pub owner: String,
}

#[derive(Debug, Clone)]
pub struct OpenApiCapabilityCatalogEntry {
    pub interface: OpenApiInterfaceCatalogEntry,
    pub source: OpenApiCapabilitySource,
    pub risk_level: &'static str,
    pub bindable: bool,
    pub disabled_reason: Option<&'static str>,
    pub activated_operation: Option<ActivatedInterfaceOperationProjection>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenApiCapabilityIndexEntry {
    pub interface_id: String,
    pub method: String,
    pub path: String,
    pub name: String,
    pub short_description: String,
    pub source: OpenApiCapabilitySource,
    pub risk_level: &'static str,
    pub bindable: bool,
    pub disabled_reason: Option<&'static str>,
}

#[derive(Debug, Clone)]
pub struct OpenApiCapabilityCatalogSummary {
    pub interface_id: String,
    pub method: String,
    pub path: String,
    pub source: OpenApiCapabilitySource,
}

#[derive(Debug, Clone)]
pub struct OpenApiCapabilityCatalogQuery {
    pub path_prefixes: Vec<String>,
    pub path_query: Option<String>,
    pub adapter_id: Option<String>,
    pub method: Option<String>,
    pub offset: usize,
    pub limit: usize,
}

#[derive(Debug, Clone)]
pub struct OpenApiCapabilityCatalogPage {
    pub items: Vec<OpenApiCapabilityCatalogSummary>,
    pub total: usize,
    pub offset: usize,
    pub limit: usize,
    pub has_more: bool,
    pub next_offset: Option<usize>,
    pub adapter_ids: Vec<String>,
    pub methods: Vec<String>,
}

pub async fn query_openapi_capability_catalog(
    state: &ApiState,
    workspace_id: uuid::Uuid,
    query: OpenApiCapabilityCatalogQuery,
) -> Result<OpenApiCapabilityCatalogPage, ApiError> {
    query_openapi_capability_catalog_with(
        &OpenApiCapabilityCatalogDependencies {
            store: state.store.clone(),
            console_operations: state.console_operation_registry.shared_inventory(),
            interface_registry: state
                .extension_boot_snapshot
                .as_ref()
                .and_then(|snapshot| snapshot.interface_registry())
                .map(|registry| registry.snapshot()),
            api_docs: Arc::clone(&state.api_docs),
            template_catalog: state.runtime_engine.template_catalog().clone(),
        },
        workspace_id,
        query,
    )
    .await
}

pub async fn query_openapi_capability_catalog_with(
    dependencies: &OpenApiCapabilityCatalogDependencies,
    workspace_id: uuid::Uuid,
    query: OpenApiCapabilityCatalogQuery,
) -> Result<OpenApiCapabilityCatalogPage, ApiError> {
    let mut summaries =
        openapi_capability_catalog_summaries_with(dependencies, workspace_id).await?;
    let adapter_ids = summaries
        .iter()
        .map(|entry| entry.source.adapter_id().to_string())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let methods = summaries
        .iter()
        .map(|entry| entry.method.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();

    if !query.path_prefixes.is_empty() {
        summaries.retain(|entry| {
            query
                .path_prefixes
                .iter()
                .any(|prefix| entry.path.starts_with(prefix))
        });
    }

    if let Some(path_query) = query
        .path_query
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let path_query = path_query.to_ascii_lowercase();
        summaries.retain(|entry| entry.path.to_ascii_lowercase().contains(&path_query));
    }
    if let Some(adapter_id) = query
        .adapter_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        summaries.retain(|entry| entry.source.adapter_id() == adapter_id);
    }
    if let Some(method) = query
        .method
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let method = method.to_ascii_uppercase();
        summaries.retain(|entry| entry.method == method);
    }

    let total = summaries.len();
    let offset = query.offset.min(total);
    let limit = query.limit.max(1);
    let end = offset.saturating_add(limit).min(total);
    let has_more = end < total;
    Ok(OpenApiCapabilityCatalogPage {
        items: summaries[offset..end].to_vec(),
        total,
        offset,
        limit,
        has_more,
        next_offset: has_more.then_some(end),
        adapter_ids,
        methods,
    })
}

pub async fn get_openapi_capability(
    state: &ApiState,
    workspace_id: uuid::Uuid,
    interface_id: &str,
) -> Result<Option<OpenApiCapabilityCatalogEntry>, ApiError> {
    get_openapi_capability_with(
        &OpenApiCapabilityCatalogDependencies {
            store: state.store.clone(),
            console_operations: state.console_operation_registry.shared_inventory(),
            interface_registry: state
                .extension_boot_snapshot
                .as_ref()
                .and_then(|snapshot| snapshot.interface_registry())
                .map(|registry| registry.snapshot()),
            api_docs: Arc::clone(&state.api_docs),
            template_catalog: state.runtime_engine.template_catalog().clone(),
        },
        workspace_id,
        interface_id,
    )
    .await
}

pub async fn get_openapi_capability_with(
    dependencies: &OpenApiCapabilityCatalogDependencies,
    workspace_id: uuid::Uuid,
    interface_id: &str,
) -> Result<Option<OpenApiCapabilityCatalogEntry>, ApiError> {
    if let Some((operation, spec)) = static_operation(dependencies, interface_id) {
        return Ok(static_entry(&operation, &spec));
    }
    if let Some((model, operation_code, operation)) =
        model_operation(dependencies, workspace_id, interface_id).await?
    {
        let entry = runtime_data_model_docs::build_operation_openapi(
            &model,
            &operation_code,
            &dependencies.template_catalog,
        )
        .and_then(|spec| catalog_entry_from_operation(&operation, &spec))
        .map(|interface| capability_entry(interface, model_source(&model), None));
        return Ok(entry);
    }
    if let Some(publication) =
        workflow_publication(dependencies, workspace_id, interface_id).await?
    {
        let operation = PublishedWorkflowOperation::from_publication(publication)
            .map_err(|_| control_plane::errors::ControlPlaneError::Conflict("workflow_route"))?;
        return workflow_entry(operation).map(Some);
    }
    Ok(missing_interface(dependencies, interface_id).map(missing_openapi_entry))
}

pub async fn get_openapi_capability_by_route(
    state: &ApiState,
    workspace_id: uuid::Uuid,
    method: &str,
    path: &str,
) -> Result<Option<OpenApiCapabilityCatalogEntry>, ApiError> {
    get_openapi_capability_by_route_with(
        &OpenApiCapabilityCatalogDependencies {
            store: state.store.clone(),
            console_operations: state.console_operation_registry.shared_inventory(),
            interface_registry: state
                .extension_boot_snapshot
                .as_ref()
                .and_then(|snapshot| snapshot.interface_registry())
                .map(|registry| registry.snapshot()),
            api_docs: Arc::clone(&state.api_docs),
            template_catalog: state.runtime_engine.template_catalog().clone(),
        },
        workspace_id,
        method,
        path,
    )
    .await
}

pub async fn get_openapi_capability_by_route_with(
    dependencies: &OpenApiCapabilityCatalogDependencies,
    workspace_id: uuid::Uuid,
    method: &str,
    path: &str,
) -> Result<Option<OpenApiCapabilityCatalogEntry>, ApiError> {
    let route = route_identity(method, path);
    let mut matches = build_openapi_capability_catalog_with(dependencies, workspace_id)
        .await?
        .into_iter()
        .filter(|entry| route_identity(&entry.interface.method, &entry.interface.path) == route)
        .filter(|entry| entry.bindable);
    let found = matches.next();
    if matches.next().is_some() {
        return Err(
            control_plane::errors::ControlPlaneError::Conflict("openapi_capability_route").into(),
        );
    }
    Ok(found)
}

async fn openapi_capability_catalog_summaries_with(
    dependencies: &OpenApiCapabilityCatalogDependencies,
    workspace_id: uuid::Uuid,
) -> Result<Vec<OpenApiCapabilityCatalogSummary>, ApiError> {
    let mut summaries = list_openapi_capability_index_with(dependencies, workspace_id)
        .await?
        .into_iter()
        .filter(|entry| {
            matches!(
                entry.source,
                OpenApiCapabilitySource::BuiltinDataModelCrud
                    | OpenApiCapabilitySource::WorkspaceDataModelCrud
                    | OpenApiCapabilitySource::PublishedWorkflow
            ) || static_operation_is_bindable(&entry.path)
        })
        .map(|entry| OpenApiCapabilityCatalogSummary {
            interface_id: entry.interface_id,
            method: entry.method,
            path: entry.path,
            source: entry.source,
        })
        .collect::<Vec<_>>();
    summaries.sort_by(|left, right| {
        left.path
            .cmp(&right.path)
            .then_with(|| left.method.cmp(&right.method))
            .then_with(|| left.interface_id.cmp(&right.interface_id))
    });
    Ok(summaries)
}

pub async fn build_openapi_capability_catalog(
    state: &ApiState,
    workspace_id: uuid::Uuid,
) -> Result<Vec<OpenApiCapabilityCatalogEntry>, ApiError> {
    build_openapi_capability_catalog_with(
        &OpenApiCapabilityCatalogDependencies {
            store: state.store.clone(),
            console_operations: state.console_operation_registry.shared_inventory(),
            interface_registry: state
                .extension_boot_snapshot
                .as_ref()
                .and_then(|snapshot| snapshot.interface_registry())
                .map(|registry| registry.snapshot()),
            api_docs: Arc::clone(&state.api_docs),
            template_catalog: state.runtime_engine.template_catalog().clone(),
        },
        workspace_id,
    )
    .await
}

#[derive(Clone)]
pub struct OpenApiCapabilityCatalogDependencies {
    pub store: storage_durable_postgres::MainDurableStore,
    pub console_operations: Arc<access_control::ConsoleOperationCompiledInventory>,
    pub interface_registry: Option<Arc<interface_runtime::CompiledInterfaceRegistry>>,
    pub api_docs: Arc<crate::openapi_docs::ApiDocsRegistry>,
    pub template_catalog: runtime_core::data_model_template_registry::DataModelTemplateCatalog,
}

/// Include the documented operation IDs: MCP wrappers can address the same
/// console route through its OpenAPI ID instead of its compiled interface ID.
pub fn root_workspace_only_capability_ids<'a>(
    inventory: &'a access_control::ConsoleOperationCompiledInventory,
    api_docs: &'a crate::openapi_docs::ApiDocsRegistry,
) -> BTreeSet<&'a str> {
    let mut ids = inventory.root_workspace_only_interface_ids();
    let restricted_routes = inventory
        .operations
        .iter()
        .filter(|operation| operation.root_workspace_only)
        .flat_map(|operation| &operation.routes)
        .map(|route| route_identity(&route.method, &route.path))
        .collect::<BTreeSet<_>>();
    for category in &api_docs.catalog().categories {
        if let Some(operations) = api_docs.category_operations(&category.id) {
            ids.extend(
                operations
                    .operations
                    .iter()
                    .filter(|operation| {
                        restricted_routes
                            .contains(&route_identity(&operation.method, &operation.path))
                    })
                    .map(|operation| operation.id.as_str()),
            );
        }
    }
    ids
}

pub async fn build_openapi_capability_catalog_with(
    dependencies: &OpenApiCapabilityCatalogDependencies,
    workspace_id: uuid::Uuid,
) -> Result<Vec<OpenApiCapabilityCatalogEntry>, ApiError> {
    let mut entries = Vec::new();
    let mut documented_routes = BTreeSet::new();
    for operation in static_operations(dependencies) {
        let Some(spec) = dependencies.api_docs.operation_spec_view(&operation.id) else {
            continue;
        };
        if let Some(entry) = static_entry(operation, &spec) {
            documented_routes.insert(route_identity(&operation.method, &operation.path));
            entries.push(entry);
        }
    }
    entries.extend(missing_interfaces(dependencies, &documented_routes).map(missing_openapi_entry));

    for model in published_models(dependencies, workspace_id).await? {
        let Some(spec) =
            runtime_data_model_docs::build_model_openapi(&model, &dependencies.template_catalog)
        else {
            continue;
        };
        let source = model_source(&model);
        let operations = runtime_data_model_docs::build_category_operations(
            std::slice::from_ref(&model),
            &dependencies.template_catalog,
        );
        entries.extend(operations.operations.iter().filter_map(|operation| {
            catalog_entry_from_operation(operation, &spec)
                .map(|interface| capability_entry(interface, source, None))
        }));
    }

    let publications = dependencies
        .store
        .list_enabled_extension_publications()
        .await?;
    let operations = build_published_workflow_operations(publications)
        .map_err(|_| control_plane::errors::ControlPlaneError::Conflict("workflow_route"))?;
    for operation in operations
        .into_iter()
        .filter(|operation| operation.workspace_id == workspace_id)
    {
        entries.push(workflow_entry(operation)?);
    }

    entries.sort_by(|left, right| {
        left.interface
            .operation_id
            .cmp(&right.interface.operation_id)
    });
    Ok(entries)
}

pub async fn list_openapi_capability_index_with(
    dependencies: &OpenApiCapabilityCatalogDependencies,
    workspace_id: uuid::Uuid,
) -> Result<Vec<OpenApiCapabilityIndexEntry>, ApiError> {
    let mut entries = Vec::new();
    let mut documented_routes = BTreeSet::new();
    for operation in static_operations(dependencies) {
        let Some(spec) = dependencies.api_docs.operation_spec_view(&operation.id) else {
            continue;
        };
        if let Some(entry) = static_index_entry(operation, &spec) {
            documented_routes.insert(route_identity(&operation.method, &operation.path));
            entries.push(entry);
        }
    }
    entries.extend(missing_interfaces(dependencies, &documented_routes).map(missing_index_entry));

    for model in published_models(dependencies, workspace_id).await? {
        let source = model_source(&model);
        let operations = runtime_data_model_docs::build_category_operations(
            std::slice::from_ref(&model),
            &dependencies.template_catalog,
        );
        entries.extend(
            operations
                .operations
                .into_iter()
                .map(|operation| index_entry(operation, source, None)),
        );
    }

    let publications = dependencies
        .store
        .list_enabled_extension_publications()
        .await?;
    // Preserve the legacy catalog's validation of the globally mounted workflow routes.
    let mut workflows = Vec::new();
    for publication in &publications {
        let operation = workflow_metadata(publication)?;
        if workflows
            .iter()
            .any(|(_, candidate): &(uuid::Uuid, DocsCatalogOperation)| {
                candidate.id != operation.id
                    && candidate.method == operation.method
                    && workflow_route_shapes_conflict(&candidate.path, &operation.path)
            })
        {
            return Err(
                control_plane::errors::ControlPlaneError::Conflict("workflow_route").into(),
            );
        }
        workflows.push((publication.workspace_id, operation));
    }
    entries.extend(
        workflows
            .into_iter()
            .filter(|(id, _)| *id == workspace_id)
            .map(|(_, operation)| {
                index_entry(operation, OpenApiCapabilitySource::PublishedWorkflow, None)
            }),
    );
    entries.sort_by(|left, right| left.interface_id.cmp(&right.interface_id));
    Ok(entries)
}

pub async fn get_openapi_capability_index_entry_with(
    dependencies: &OpenApiCapabilityCatalogDependencies,
    workspace_id: uuid::Uuid,
    interface_id: &str,
) -> Result<Option<OpenApiCapabilityIndexEntry>, ApiError> {
    if let Some((operation, spec)) = static_operation(dependencies, interface_id) {
        return Ok(static_index_entry(&operation, &spec));
    }
    if let Some((model, _, operation)) =
        model_operation(dependencies, workspace_id, interface_id).await?
    {
        return Ok(Some(index_entry(operation, model_source(&model), None)));
    }
    if let Some(publication) =
        workflow_publication(dependencies, workspace_id, interface_id).await?
    {
        return Ok(Some(index_entry(
            workflow_metadata(&publication)?,
            OpenApiCapabilitySource::PublishedWorkflow,
            None,
        )));
    }
    Ok(missing_interface(dependencies, interface_id).map(missing_index_entry))
}

fn static_operations(
    dependencies: &OpenApiCapabilityCatalogDependencies,
) -> impl Iterator<Item = &DocsCatalogOperation> {
    dependencies
        .api_docs
        .catalog()
        .categories
        .iter()
        .filter_map(|category| dependencies.api_docs.category_operations(&category.id))
        .flat_map(|category| &category.operations)
}

fn static_operation<'a>(
    dependencies: &'a OpenApiCapabilityCatalogDependencies,
    interface_id: &str,
) -> Option<(
    DocsCatalogOperation,
    crate::openapi_docs::OperationSpecView<'a>,
)> {
    Some((
        dependencies.api_docs.operation(interface_id)?,
        dependencies.api_docs.operation_spec_view(interface_id)?,
    ))
}

fn static_entry(
    operation: &DocsCatalogOperation,
    spec: &crate::openapi_docs::OperationSpecView<'_>,
) -> Option<OpenApiCapabilityCatalogEntry> {
    let interface = super::catalog_entry_from_view(operation, spec)?;
    let disabled_reason = static_disabled_reason(
        operation,
        spec.source(),
        interface.response_media_type.as_deref(),
    );
    Some(capability_entry(
        interface,
        OpenApiCapabilitySource::StaticApiDocs,
        disabled_reason,
    ))
}

fn static_index_entry(
    operation: &DocsCatalogOperation,
    spec: &crate::openapi_docs::OperationSpecView<'_>,
) -> Option<OpenApiCapabilityIndexEntry> {
    spec.source()
        .get("paths")?
        .get(&operation.path)?
        .get(operation.method.to_ascii_lowercase())?;
    let media_type = if static_operation_is_bindable(&operation.path) {
        super::catalog::response_media_type_from_view(operation, spec)
    } else {
        None
    };
    let disabled_reason = static_disabled_reason(operation, spec.source(), media_type.as_deref());
    Some(index_entry(
        operation.clone(),
        OpenApiCapabilitySource::StaticApiDocs,
        disabled_reason,
    ))
}

fn capability_entry(
    interface: OpenApiInterfaceCatalogEntry,
    source: OpenApiCapabilitySource,
    disabled_reason: Option<&'static str>,
) -> OpenApiCapabilityCatalogEntry {
    OpenApiCapabilityCatalogEntry {
        risk_level: operation_risk_level(&interface.method),
        interface,
        source,
        bindable: disabled_reason.is_none(),
        disabled_reason,
        activated_operation: None,
    }
}

fn index_entry(
    operation: DocsCatalogOperation,
    source: OpenApiCapabilitySource,
    disabled_reason: Option<&'static str>,
) -> OpenApiCapabilityIndexEntry {
    OpenApiCapabilityIndexEntry {
        name: operation.summary.unwrap_or_else(|| operation.id.clone()),
        short_description: operation
            .description
            .unwrap_or_else(|| format!("{} {}", operation.method, operation.path)),
        risk_level: operation_risk_level(&operation.method),
        interface_id: operation.id,
        method: operation.method,
        path: operation.path,
        source,
        bindable: disabled_reason.is_none(),
        disabled_reason,
    }
}

async fn published_models(
    dependencies: &OpenApiCapabilityCatalogDependencies,
    workspace_id: uuid::Uuid,
) -> Result<Vec<domain::ModelDefinitionRecord>, ApiError> {
    let mut models = dependencies
        .store
        .list_model_definitions(workspace_id)
        .await?;
    models.retain(|model| model.status == domain::DataModelStatus::Published);
    models.sort_by(|left, right| left.code.cmp(&right.code));
    Ok(models)
}

async fn model_operation(
    dependencies: &OpenApiCapabilityCatalogDependencies,
    workspace_id: uuid::Uuid,
    interface_id: &str,
) -> Result<Option<(domain::ModelDefinitionRecord, String, DocsCatalogOperation)>, ApiError> {
    let Ok(Some((model_id, operation_code))) =
        runtime_data_model_docs::parse_operation_id(interface_id)
    else {
        return Ok(None);
    };
    let Some(model) = dependencies
        .store
        .get_model_definition(workspace_id, model_id)
        .await?
        .filter(|model| model.status == domain::DataModelStatus::Published)
    else {
        return Ok(None);
    };
    Ok(runtime_data_model_docs::operation_metadata(
        &model,
        &operation_code,
        &dependencies.template_catalog,
    )
    .filter(|operation| operation.id == interface_id)
    .map(|operation| (model, operation_code, operation)))
}

fn model_source(model: &domain::ModelDefinitionRecord) -> OpenApiCapabilitySource {
    if domain::builtin_contract_for_model(model).is_some() {
        OpenApiCapabilitySource::BuiltinDataModelCrud
    } else {
        OpenApiCapabilitySource::WorkspaceDataModelCrud
    }
}

async fn workflow_publication(
    dependencies: &OpenApiCapabilityCatalogDependencies,
    workspace_id: uuid::Uuid,
    interface_id: &str,
) -> Result<Option<ApplicationPublicationVersionRecord>, ApiError> {
    let Some(application_id) = interface_id.strip_prefix("published_workflow_operation:") else {
        return Ok(None);
    };
    Ok(dependencies
        .store
        .list_enabled_extension_publications()
        .await?
        .into_iter()
        .find(|publication| {
            publication.workspace_id == workspace_id
                && publication.application_id.to_string() == application_id
        }))
}

fn workflow_metadata(
    publication: &ApplicationPublicationVersionRecord,
) -> Result<DocsCatalogOperation, ApiError> {
    let conflict = || control_plane::errors::ControlPlaneError::Conflict("workflow_route");
    let extension = publication
        .mapping_snapshot
        .extension
        .as_ref()
        .ok_or_else(conflict)?;
    validate_published_workflow_contract(extension, &publication.document_snapshot)
        .map_err(|_| conflict())?;
    Ok(workflow_docs(
        format!(
            "published_workflow_operation:{}",
            publication.application_id
        ),
        publication.application_id,
        extension.method,
        format!("/api/ex/{}", extension.slug),
    ))
}

fn workflow_docs(
    id: String,
    application_id: uuid::Uuid,
    method: WorkflowExtensionHttpMethod,
    path: String,
) -> DocsCatalogOperation {
    DocsCatalogOperation {
        id,
        method: method.as_str().to_string(),
        path,
        summary: Some(format!("Invoke published workflow {application_id}")),
        description: Some("Invoke the active publication of a Workflow application".into()),
        tags: vec!["Workflow Extensions".into()],
        group: "workflow_extensions".into(),
        deprecated: false,
    }
}

fn workflow_entry(
    operation: PublishedWorkflowOperation,
) -> Result<OpenApiCapabilityCatalogEntry, ApiError> {
    let docs = workflow_docs(
        operation.interface_id.clone(),
        operation.application_id,
        operation.method,
        operation.public_path(),
    );
    let spec = json!({"openapi": "3.1.0", "paths": {
        (&docs.path): { (docs.method.to_ascii_lowercase()): crate::openapi::workflow_extension_operation(&operation) }
    }});
    let interface = catalog_entry_from_operation(&docs, &spec).ok_or(
        control_plane::errors::ControlPlaneError::Conflict("workflow_interface_contract"),
    )?;
    Ok(capability_entry(
        interface,
        OpenApiCapabilitySource::PublishedWorkflow,
        None,
    ))
}

fn missing_interfaces<'a>(
    dependencies: &'a OpenApiCapabilityCatalogDependencies,
    documented_routes: &'a BTreeSet<(String, String)>,
) -> impl Iterator<Item = &'a access_control::ConsoleInterfaceInventoryEntry> {
    dependencies
        .console_operations
        .interfaces
        .iter()
        .map(|interface| {
            (
                route_identity(&interface.route.method, &interface.route.path),
                interface,
            )
        })
        .collect::<BTreeMap<_, _>>()
        .into_iter()
        .filter(|(route, _)| !documented_routes.contains(route))
        .map(|(_, interface)| interface)
}

fn missing_interface<'a>(
    dependencies: &'a OpenApiCapabilityCatalogDependencies,
    interface_id: &str,
) -> Option<&'a access_control::ConsoleInterfaceInventoryEntry> {
    let interface = dependencies
        .console_operations
        .interfaces
        .iter()
        .find(|interface| interface.interface_id == interface_id)?;
    let route = route_identity(&interface.route.method, &interface.route.path);
    (!static_operations(dependencies).any(|operation| {
        route_identity(&operation.method, &operation.path) == route
            && dependencies
                .api_docs
                .operation_spec_view(&operation.id)
                .is_some()
    }))
    .then_some(interface)
}

fn missing_index_entry(
    compiled: &access_control::ConsoleInterfaceInventoryEntry,
) -> OpenApiCapabilityIndexEntry {
    let method = compiled.route.method.to_ascii_uppercase();
    OpenApiCapabilityIndexEntry {
        risk_level: operation_risk_level(&method),
        interface_id: compiled.interface_id.clone(),
        method,
        path: canonical_openapi_path(&compiled.route.path),
        name: compiled.summary.clone(),
        short_description: compiled.description.clone(),
        source: OpenApiCapabilitySource::StaticApiDocs,
        bindable: false,
        disabled_reason: Some("missing_openapi_contract"),
    }
}

pub fn operation_risk_level(method: &str) -> &'static str {
    match method.to_ascii_uppercase().as_str() {
        "GET" | "HEAD" | "OPTIONS" => "low",
        "DELETE" => "critical",
        "POST" | "PUT" | "PATCH" => "high",
        _ => "medium",
    }
}

fn static_operation_is_bindable(path: &str) -> bool {
    path == "/api/runtime/models/{model_code}/metadata"
        || path.starts_with("/api/console/")
        || path.starts_with("/api/public/")
}

fn static_disabled_reason(
    operation: &DocsCatalogOperation,
    spec: &Value,
    response_media_type: Option<&str>,
) -> Option<&'static str> {
    if !static_operation_is_bindable(&operation.path) {
        return Some("unsupported_interface_scope");
    }
    match response_media_type {
        Some(media_type) if is_json_media_type(media_type) => None,
        Some(_) => Some("unsupported_response_media_type"),
        None if has_no_content_success(operation, spec) => None,
        // The operation is documented. Its upgrade/redirect/empty carrier has
        // no response shape for an MCP interface wrapper; event phases decide
        // from their own principal, typed codec and terminal owner.
        None => Some("unsupported_response_semantics"),
    }
}

fn is_json_media_type(media_type: &str) -> bool {
    media_type.eq_ignore_ascii_case("application/json") || media_type.ends_with("+json")
}

fn has_no_content_success(operation: &DocsCatalogOperation, spec: &Value) -> bool {
    spec.pointer(&format!(
        "/paths/{}/{}/responses/204",
        escape_pointer(&operation.path),
        operation.method.to_ascii_lowercase()
    ))
    .is_some()
}

fn missing_openapi_entry(
    compiled: &access_control::ConsoleInterfaceInventoryEntry,
) -> OpenApiCapabilityCatalogEntry {
    let method = compiled.route.method.to_ascii_uppercase();
    OpenApiCapabilityCatalogEntry {
        risk_level: operation_risk_level(&method),
        interface: OpenApiInterfaceCatalogEntry {
            operation_id: compiled.interface_id.clone(),
            method,
            path: canonical_openapi_path(&compiled.route.path),
            name: compiled.summary.clone(),
            description: compiled.description.clone(),
            parameter_descriptors: Vec::new(),
            request_schema: json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false
            }),
            response_schema: Value::Bool(false),
            request_media_type: None,
            response_media_type: None,
            security: Value::Array(Vec::new()),
        },
        source: OpenApiCapabilitySource::StaticApiDocs,
        bindable: false,
        disabled_reason: Some("missing_openapi_contract"),
        activated_operation: None,
    }
}

fn route_identity(method: &str, path: &str) -> (String, String) {
    (
        method.to_ascii_uppercase(),
        path.split('/')
            .map(|segment| {
                if segment.starts_with(':') || (segment.starts_with('{') && segment.ends_with('}'))
                {
                    "{}"
                } else {
                    segment
                }
            })
            .collect::<Vec<_>>()
            .join("/"),
    )
}

fn canonical_openapi_path(path: &str) -> String {
    path.split('/')
        .map(|segment| {
            segment
                .strip_prefix(':')
                .map(|name| format!("{{{name}}}"))
                .unwrap_or_else(|| segment.to_string())
        })
        .collect::<Vec<_>>()
        .join("/")
}

fn escape_pointer(token: &str) -> String {
    token.replace('~', "~0").replace('/', "~1")
}

#[cfg(test)]
#[path = "_tests/capability_catalog.rs"]
mod tests;
