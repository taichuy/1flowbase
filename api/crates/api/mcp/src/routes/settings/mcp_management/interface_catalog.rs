use super::*;
use crate::openapi_interface::{OpenApiCapabilityIndexEntry, OpenApiCapabilitySource};

pub trait McpInterfaceRegistrySnapshotPort: Send + Sync + 'static {
    fn snapshot(&self) -> Result<Arc<interface_runtime::CompiledInterfaceRegistry>, ApiError>;
}

pub struct DynamicMcpInterfaceRegistrySnapshotPort {
    registry: std::sync::Weak<interface_runtime::DynamicInterfaceRegistry>,
}

impl DynamicMcpInterfaceRegistrySnapshotPort {
    pub fn new(registry: Arc<interface_runtime::DynamicInterfaceRegistry>) -> Self {
        Self {
            registry: Arc::downgrade(&registry),
        }
    }
}

impl McpInterfaceRegistrySnapshotPort for DynamicMcpInterfaceRegistrySnapshotPort {
    fn snapshot(&self) -> Result<Arc<interface_runtime::CompiledInterfaceRegistry>, ApiError> {
        Ok(self
            .registry
            .upgrade()
            .ok_or_else(crate::generation_dispatch::unavailable)?
            .snapshot())
    }
}

pub async fn mcp_interface_catalog_entries(
    state: &ApiState,
    actor: &domain::ActorContext,
) -> Result<Vec<domain::McpInterfaceCatalogEntry>, ApiError> {
    let dependencies = McpInterfaceCatalogDependencies {
        store: state.store.clone(),
        openapi: crate::openapi_interface::OpenApiCapabilityCatalogDependencies {
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
    };
    mcp_interface_catalog_entries_with(&dependencies, actor).await
}

pub use api_host_kernel::mcp_catalog::McpInterfaceCatalogDependencies;

pub async fn mcp_interface_catalog_entries_with(
    dependencies: &McpInterfaceCatalogDependencies,
    actor: &domain::ActorContext,
) -> Result<Vec<domain::McpInterfaceCatalogEntry>, ApiError> {
    let restricted = restricted_interfaces(dependencies, actor).await?;
    let entries = crate::openapi_interface::build_openapi_capability_catalog_with(
        &dependencies.openapi,
        actor.current_workspace_id,
    )
    .await?
    .into_iter()
    .filter(|entry| !restricted.contains(entry.interface.operation_id.as_str()))
    .map(mcp_interface_entry_from_capability)
    .collect::<Vec<_>>();
    Ok(entries)
}

async fn restricted_interfaces<'a>(
    dependencies: &'a McpInterfaceCatalogDependencies,
    actor: &domain::ActorContext,
) -> Result<std::collections::BTreeSet<&'a str>, ApiError> {
    use control_plane::ports::RoleRepository;
    if actor.current_workspace_id == dependencies.store.bootstrap_workspace_id().await? {
        return Ok(Default::default());
    }
    Ok(
        crate::openapi_interface::root_workspace_only_capability_ids(
            &dependencies.openapi.console_operations,
            &dependencies.openapi.api_docs,
        ),
    )
}

pub fn mcp_interface_entry_from_capability(
    entry: OpenApiCapabilityCatalogEntry,
) -> domain::McpInterfaceCatalogEntry {
    let source = match entry.source {
        OpenApiCapabilitySource::StaticApiDocs
        | OpenApiCapabilitySource::ActivatedInterfaceOperation => {
            domain::McpInterfaceCatalogSource::StaticApi
        }
        OpenApiCapabilitySource::BuiltinDataModelCrud => {
            domain::McpInterfaceCatalogSource::BuiltinDataModelCrud
        }
        OpenApiCapabilitySource::WorkspaceDataModelCrud => {
            domain::McpInterfaceCatalogSource::WorkspaceDataModelCrud
        }
        OpenApiCapabilitySource::PublishedWorkflow => {
            domain::McpInterfaceCatalogSource::PublishedWorkflow
        }
    };
    let permission_code = operation_permission_code(&entry.interface.method, &entry.interface.path);
    let interface = entry.interface;
    domain::McpInterfaceCatalogEntry {
        interface_id: interface.operation_id,
        source,
        method: interface.method.clone(),
        path: interface.path.clone(),
        name: interface.name,
        short_description: interface.description,
        parameter_descriptors: interface
            .parameter_descriptors
            .into_iter()
            .filter_map(|descriptor| {
                let parameter_type = match descriptor.location {
                    OpenApiParameterLocation::Path | OpenApiParameterLocation::Query => {
                        McpParameterType::Url
                    }
                    OpenApiParameterLocation::JsonBody => McpParameterType::JsonBody,
                    OpenApiParameterLocation::FormBody => McpParameterType::Form,
                    OpenApiParameterLocation::Header => return None,
                };
                Some(McpParameterDescriptor {
                    name: descriptor.name,
                    field_type: descriptor.field_type,
                    parameter_type,
                    description: descriptor.description,
                    required: descriptor.required,
                    schema: descriptor.schema,
                })
            })
            .collect(),
        parameter_schema: interface.request_schema,
        result_schema: interface.response_schema,
        permission_code,
        security: interface.security,
        risk_level: mcp_risk_level(entry.risk_level),
        bindable: entry.bindable,
        disabled_reason: entry.disabled_reason.map(str::to_string),
    }
}

#[allow(dead_code)]
pub(crate) async fn bindable_mcp_interface(
    state: &ApiState,
    actor: &domain::ActorContext,
    interface_id: &str,
) -> Result<domain::McpInterfaceCatalogEntry, ApiError> {
    let entry = mcp_interface_catalog_entries(state, actor)
        .await?
        .into_iter()
        .find(|entry| entry.interface_id == interface_id)
        .ok_or(control_plane::errors::ControlPlaneError::NotFound(
            "mcp_interface",
        ))?;

    if !entry.bindable {
        return Err(control_plane::errors::ControlPlaneError::InvalidInput("interface_id").into());
    }

    Ok(entry)
}

pub async fn bindable_mcp_interface_with(
    dependencies: &McpInterfaceCatalogDependencies,
    actor: &domain::ActorContext,
    interface_id: &str,
) -> Result<domain::McpInterfaceCatalogEntry, ApiError> {
    if restricted_interfaces(dependencies, actor)
        .await?
        .contains(interface_id)
    {
        return Err(control_plane::errors::ControlPlaneError::PermissionDenied(
            "permission_denied",
        )
        .into());
    }
    let entry = crate::openapi_interface::get_openapi_capability_with(
        &dependencies.openapi,
        actor.current_workspace_id,
        interface_id,
    )
    .await?
    .map(mcp_interface_entry_from_capability)
    .ok_or(control_plane::errors::ControlPlaneError::NotFound(
        "mcp_interface",
    ))?;
    if !entry.bindable {
        return Err(control_plane::errors::ControlPlaneError::InvalidInput("interface_id").into());
    }
    Ok(entry)
}

pub(super) async fn mcp_interface_operation_map_with(
    dependencies: &McpInterfaceCatalogDependencies,
    actor: &domain::ActorContext,
) -> Result<HashMap<String, String>, ApiError> {
    Ok(mcp_interface_index_with(dependencies, actor)
        .await?
        .into_iter()
        .map(|entry| {
            (
                entry.interface_id,
                format!("{} {}", entry.method, entry.path),
            )
        })
        .collect())
}

pub(super) async fn mcp_interface_index_with(
    dependencies: &McpInterfaceCatalogDependencies,
    actor: &domain::ActorContext,
) -> Result<Vec<OpenApiCapabilityIndexEntry>, ApiError> {
    let restricted = restricted_interfaces(dependencies, actor).await?;
    Ok(
        crate::openapi_interface::list_openapi_capability_index_with(
            &dependencies.openapi,
            actor.current_workspace_id,
        )
        .await?
        .into_iter()
        .filter(|entry| !restricted.contains(entry.interface_id.as_str()))
        .collect(),
    )
}

pub(super) async fn mcp_interface_operation_for_id_with(
    dependencies: &McpInterfaceCatalogDependencies,
    actor: &domain::ActorContext,
    interface_id: &str,
) -> Result<Option<String>, ApiError> {
    if restricted_interfaces(dependencies, actor)
        .await?
        .contains(interface_id)
    {
        return Ok(None);
    }
    Ok(
        crate::openapi_interface::get_openapi_capability_index_entry_with(
            &dependencies.openapi,
            actor.current_workspace_id,
            interface_id,
        )
        .await?
        .map(|entry| format!("{} {}", entry.method, entry.path)),
    )
}

pub(super) async fn mcp_interface_detail_with(
    dependencies: &McpInterfaceCatalogDependencies,
    actor: &domain::ActorContext,
    interface_id: &str,
) -> Result<domain::McpInterfaceCatalogEntry, ApiError> {
    let not_found = control_plane::errors::ControlPlaneError::NotFound("mcp_interface");
    if restricted_interfaces(dependencies, actor)
        .await?
        .contains(interface_id)
    {
        return Err(not_found.into());
    }
    Ok(crate::openapi_interface::get_openapi_capability_with(
        &dependencies.openapi,
        actor.current_workspace_id,
        interface_id,
    )
    .await?
    .map(mcp_interface_entry_from_capability)
    .ok_or(not_found)?)
}

pub(super) fn interface_operation(entry: &domain::McpInterfaceCatalogEntry) -> String {
    format!("{} {}", entry.method, entry.path)
}

#[cfg(test)]
pub(super) fn mcp_interface_entry_from_operation(
    operation: &crate::openapi_docs::DocsCatalogOperation,
    spec: &Value,
) -> Option<domain::McpInterfaceCatalogEntry> {
    let interface = crate::openapi_interface::catalog_entry_from_operation(operation, spec)?;
    Some(mcp_interface_entry_from_capability(
        OpenApiCapabilityCatalogEntry {
            risk_level: crate::openapi_interface::operation_risk_level(&interface.method),
            interface,
            source: OpenApiCapabilitySource::PublishedWorkflow,
            bindable: true,
            disabled_reason: None,
            activated_operation: None,
        },
    ))
}

pub(super) fn mcp_risk_level(risk_level: &str) -> domain::McpRiskLevel {
    match risk_level {
        "low" => domain::McpRiskLevel::Low,
        "medium" => domain::McpRiskLevel::Medium,
        "high" => domain::McpRiskLevel::High,
        "critical" => domain::McpRiskLevel::Critical,
        _ => unreachable!("shared OpenAPI capability catalog emitted an unknown risk level"),
    }
}

pub(super) fn permission_code(code: &str) -> Option<String> {
    Some(code.to_string())
}

pub(super) fn read_or_manage_permission(method: &str, resource: &str) -> Option<String> {
    let action = match method {
        "GET" | "HEAD" | "OPTIONS" => "view",
        _ => "manage",
    };
    permission_code(&format!("{resource}.{action}.all"))
}

pub(super) fn view_or_configure_permission(method: &str, resource: &str) -> Option<String> {
    let action = match method {
        "GET" | "HEAD" | "OPTIONS" => "view",
        _ => "configure",
    };
    permission_code(&format!("{resource}.{action}.all"))
}

pub(super) fn application_permission(method: &str, path: &str) -> Option<String> {
    match method {
        "GET" | "HEAD" | "OPTIONS" => permission_code("application.view.all"),
        "POST" if path == "/api/console/applications" => permission_code("application.create.all"),
        "DELETE" => permission_code("application.delete.all"),
        "POST" if path.contains("/actions/") || path.contains("/runs/") => {
            permission_code("application.use.all")
        }
        _ => permission_code("application.edit.all"),
    }
}

pub(super) fn file_table_permission(method: &str, path: &str) -> Option<String> {
    match method {
        "GET" | "HEAD" | "OPTIONS" => permission_code("file_table.view.all"),
        "POST" if path == "/api/console/file-tables" => permission_code("file_table.create.all"),
        "DELETE" => permission_code("file_table.delete.all"),
        "PUT" if path.ends_with("/binding") => permission_code("file_table.bind.all"),
        _ => permission_code("file_table.bind.all"),
    }
}

pub(super) fn state_model_permission(method: &str, path: &str) -> Option<String> {
    match method {
        "GET" | "HEAD" | "OPTIONS" => permission_code("state_model.view.all"),
        "POST" if path == "/api/console/models" => permission_code("state_model.create.all"),
        "POST" if path == "/api/console/models:batchDelete" => {
            permission_code("state_model.delete.all")
        }
        "DELETE" => permission_code("state_model.delete.all"),
        _ => permission_code("state_model.edit.all"),
    }
}

pub(super) fn external_data_source_permission(method: &str, path: &str) -> Option<String> {
    match method {
        "GET" | "HEAD" | "OPTIONS" => permission_code("external_data_source.view.all"),
        "POST" if path == "/api/console/data-sources" => {
            permission_code("external_data_source.create.all")
        }
        "DELETE" => permission_code("external_data_source.delete.all"),
        _ => permission_code("external_data_source.configure.all"),
    }
}

pub(super) fn operation_permission_code(method: &str, path: &str) -> Option<String> {
    if path.starts_with("/api/console/settings/members") {
        return permission_code(access_control::SYSTEM_MEMBERS_SETTINGS_FEATURE_PERMISSION);
    }
    if path.starts_with("/api/console/settings/roles") {
        return permission_code(access_control::SYSTEM_ROLES_SETTINGS_FEATURE_PERMISSION);
    }
    if path.starts_with("/api/console/docs/") {
        return permission_code(access_control::SYSTEM_DOCS_SETTINGS_FEATURE_PERMISSION);
    }
    if path.starts_with("/api/console/user-api-keys") {
        return permission_code(
            access_control::SYSTEM_API_KEY_AUTHENTICATION_SETTINGS_FEATURE_PERMISSION,
        );
    }
    if path.starts_with("/api/console/system/runtime-profile")
        || path.starts_with("/api/console/system/release-status")
    {
        return permission_code(access_control::SYSTEM_SYSTEM_RUNTIME_SETTINGS_FEATURE_PERMISSION);
    }
    if path.starts_with("/api/console/workspace") || path.starts_with("/api/console/workspaces") {
        return view_or_configure_permission(method, "workspace");
    }
    if path.starts_with("/api/console/mcp/") {
        return permission_code(access_control::SYSTEM_MCP_MANAGEMENT_SETTINGS_FEATURE_PERMISSION);
    }
    if path.starts_with("/api/console/file-storages") {
        return read_or_manage_permission(method, "file_storage");
    }
    if path.starts_with("/api/console/file-tables") {
        return file_table_permission(method, path);
    }
    if path.starts_with("/api/console/model-providers")
        || path.starts_with("/api/console/plugins")
        || path.starts_with("/api/console/host-infrastructure")
    {
        return view_or_configure_permission(method, "plugin_config");
    }
    if path.starts_with("/api/console/data-sources") {
        return external_data_source_permission(method, path);
    }
    if path.starts_with("/api/console/models") {
        return state_model_permission(method, path);
    }
    if path.starts_with("/api/console/applications") {
        return application_permission(method, path);
    }
    if path.starts_with("/api/console/node-contributions")
        || path.starts_with("/api/console/frontend-blocks")
        || path.starts_with("/api/console/js-dependencies")
    {
        return permission_code("plugin_config.view.all");
    }

    None
}
