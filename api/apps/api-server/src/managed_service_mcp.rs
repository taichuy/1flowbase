//! Read-only MCP projection of package-owned fixed operations. No workspace configuration writes.
use crate::{
    managed_services::ManagedServiceRegistration,
    routes::mcp_protocol::virtual_ui::McpInterfaceCatalogPort,
};
use anyhow::{bail, Result};
use control_plane_contracts::ports::{PluginRepository, RoleConsolePolicyReader};
use domain::{
    McpCatalogSnapshot, McpGroupRecord, McpInstanceRecord, McpInstanceStatus, McpRiskLevel,
    McpToolBindingRecord, McpToolRecord, McpToolStatus,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use storage_durable_postgres::MainDurableStore;
use uuid::Uuid;

pub(crate) async fn append(
    store: &MainDurableStore,
    services: &[ManagedServiceRegistration],
    actor: &domain::ActorContext,
    catalog: &mut McpCatalogSnapshot,
    selected_instances: &[String],
    interfaces: &dyn McpInterfaceCatalogPort,
) -> Result<()> {
    if services.is_empty() {
        return Ok(());
    }
    // A call operates on one selected MCP instance. Never grant browser capability exposure.
    let Some(instance) = selected_instance(catalog, selected_instances).cloned() else {
        return Ok(());
    };
    let policies = if actor.is_root {
        Vec::new()
    } else {
        store.load_role_console_policies_for_user(actor).await?
    };
    let mut additions = catalog.clone();
    for service in services {
        if !service
            .declaration
            .operations
            .iter()
            .any(|operation| operation.mcp.is_some())
        {
            continue;
        }
        let Some(current) = store.get_installation(service.installation_id).await? else {
            continue;
        };
        if !current_matches(service, &current) {
            continue;
        }
        let mut operations = Vec::new();
        for operation in &service.declaration.operations {
            if operation.mcp.is_none()
                || interfaces
                    .bindable_interface(&operation.interface_id)
                    .is_err()
            {
                continue;
            }
            if operation_accessible(actor, &policies, service, operation)? {
                operations.push(operation);
            }
        }
        append_service(
            &mut additions,
            &instance,
            service,
            &operations,
            actor.user_id,
        )?;
    }
    *catalog = additions;
    Ok(())
}
fn selected_instance<'a>(
    catalog: &'a McpCatalogSnapshot,
    selected: &[String],
) -> Option<&'a McpInstanceRecord> {
    selected.iter().find_map(|id| {
        catalog.instances.iter().find(|i| {
            &i.instance_id == id
                && i.instance_id != "frontstage_browser"
                && i.status == McpInstanceStatus::Enabled
        })
    })
}
fn operation_accessible(
    actor: &domain::ActorContext,
    policies: &[domain::RoleConsolePolicy],
    service: &ManagedServiceRegistration,
    operation: &plugin_framework::ManagedServiceOperation,
) -> Result<bool> {
    let group =
        domain::ConsolePolicyGroup::settings_feature(&service.declaration.feature.feature_id)?;
    let operation_id = domain::ConsoleOperationId::try_from(operation.interface_id.as_str())?;
    Ok(
        actor.is_root
            || domain::effective_console_simple_operation(policies, &group, &operation_id),
    )
}

fn current_matches(
    service: &ManagedServiceRegistration,
    current: &domain::PluginInstallationRecord,
) -> bool {
    current.id == service.installation_id
        && current.desired_state == domain::PluginDesiredState::ActiveRequested
        && current.provider_code == service.plugin_code
        && current.plugin_version == service.plugin_version
        && current.contract_version == "1flowbase.extension-bus/v1"
        && domain::managed_installation_scope(current, domain::DEFAULT_SCOPE_ID)
            == domain::SYSTEM_SCOPE_ID
        && serde_json::to_value(&service.declaration).ok().as_ref()
            == current.metadata_json.get("managed_service")
}
fn identity(kind: &str, owner: &str, key: &str) -> Uuid {
    let bytes = serde_json::to_vec(&("managed_service_mcp/v1", kind, owner, key))
        .expect("string tuple serializes");
    let digest = Sha256::digest(bytes);
    let mut id = [0; 16];
    id.copy_from_slice(&digest[..16]);
    Uuid::from_bytes(id)
}
pub(crate) fn append_service(
    catalog: &mut McpCatalogSnapshot,
    instance: &McpInstanceRecord,
    service: &ManagedServiceRegistration,
    operations: &[&plugin_framework::ManagedServiceOperation],
    actor: Uuid,
) -> Result<()> {
    if operations.is_empty() {
        return Ok(());
    }
    let now = time::OffsetDateTime::now_utc();
    let path = format!("/plugins/{}", service.plugin_code);
    let root_id = identity("group", &instance.id.to_string(), "/plugins");
    if catalog
        .groups
        .iter()
        .any(|g| g.instance_record_id == instance.id && g.path == "/plugins" && g.id != root_id)
    {
        bail!("managed_mcp_group_collision");
    }
    if !catalog.groups.iter().any(|g| g.id == root_id) {
        catalog.groups.push(McpGroupRecord {
            id: root_id,
            instance_record_id: instance.id,
            path: "/plugins".into(),
            display_name: "Plugins".into(),
            description_short: Some("Package-owned operations".into()),
            enabled: true,
            sort_order: 0,
            created_by: actor,
            updated_by: actor,
            created_at: now,
            updated_at: now,
        });
    }
    if catalog
        .groups
        .iter()
        .any(|g| g.instance_record_id == instance.id && g.path == path)
    {
        bail!("managed_mcp_group_collision");
    }
    catalog.groups.push(McpGroupRecord {
        id: identity("group", &instance.id.to_string(), &path),
        instance_record_id: instance.id,
        path: path.clone(),
        display_name: service.declaration.feature.label.clone(),
        description_short: Some(service.declaration.feature.description.clone()),
        enabled: true,
        sort_order: 0,
        created_by: actor,
        updated_by: actor,
        created_at: now,
        updated_at: now,
    });
    for operation in operations {
        let mcp = operation
            .mcp
            .as_ref()
            .expect("only declared operations are projected");
        let tool_id = format!("plugin.{}.{}", service.plugin_code, mcp.name);
        let id = identity("tool", &service.plugin_code, &mcp.name);
        if catalog
            .tools
            .iter()
            .any(|tool| tool.tool_id == tool_id || tool.id == id)
            || catalog
                .bindings
                .iter()
                .any(|b| b.instance_record_id == instance.id && b.tool_id == tool_id)
        {
            bail!("managed_mcp_tool_collision");
        }
        let digest = Sha256::digest(serde_json::to_vec(&(operation, &service.plugin_version))?);
        catalog.tools.push(McpToolRecord {
            id,
            workspace_id: instance.workspace_id,
            tool_id: tool_id.clone(),
            name: mcp.name.clone(),
            short_description: mcp.description.clone(),
            full_description: mcp.description.clone(),
            execution_target: domain::McpToolExecutionTarget::InterfaceWrapper {
                interface_id: operation.interface_id.clone(),
            },
            parameter_schema: parameter_schema(&operation.input_schema),
            result_schema: media_result_schema(
                &operation.output_schema,
                mcp.output_mapping.as_ref(),
            ),
            input_mapping: input_mapping(&operation.input_schema),
            output_mapping: mcp
                .output_mapping
                .clone()
                .unwrap_or_else(|| operation.output_schema.clone()),
            max_inline_chars: None,
            response_fields: None,
            permission_code: None,
            risk_level: if operation.method == "GET" {
                McpRiskLevel::Low
            } else {
                McpRiskLevel::High
            },
            des_id: format!("managed:{:x}", base16ct::HexDisplay(&digest)),
            des_id_required: false,
            status: McpToolStatus::Enabled,
            revision: 1,
            managed_by: None,
            created_by: actor,
            updated_by: actor,
            created_at: now,
            updated_at: now,
        });
        catalog.bindings.push(McpToolBindingRecord {
            id: identity("binding", &instance.id.to_string(), &tool_id),
            instance_record_id: instance.id,
            tool_record_id: id,
            group_path: path.clone(),
            tool_id,
            display_alias: None,
            visible: true,
            sort_order: 0,
            created_by: actor,
            updated_by: actor,
            created_at: now,
            updated_at: now,
        });
    }
    Ok(())
}
// Managed MCP arguments share DTO field names, without HTTP transport containers.
fn parameter_schema(schema: &Value) -> Value {
    let mut properties = serde_json::Map::new();
    let mut required = Vec::new();
    for location in ["path", "query", "body"] {
        let Some(location_schema) = schema.pointer(&format!("/properties/{location}")) else {
            continue;
        };
        let Some(fields) = location_schema.get("properties").and_then(Value::as_object) else {
            continue;
        };
        for (name, field_schema) in fields {
            // Intake rejects duplicate field names across locations.
            properties.insert(name.clone(), field_schema.clone());
            if location == "path"
                || location_schema
                    .get("required")
                    .and_then(Value::as_array)
                    .is_some_and(|items| items.iter().any(|value| value.as_str() == Some(name)))
            {
                required.push(Value::String(name.clone()));
            }
        }
    }
    json!({"type":"object", "properties":properties, "required":required, "additionalProperties":false})
}

fn input_mapping(schema: &Value) -> Value {
    let mut mappings = Vec::new();
    for location in ["path", "query", "body"] {
        let Some(location_schema) = schema.pointer(&format!("/properties/{location}")) else {
            continue;
        };
        let Some(properties) = location_schema.get("properties").and_then(Value::as_object) else {
            continue;
        };
        for (name, property) in properties {
            let required = location == "path"
                || location_schema
                    .get("required")
                    .and_then(Value::as_array)
                    .is_some_and(|items| items.iter().any(|v| v.as_str() == Some(name)));
            let path = name;
            let mut mapping = json!({"interface_param":name,"mcp_param":path,"source":{"kind":"mcp_argument","path":path},"required":required,"description":property.get("description").and_then(Value::as_str).unwrap_or("")});
            if let Some(default) = property.get("default") {
                mapping["default_value"] = default.clone();
            }
            mappings.push(mapping);
        }
    }
    json!({"mappings":mappings})
}

#[cfg(test)]
#[path = "_tests/mcp_routes/managed_service_mcp.rs"]
mod tests;

// Project only a real declared metadata subschema; unresolved schemas remain unconstrained.
fn media_result_schema(schema: &Value, mapping: Option<&Value>) -> Value {
    let Some(mapping) = mapping else {
        return schema.clone();
    };
    let Ok(mapping) = plugin_framework::ManagedMcpMediaMapping::parse(mapping) else {
        return json!({});
    };
    let mut current = schema;
    for segment in mapping.structured_content_path.split('/').skip(1) {
        let name = segment.replace("~1", "/").replace("~0", "~");
        let next = if current.get("type").and_then(Value::as_str) == Some("array") {
            name.parse::<usize>()
                .ok()
                .and_then(|_| current.get("items"))
        } else {
            current
                .get("properties")
                .and_then(|properties| properties.get(&name))
        };
        let Some(next) = next else {
            return json!({});
        };
        current = next;
    }
    current.clone()
}
