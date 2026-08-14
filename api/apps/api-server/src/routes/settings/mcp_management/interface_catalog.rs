use super::*;
use crate::openapi_interface::OpenApiCapabilitySource;

pub(super) async fn mcp_interface_catalog_entries(
    state: &ApiState,
    actor: &domain::ActorContext,
) -> Result<Vec<domain::McpInterfaceCatalogEntry>, ApiError> {
    let mut entries = build_openapi_capability_catalog(state, actor.current_workspace_id)
        .await?
        .into_iter()
        .map(mcp_interface_entry_from_capability)
        .collect::<Vec<_>>();
    let publications = state.store.list_enabled_extension_publications().await?;
    let operations = build_published_workflow_operations(publications)
        .map_err(|_| control_plane::errors::ControlPlaneError::Conflict("workflow_route"))?;
    for operation in operations
        .into_iter()
        .filter(|operation| operation.workspace_id == actor.current_workspace_id)
    {
        let path = operation.public_path();
        let method = operation.method.as_str().to_string();
        let docs_operation = DocsCatalogOperation {
            id: operation.interface_id.clone(),
            method: method.clone(),
            path: path.clone(),
            summary: Some(format!(
                "Invoke published workflow {}",
                operation.application_id
            )),
            description: Some("Invoke the active publication of a Workflow application".into()),
            tags: vec!["Workflow Extensions".into()],
            group: "workflow_extensions".into(),
            deprecated: false,
        };
        let spec = serde_json::json!({
            "openapi": "3.1.0",
            "paths": {
                (path): {
                    (method.to_ascii_lowercase()): crate::openapi::workflow_extension_operation(&operation)
                }
            }
        });
        if let Some(entry) = mcp_interface_entry_from_operation(&docs_operation, &spec) {
            entries.push(entry);
        }
    }
    Ok(entries)
}

pub(super) fn mcp_interface_entry_from_capability(
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
    };
    let permission_code = if entry.activated_operation.is_some() {
        Some(access_control::SYSTEM_HOST_INFRASTRUCTURE_SETTINGS_FEATURE_PERMISSION.to_string())
    } else {
        operation_permission_code(&entry.interface.method, &entry.interface.path)
    };
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

pub(super) async fn mcp_interface_operation_map(
    state: &ApiState,
    actor: &domain::ActorContext,
) -> Result<HashMap<String, String>, ApiError> {
    Ok(mcp_interface_catalog_entries(state, actor)
        .await?
        .into_iter()
        .map(|entry| {
            let operation = interface_operation(&entry);
            (entry.interface_id, operation)
        })
        .collect())
}

pub(super) fn interface_operation(entry: &domain::McpInterfaceCatalogEntry) -> String {
    format!("{} {}", entry.method, entry.path)
}

pub(super) fn mcp_interface_entry_from_operation(
    operation: &DocsCatalogOperation,
    spec: &Value,
) -> Option<domain::McpInterfaceCatalogEntry> {
    let interface = crate::openapi_interface::catalog_entry_from_operation(operation, spec)?;

    Some(domain::McpInterfaceCatalogEntry {
        interface_id: interface.operation_id,
        source: domain::McpInterfaceCatalogSource::PublishedWorkflow,
        method: interface.method,
        path: interface.path,
        name: interface.name,
        short_description: interface.description,
        parameter_descriptors: interface
            .parameter_descriptors
            .into_iter()
            .filter_map(|descriptor| {
                use crate::openapi_interface::OpenApiParameterLocation;
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
        permission_code: operation_permission_code(&operation.method, &operation.path),
        security: interface.security,
        risk_level: operation_risk_level(&operation.method),
        bindable: true,
        disabled_reason: None,
    })
}

pub(super) fn operation_risk_level(method: &str) -> domain::McpRiskLevel {
    mcp_risk_level(crate::openapi_interface::operation_risk_level(method))
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

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use serde_json::json;

    use super::*;
    use crate::{
        extension_bus::{
            assemble_extension_graph_input, ExtensionBootSnapshot, DEFAULT_PLUGIN_SET_PATH,
        },
        openapi_interface::{ActivatedInterfaceOperationProjection, OpenApiInterfaceCatalogEntry},
    };

    #[test]
    fn activated_openapi_projection_becomes_the_same_mcp_interface_contract() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let assembly =
            assemble_extension_graph_input(root, DEFAULT_PLUGIN_SET_PATH, Vec::new()).unwrap();
        let snapshot = ExtensionBootSnapshot::compile(
            Arc::new(assembly.compile_graph().unwrap()),
            assembly.interface_operations(),
        )
        .unwrap();
        let binding = snapshot.interface_operations().unwrap().providers_view();
        let descriptor = binding.definition().descriptor();
        let parameter_schema = json!({"type": "object", "properties": {}});
        let result_schema = json!({"type": "array", "items": {"type": "object"}});
        let entry = OpenApiCapabilityCatalogEntry {
            interface: OpenApiInterfaceCatalogEntry {
                operation_id: descriptor.operation_id.clone(),
                method: descriptor.method.as_str().to_string(),
                path: descriptor.path.clone(),
                name: "providers".to_string(),
                description: "providers".to_string(),
                parameter_descriptors: Vec::new(),
                request_schema: parameter_schema.clone(),
                response_schema: result_schema.clone(),
                request_media_type: None,
                response_media_type: Some("application/json".to_string()),
                security: json!([{"cookie_auth": []}]),
            },
            source: OpenApiCapabilitySource::ActivatedInterfaceOperation,
            risk_level: "low",
            bindable: true,
            disabled_reason: None,
            activated_operation: Some(ActivatedInterfaceOperationProjection {
                operation_id: descriptor.operation_id.clone(),
                input_contract_id: descriptor.input.contract_id.clone(),
                input_contract_version: descriptor.input.contract_version.clone(),
                output_contract_id: descriptor.output.contract_id.clone(),
                output_contract_version: descriptor.output.contract_version.clone(),
                required_core_permission: descriptor.required_core_permission.clone(),
                auth_policy: descriptor.auth_policy,
                audit_policy: descriptor.audit_policy,
                error_policy: descriptor.error_policy,
                graph_fingerprint: binding.graph_fingerprint().to_string(),
                provenance: binding.provenance().clone(),
            }),
        };

        let mcp = mcp_interface_entry_from_capability(entry);
        assert_eq!(mcp.interface_id, descriptor.operation_id);
        assert_eq!(mcp.method, descriptor.method.as_str());
        assert_eq!(mcp.path, descriptor.path);
        assert_eq!(mcp.parameter_schema, parameter_schema);
        assert_eq!(mcp.result_schema, result_schema);
        assert_eq!(
            mcp.permission_code.as_deref(),
            Some(access_control::SYSTEM_HOST_INFRASTRUCTURE_SETTINGS_FEATURE_PERMISSION)
        );
        assert_eq!(mcp.source, domain::McpInterfaceCatalogSource::StaticApi);
    }
}
