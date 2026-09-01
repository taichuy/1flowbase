use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use control_plane::ports::ModelDefinitionRepository;
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
}

impl OpenApiCapabilitySource {
    pub fn adapter_id(self) -> &'static str {
        match self {
            Self::StaticApiDocs | Self::ActivatedInterfaceOperation => "console_openapi",
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
            console_operations: state.console_operation_registry.inventory().clone(),
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

pub(crate) async fn query_openapi_capability_catalog_with(
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
            console_operations: state.console_operation_registry.inventory().clone(),
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

pub(crate) async fn get_openapi_capability_with(
    dependencies: &OpenApiCapabilityCatalogDependencies,
    workspace_id: uuid::Uuid,
    interface_id: &str,
) -> Result<Option<OpenApiCapabilityCatalogEntry>, ApiError> {
    Ok(
        build_openapi_capability_catalog_with(dependencies, workspace_id)
            .await?
            .into_iter()
            .find(|entry| entry.interface.operation_id == interface_id),
    )
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
            console_operations: state.console_operation_registry.inventory().clone(),
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

pub(crate) async fn get_openapi_capability_by_route_with(
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
    let mut summaries = build_openapi_capability_catalog_with(dependencies, workspace_id)
        .await?
        .into_iter()
        .filter(|entry| {
            matches!(
                entry.source,
                OpenApiCapabilitySource::BuiltinDataModelCrud
                    | OpenApiCapabilitySource::WorkspaceDataModelCrud
            ) || static_operation_is_bindable(&entry.interface.path)
        })
        .map(|entry| OpenApiCapabilityCatalogSummary {
            interface_id: entry.interface.operation_id,
            method: entry.interface.method,
            path: entry.interface.path,
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
            console_operations: state.console_operation_registry.inventory().clone(),
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
pub(crate) struct OpenApiCapabilityCatalogDependencies {
    pub(crate) store: storage_durable_postgres::MainDurableStore,
    pub(crate) console_operations: access_control::ConsoleOperationCompiledInventory,
    pub(crate) interface_registry: Option<Arc<interface_runtime::CompiledInterfaceRegistry>>,
    pub(crate) api_docs: Arc<crate::openapi_docs::ApiDocsRegistry>,
    pub(crate) template_catalog:
        runtime_core::data_model_template_registry::DataModelTemplateCatalog,
}

pub(crate) async fn build_openapi_capability_catalog_with(
    dependencies: &OpenApiCapabilityCatalogDependencies,
    workspace_id: uuid::Uuid,
) -> Result<Vec<OpenApiCapabilityCatalogEntry>, ApiError> {
    let mut entries = Vec::new();
    let compiled_interfaces = dependencies
        .console_operations
        .interfaces
        .iter()
        .map(|interface| {
            (
                route_identity(&interface.route.method, &interface.route.path),
                interface,
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut documented_console_routes = BTreeSet::new();
    let interface_snapshot = dependencies.interface_registry.clone();

    for category in &dependencies.api_docs.catalog().categories {
        let Some(operations) = dependencies.api_docs.category_operations(&category.id) else {
            continue;
        };
        for operation in &operations.operations {
            let Some(spec) = dependencies.api_docs.operation_spec(&operation.id) else {
                continue;
            };
            let Some(interface) = catalog_entry_from_operation(operation, spec) else {
                continue;
            };
            let route = route_identity(&operation.method, &operation.path);
            if interface_snapshot.as_deref().is_some_and(|registry| {
                activated_providers_view_route_matches(registry, &interface)
            }) {
                let registry = interface_snapshot
                    .as_deref()
                    .expect("activated interface route requires a registry snapshot");
                let projected = activated_providers_view_entry(registry, interface)?;
                documented_console_routes.insert(route);
                entries.push(projected);
                continue;
            }
            if compiled_interfaces.contains_key(&route) {
                documented_console_routes.insert(route);
            }
            let disabled_reason = static_disabled_reason(operation, spec, &interface);
            entries.push(OpenApiCapabilityCatalogEntry {
                risk_level: operation_risk_level(&interface.method),
                interface,
                source: OpenApiCapabilitySource::StaticApiDocs,
                bindable: disabled_reason.is_none(),
                disabled_reason,
                activated_operation: None,
            });
        }
    }

    entries.extend(
        compiled_interfaces
            .into_iter()
            .filter(|(route, _)| !documented_console_routes.contains(route))
            .filter(|(_, interface)| {
                interface.interface_id
                    != crate::routes::host_infrastructure::interface_operation::HOST_INFRASTRUCTURE_PROVIDERS_VIEW_OPERATION_ID
            })
            .map(|(_, interface)| missing_openapi_entry(interface)),
    );

    let mut models = dependencies
        .store
        .list_model_definitions(workspace_id)
        .await?;
    models.retain(|model| model.status == domain::DataModelStatus::Published);
    models.sort_by(|left, right| left.code.cmp(&right.code));
    let operations =
        runtime_data_model_docs::build_category_operations(&models, &dependencies.template_catalog);
    for operation in operations.operations {
        let Ok(Some((model_id, operation_code))) =
            runtime_data_model_docs::parse_operation_id(&operation.id)
        else {
            continue;
        };
        let Some(model) = models.iter().find(|model| model.id == model_id) else {
            continue;
        };
        let Some(spec) = runtime_data_model_docs::build_operation_openapi(
            model,
            &operation_code,
            &dependencies.template_catalog,
        ) else {
            continue;
        };
        let Some(interface) = catalog_entry_from_operation(&operation, &spec) else {
            continue;
        };
        let source = if domain::builtin_contract_for_model(model).is_some() {
            OpenApiCapabilitySource::BuiltinDataModelCrud
        } else {
            OpenApiCapabilitySource::WorkspaceDataModelCrud
        };
        entries.push(OpenApiCapabilityCatalogEntry {
            risk_level: operation_risk_level(&interface.method),
            interface,
            source,
            bindable: true,
            disabled_reason: None,
            activated_operation: None,
        });
    }

    entries.sort_by(|left, right| {
        left.interface
            .operation_id
            .cmp(&right.interface.operation_id)
    });
    Ok(entries)
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
    path.starts_with("/api/console/") || path.starts_with(crate::routes::PUBLIC_API_PATH_PREFIX)
}

fn static_disabled_reason(
    operation: &DocsCatalogOperation,
    spec: &Value,
    interface: &OpenApiInterfaceCatalogEntry,
) -> Option<&'static str> {
    if !static_operation_is_bindable(&operation.path) {
        return Some("unsupported_interface_scope");
    }
    match interface.response_media_type.as_deref() {
        Some(media_type) if is_json_media_type(media_type) => None,
        Some(_) => Some("unsupported_response_media_type"),
        None if has_no_content_success(operation, spec) => None,
        None => Some("missing_openapi_contract"),
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

fn activated_providers_view_entry(
    registry: &interface_runtime::CompiledInterfaceRegistry,
    mut interface: OpenApiInterfaceCatalogEntry,
) -> Result<OpenApiCapabilityCatalogEntry, ApiError> {
    let definition =
        crate::routes::host_infrastructure::interface_operation::providers_view_definition(
            registry,
        )?;
    let route = registry
        .plan_for_interface(definition.interface_id())
        .and_then(|plan| plan.binding().projection().http_route())
        .ok_or_else(|| anyhow::anyhow!("activated interface operation has no route projection"))?;
    if !interface.method.eq_ignore_ascii_case(route.method()) || interface.path != route.path() {
        return Err(anyhow::anyhow!(
            "activated interface operation disagrees with the generated OpenAPI contract"
        )
        .into());
    }
    interface.operation_id = definition.interface_id().as_str().to_string();
    interface.method = route.method().to_string();
    interface.path = route.path().to_string();
    Ok(OpenApiCapabilityCatalogEntry {
        risk_level: operation_risk_level(&interface.method),
        interface,
        source: OpenApiCapabilitySource::ActivatedInterfaceOperation,
        bindable: true,
        disabled_reason: None,
        activated_operation: Some(ActivatedInterfaceOperationProjection {
            operation_id: definition.interface_id().as_str().to_string(),
            input_contract_id: definition.input_contract().contract_id().to_string(),
            input_contract_version: definition.input_contract().version().to_string(),
            output_contract_id: definition.output_contract().contract_id().to_string(),
            output_contract_version: definition.output_contract().version().to_string(),
            required_core_permission: definition.authorization_operation().as_str().to_string(),
            auth_policy: definition.authentication(),
            audit_policy: definition.audit(),
            error_policy: definition.error(),
            graph_fingerprint: registry.graph_fingerprint().as_str().to_string(),
            registry_fingerprint: registry.fingerprint().as_str().to_string(),
            owner: definition.owner().as_str().to_string(),
        }),
    })
}

fn activated_providers_view_route_matches(
    registry: &interface_runtime::CompiledInterfaceRegistry,
    interface: &OpenApiInterfaceCatalogEntry,
) -> bool {
    crate::routes::host_infrastructure::interface_operation::providers_view_definition(registry)
        .ok()
        .and_then(|definition| registry.plan_for_interface(definition.interface_id()))
        .and_then(|plan| plan.binding().projection().http_route())
        .is_some_and(|route| {
            interface.method.eq_ignore_ascii_case(route.method()) && interface.path == route.path()
        })
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
mod tests {
    use std::sync::Arc;

    use serde_json::json;

    use super::*;
    use crate::{
        extension_bus::{
            assemble_extension_graph_input, ExtensionBootSnapshot, DEFAULT_PLUGIN_SET_PATH,
        },
        routes::host_infrastructure::interface_operation::{
            HOST_INFRASTRUCTURE_PROVIDERS_VIEW_OPERATION_ID,
            HOST_INFRASTRUCTURE_PROVIDERS_VIEW_PATH,
        },
    };

    #[test]
    fn activated_projection_keeps_openapi_schema_and_uses_binding_identity() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let assembly =
            assemble_extension_graph_input(root, DEFAULT_PLUGIN_SET_PATH, Vec::new()).unwrap();
        let snapshot = ExtensionBootSnapshot::compile_for_test(
            Arc::new(assembly.compile_graph().unwrap()),
            assembly.interface_operations(),
        )
        .unwrap();
        let registry = snapshot.interface_registry().unwrap().snapshot();
        let request_schema = json!({"type": "object", "properties": {}});
        let response_schema = json!({"type": "array", "items": {"type": "object"}});
        let projected = activated_providers_view_entry(
            registry.as_ref(),
            OpenApiInterfaceCatalogEntry {
                operation_id: HOST_INFRASTRUCTURE_PROVIDERS_VIEW_OPERATION_ID.to_string(),
                method: "GET".to_string(),
                path: HOST_INFRASTRUCTURE_PROVIDERS_VIEW_PATH.to_string(),
                name: "providers".to_string(),
                description: "providers".to_string(),
                parameter_descriptors: Vec::new(),
                request_schema: request_schema.clone(),
                response_schema: response_schema.clone(),
                request_media_type: None,
                response_media_type: Some("application/json".to_string()),
                security: json!([{"cookie_auth": []}]),
            },
        )
        .unwrap();

        assert_eq!(
            projected.source,
            OpenApiCapabilitySource::ActivatedInterfaceOperation
        );
        assert_eq!(projected.interface.request_schema, request_schema);
        assert_eq!(projected.interface.response_schema, response_schema);
        let activated = projected.activated_operation.unwrap();
        let definition =
            crate::routes::host_infrastructure::interface_operation::providers_view_definition(
                registry.as_ref(),
            )
            .unwrap();
        assert_eq!(activated.operation_id, definition.interface_id().as_str());
        assert_eq!(
            activated.input_contract_id,
            definition.input_contract().contract_id()
        );
        assert_eq!(
            activated.output_contract_id,
            definition.output_contract().contract_id()
        );
        assert_eq!(activated.auth_policy, definition.authentication());
        assert_eq!(activated.audit_policy, definition.audit());
        assert_eq!(activated.error_policy, definition.error());
        assert_eq!(
            activated.graph_fingerprint,
            registry.graph_fingerprint().as_str()
        );
        assert_eq!(
            activated.registry_fingerprint,
            registry.fingerprint().as_str()
        );
        assert_eq!(activated.owner, definition.owner().as_str());
        assert_eq!(
            registry.graph_fingerprint().as_str(),
            snapshot.fingerprint()
        );
    }
}
