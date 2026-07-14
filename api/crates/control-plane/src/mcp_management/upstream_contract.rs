use anyhow::Result;
use uuid::Uuid;

use crate::errors::ControlPlaneError;

pub struct UpdateMcpProxyToolCommand {
    pub actor_user_id: Uuid,
    pub tool_id: String,
    pub des_id: Option<String>,
    pub name: String,
    pub short_description: String,
    pub full_description: String,
    pub execution_target: domain::McpToolExecutionTarget,
    pub parameter_schema: serde_json::Value,
    pub result_schema: serde_json::Value,
    pub input_mapping: serde_json::Value,
    pub output_mapping: serde_json::Value,
    pub risk_level: domain::McpRiskLevel,
    pub status: domain::McpToolStatus,
}

pub struct SaveMcpUpstreamConnectionCommand {
    pub actor_user_id: Uuid,
    pub connection_id: Option<Uuid>,
    pub name: String,
    pub endpoint: String,
    pub transport: domain::McpUpstreamTransport,
    pub auth_type: domain::McpUpstreamAuthType,
    pub custom_header_name: Option<String>,
    pub status: domain::McpUpstreamConnectionStatus,
}

pub enum McpUpstreamCredential {
    Bearer {
        token: String,
    },
    CustomHeader {
        header_name: String,
        header_value: String,
    },
}

pub struct SaveMcpUpstreamCredentialCommand {
    pub actor_user_id: Uuid,
    pub connection_id: Uuid,
    pub credential: McpUpstreamCredential,
    pub master_key: String,
}

#[derive(Debug, Clone)]
pub struct McpRemoteToolDefinition {
    pub remote_tool_name: String,
    pub description: Option<String>,
    pub input_schema: serde_json::Value,
    pub output_schema: serde_json::Value,
    pub schema_hash: String,
}

pub struct RecordMcpUpstreamDiscoveryCommand {
    pub actor_user_id: Uuid,
    pub connection_id: Uuid,
    pub discovered_at: time::OffsetDateTime,
    pub tools: Vec<McpRemoteToolDefinition>,
}

pub(super) fn validate_upstream_endpoint(endpoint: &str) -> Result<()> {
    if !endpoint.starts_with("https://") || endpoint.len() > 2048 {
        return Err(ControlPlaneError::InvalidInput("endpoint").into());
    }
    Ok(())
}

pub(super) fn validate_upstream_header_name(
    auth_type: domain::McpUpstreamAuthType,
    header_name: Option<&str>,
) -> Result<()> {
    if auth_type != domain::McpUpstreamAuthType::CustomHeader {
        if header_name.is_some() {
            return Err(ControlPlaneError::InvalidInput("custom_header_name").into());
        }
        return Ok(());
    }
    let Some(header_name) = header_name else {
        return Err(ControlPlaneError::InvalidInput("custom_header_name").into());
    };
    let normalized = header_name.to_ascii_lowercase();
    let forbidden = [
        "authorization",
        "connection",
        "content-length",
        "host",
        "keep-alive",
        "proxy-authenticate",
        "proxy-authorization",
        "te",
        "trailer",
        "transfer-encoding",
        "upgrade",
    ];
    if header_name.is_empty()
        || header_name.len() > 128
        || !header_name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        || forbidden.contains(&normalized.as_str())
    {
        return Err(ControlPlaneError::InvalidInput("custom_header_name").into());
    }
    Ok(())
}

pub(super) fn proxy_tool_id(connection_id: Uuid, remote_tool_name: &str) -> String {
    let normalized = remote_tool_name
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '_' || character == '-' {
                character
            } else {
                '_'
            }
        })
        .collect::<String>();
    let connection = connection_id.simple().to_string();
    format!("mcp_{}_{}", &connection[..8], normalized)
        .chars()
        .take(255)
        .collect()
}

pub(super) fn proxy_input_mapping(schema: &serde_json::Value) -> serde_json::Value {
    serde_json::json!({"mappings": domain::mcp_management::identity_mcp_field_mapping(schema)
        .into_iter().map(|mapping| serde_json::json!({
            "local_path": mapping.source_path, "remote_path": mapping.target_path,
            "required": mapping.required,
        })).collect::<Vec<_>>()})
}

pub(super) fn proxy_output_mapping(schema: &serde_json::Value) -> serde_json::Value {
    serde_json::json!({"mappings": domain::mcp_management::identity_mcp_field_mapping(schema)
        .into_iter().map(|mapping| serde_json::json!({
            "remote_path": mapping.source_path, "local_path": mapping.target_path,
            "required": mapping.required,
        })).collect::<Vec<_>>()})
}

pub(super) fn validate_proxy_mapping_contract(
    mapping: &serde_json::Value,
    source_field: &'static str,
    target_field: &'static str,
    error_field: &'static str,
) -> Result<()> {
    let entries = mapping
        .get("mappings")
        .and_then(serde_json::Value::as_array)
        .ok_or(ControlPlaneError::InvalidInput(error_field))?;
    for entry in entries {
        let object = entry
            .as_object()
            .ok_or(ControlPlaneError::InvalidInput(error_field))?;
        if object
            .keys()
            .any(|key| ![source_field, target_field, "required"].contains(&key.as_str()))
        {
            return Err(ControlPlaneError::InvalidInput(error_field).into());
        }
        let source = object
            .get(source_field)
            .and_then(serde_json::Value::as_str)
            .ok_or(ControlPlaneError::InvalidInput(error_field))?;
        let target = object
            .get(target_field)
            .and_then(serde_json::Value::as_str)
            .ok_or(ControlPlaneError::InvalidInput(error_field))?;
        if !valid_mcp_field_path(source)
            || !valid_mcp_field_path(target)
            || !object
                .get("required")
                .is_some_and(serde_json::Value::is_boolean)
        {
            return Err(ControlPlaneError::InvalidInput(error_field).into());
        }
    }
    Ok(())
}

fn valid_mcp_field_path(path: &str) -> bool {
    !path.is_empty()
        && path.split('.').all(|segment| {
            !segment.is_empty()
                && segment.bytes().enumerate().all(|(index, byte)| match byte {
                    b'a'..=b'z' | b'A'..=b'Z' | b'_' => true,
                    b'0'..=b'9' | b'-' => index > 0,
                    _ => false,
                })
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn issue_1246_ac_004_ac_006_connection_contract_rejects_non_https_and_forbidden_headers() {
        assert!(validate_upstream_endpoint("http://example.com/mcp").is_err());
        assert!(validate_upstream_endpoint("https://example.com/mcp").is_ok());
        for header in [
            "Host",
            "Content-Length",
            "Connection",
            "Authorization",
            "Transfer-Encoding",
        ] {
            assert!(
                validate_upstream_header_name(
                    domain::McpUpstreamAuthType::CustomHeader,
                    Some(header)
                )
                .is_err(),
                "{header}"
            );
        }
        assert!(validate_upstream_header_name(
            domain::McpUpstreamAuthType::CustomHeader,
            Some("X-MCP-Key")
        )
        .is_ok());
    }

    #[test]
    fn issue_1246_ac_010_import_mapping_uses_frozen_public_field_names() {
        let schema =
            json!({"type":"object","properties":{"city":{"type":"string"}},"required":["city"]});
        assert_eq!(
            proxy_input_mapping(&schema),
            json!({"mappings":[{"local_path":"city","remote_path":"city","required":true}]})
        );
        assert_eq!(
            proxy_output_mapping(&schema),
            json!({"mappings":[{"remote_path":"city","local_path":"city","required":true}]})
        );
    }
}
