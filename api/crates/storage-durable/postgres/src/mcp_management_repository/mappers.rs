use anyhow::Result;
use control_plane::errors::ControlPlaneError;
use sqlx::Row;
use uuid::Uuid;

pub(super) fn map_mcp_instance_insert_error(error: sqlx::Error) -> anyhow::Error {
    if let sqlx::Error::Database(database_error) = &error {
        if database_error.constraint() == Some("mcp_instances_workspace_instance_id_idx") {
            return ControlPlaneError::Conflict("mcp_instance_id").into();
        }
    }
    error.into()
}

pub(super) fn parse_instance_status(value: &str) -> Result<domain::McpInstanceStatus> {
    match value {
        "draft" => Ok(domain::McpInstanceStatus::Draft),
        "enabled" => Ok(domain::McpInstanceStatus::Enabled),
        "disabled" => Ok(domain::McpInstanceStatus::Disabled),
        "archived" => Ok(domain::McpInstanceStatus::Archived),
        _ => anyhow::bail!("invalid MCP instance status"),
    }
}

pub(super) fn parse_tool_status(value: &str) -> Result<domain::McpToolStatus> {
    match value {
        "draft" => Ok(domain::McpToolStatus::Draft),
        "enabled" => Ok(domain::McpToolStatus::Enabled),
        "disabled" => Ok(domain::McpToolStatus::Disabled),
        "archived" => Ok(domain::McpToolStatus::Archived),
        _ => anyhow::bail!("invalid MCP tool status"),
    }
}

pub(super) fn parse_risk_level(value: &str) -> Result<domain::McpRiskLevel> {
    match value {
        "low" => Ok(domain::McpRiskLevel::Low),
        "medium" => Ok(domain::McpRiskLevel::Medium),
        "high" => Ok(domain::McpRiskLevel::High),
        "critical" => Ok(domain::McpRiskLevel::Critical),
        _ => anyhow::bail!("invalid MCP risk level"),
    }
}

pub(super) fn parse_upstream_transport(value: &str) -> Result<domain::McpUpstreamTransport> {
    match value {
        "streamable_http" => Ok(domain::McpUpstreamTransport::StreamableHttp),
        _ => anyhow::bail!("invalid MCP upstream transport"),
    }
}

pub(super) fn parse_upstream_auth_type(value: &str) -> Result<domain::McpUpstreamAuthType> {
    match value {
        "none" => Ok(domain::McpUpstreamAuthType::None),
        "bearer" => Ok(domain::McpUpstreamAuthType::Bearer),
        "custom_header" => Ok(domain::McpUpstreamAuthType::CustomHeader),
        _ => anyhow::bail!("invalid MCP upstream auth type"),
    }
}

pub(super) fn parse_upstream_connection_status(
    value: &str,
) -> Result<domain::McpUpstreamConnectionStatus> {
    match value {
        "enabled" => Ok(domain::McpUpstreamConnectionStatus::Enabled),
        "disabled" => Ok(domain::McpUpstreamConnectionStatus::Disabled),
        _ => anyhow::bail!("invalid MCP upstream connection status"),
    }
}

pub(super) fn parse_upstream_source_status(value: &str) -> Result<domain::McpUpstreamSourceStatus> {
    match value {
        "not_imported" => Ok(domain::McpUpstreamSourceStatus::NotImported),
        "imported" => Ok(domain::McpUpstreamSourceStatus::Imported),
        "definition_changed" => Ok(domain::McpUpstreamSourceStatus::DefinitionChanged),
        "remote_missing" => Ok(domain::McpUpstreamSourceStatus::RemoteMissing),
        _ => anyhow::bail!("invalid MCP upstream source status"),
    }
}

pub(super) fn execution_target_kind(target: &domain::McpToolExecutionTarget) -> &'static str {
    match target {
        domain::McpToolExecutionTarget::InterfaceWrapper { .. } => "interface_wrapper",
        domain::McpToolExecutionTarget::McpProxy { .. } => "mcp_proxy",
    }
}

pub(super) fn execution_target_upstream_connection_id(
    target: &domain::McpToolExecutionTarget,
) -> Option<Uuid> {
    match target {
        domain::McpToolExecutionTarget::McpProxy {
            upstream_connection_id,
            ..
        } => Some(*upstream_connection_id),
        domain::McpToolExecutionTarget::InterfaceWrapper { .. } => None,
    }
}

pub(super) fn execution_target_remote_tool_name(
    target: &domain::McpToolExecutionTarget,
) -> Option<&str> {
    match target {
        domain::McpToolExecutionTarget::McpProxy {
            remote_tool_name, ..
        } => Some(remote_tool_name),
        domain::McpToolExecutionTarget::InterfaceWrapper { .. } => None,
    }
}

pub(super) fn execution_target_source_schema_hash(
    target: &domain::McpToolExecutionTarget,
) -> Option<&str> {
    match target {
        domain::McpToolExecutionTarget::McpProxy {
            source_schema_hash, ..
        } => Some(source_schema_hash),
        domain::McpToolExecutionTarget::InterfaceWrapper { .. } => None,
    }
}

pub(super) fn map_instance(row: sqlx::postgres::PgRow) -> Result<domain::McpInstanceRecord> {
    Ok(domain::McpInstanceRecord {
        id: row.get("id"),
        workspace_id: row.get("workspace_id"),
        instance_id: row.get("instance_id"),
        name: row.get("name"),
        description_short: row.get("description_short"),
        status: parse_instance_status(row.get::<String, _>("status").as_str())?,
        default_entry_path: row.get("default_entry_path"),
        created_by: row.get("created_by"),
        updated_by: row.get("updated_by"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

pub(super) fn map_group(row: sqlx::postgres::PgRow) -> Result<domain::McpGroupRecord> {
    Ok(domain::McpGroupRecord {
        id: row.get("id"),
        instance_record_id: row.get("instance_record_id"),
        path: row.get("path"),
        display_name: row.get("display_name"),
        description_short: row.get("description_short"),
        enabled: row.get("enabled"),
        sort_order: row.get("sort_order"),
        created_by: row.get("created_by"),
        updated_by: row.get("updated_by"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

pub(super) fn map_tool(row: sqlx::postgres::PgRow) -> Result<domain::McpToolRecord> {
    let execution_target = match row.get::<String, _>("execution_kind").as_str() {
        "interface_wrapper" => domain::McpToolExecutionTarget::InterfaceWrapper {
            interface_id: row.get("interface_id"),
        },
        "mcp_proxy" => domain::McpToolExecutionTarget::McpProxy {
            upstream_connection_id: row.get("upstream_connection_id"),
            remote_tool_name: row.get("remote_tool_name"),
            source_schema_hash: row.get("source_schema_hash"),
        },
        _ => anyhow::bail!("invalid MCP tool execution kind"),
    };
    Ok(domain::McpToolRecord {
        id: row.get("id"),
        workspace_id: row.get("workspace_id"),
        tool_id: row.get("tool_id"),
        name: row.get("name"),
        short_description: row.get("short_description"),
        full_description: row.get("full_description"),
        execution_target,
        parameter_schema: row.get("parameter_schema"),
        result_schema: row.get("result_schema"),
        input_mapping: row.get("input_mapping"),
        output_mapping: row.get("output_mapping"),
        permission_code: row.get("permission_code"),
        risk_level: parse_risk_level(row.get::<String, _>("risk_level").as_str())?,
        des_id: row.get("des_id"),
        des_id_required: row.get("des_id_required"),
        status: parse_tool_status(row.get::<String, _>("status").as_str())?,
        revision: row.get("revision"),
        created_by: row.get("created_by"),
        updated_by: row.get("updated_by"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

pub(super) fn map_binding(row: sqlx::postgres::PgRow) -> Result<domain::McpToolBindingRecord> {
    Ok(domain::McpToolBindingRecord {
        id: row.get("id"),
        instance_record_id: row.get("instance_record_id"),
        tool_record_id: row.get("tool_record_id"),
        group_path: row.get("group_path"),
        tool_id: row.get("tool_id"),
        display_alias: row.get("display_alias"),
        visible: row.get("visible"),
        sort_order: row.get("sort_order"),
        created_by: row.get("created_by"),
        updated_by: row.get("updated_by"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

pub(super) fn map_upstream_connection(
    row: sqlx::postgres::PgRow,
) -> Result<domain::McpUpstreamConnectionRecord> {
    Ok(domain::McpUpstreamConnectionRecord {
        id: row.get("id"),
        workspace_id: row.get("workspace_id"),
        name: row.get("name"),
        endpoint: row.get("endpoint"),
        transport: parse_upstream_transport(row.get::<String, _>("transport").as_str())?,
        auth_type: parse_upstream_auth_type(row.get::<String, _>("auth_type").as_str())?,
        custom_header_name: row.get("custom_header_name"),
        status: parse_upstream_connection_status(row.get::<String, _>("status").as_str())?,
        credentials_configured: row.get("credentials_configured"),
        last_connected_at: row.get("last_connected_at"),
        last_discovered_at: row.get("last_discovered_at"),
        last_error: row.get("last_error"),
        created_by: row.get("created_by"),
        updated_by: row.get("updated_by"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

pub(super) fn map_upstream_source(
    row: sqlx::postgres::PgRow,
) -> Result<domain::McpUpstreamToolSourceRecord> {
    Ok(domain::McpUpstreamToolSourceRecord {
        id: row.get("id"),
        workspace_id: row.get("workspace_id"),
        upstream_connection_id: row.get("upstream_connection_id"),
        remote_tool_name: row.get("remote_tool_name"),
        description: row.get("description"),
        input_schema: row.get("input_schema"),
        output_schema: row.get("output_schema"),
        schema_hash: row.get("schema_hash"),
        source_status: parse_upstream_source_status(
            row.get::<String, _>("source_status").as_str(),
        )?,
        imported_tool_id: row.get("imported_tool_id"),
        discovered_at: row.get("discovered_at"),
    })
}

pub(super) fn map_instance_discovery_policy(
    row: sqlx::postgres::PgRow,
) -> Result<domain::McpInstanceDiscoveryPolicyRecord> {
    Ok(domain::McpInstanceDiscoveryPolicyRecord {
        id: row.get("id"),
        workspace_id: row.get("workspace_id"),
        instance_record_id: row.get("instance_record_id"),
        list_default_limit: row.get("list_default_limit"),
        list_max_depth: row.get("list_max_depth"),
        list_regex_enabled: row.get("list_regex_enabled"),
        list_regex_max_length: row.get("list_regex_max_length"),
        list_return_fields: row.get("list_return_fields"),
        created_by: row.get("created_by"),
        updated_by: row.get("updated_by"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}
