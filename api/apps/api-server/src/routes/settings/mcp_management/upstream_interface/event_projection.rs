use super::*;
use crate::event_center::api_projection::{
    self as event, ApiEventInput, ApiEventOperationContext, ApiEventOutput,
};
use serde_json::{json, Value};
use std::sync::Arc;

// A URL is configuration, but embedded userinfo/query values can be credentials.
// Keep the endpoint location while omitting credential-bearing URL components.
fn endpoint_value(endpoint: &str) -> Result<String, Arc<str>> {
    let mut endpoint = reqwest::Url::parse(endpoint)
        .map_err(|_| Arc::<str>::from("invalid upstream endpoint event view"))?;
    endpoint
        .set_username("")
        .map_err(|_| Arc::<str>::from("unsupported upstream endpoint event view"))?;
    endpoint
        .set_password(None)
        .map_err(|_| Arc::<str>::from("unsupported upstream endpoint event view"))?;
    endpoint.set_query(None);
    endpoint.set_fragment(None);
    Ok(endpoint.to_string())
}
#[derive(serde::Serialize, utoipa::ToSchema)]
struct ConnectionConfigEvent<'a> {
    name: &'a str,
    endpoint: String,
    transport: &'a str,
    auth_type: &'a str,
    custom_header_name: &'a Option<String>,
    status: &'a str,
}
fn connection_config(
    value: &SaveMcpUpstreamConnectionBody,
) -> Result<ConnectionConfigEvent<'_>, Arc<str>> {
    Ok(ConnectionConfigEvent {
        name: &value.name,
        endpoint: endpoint_value(&value.endpoint)?,
        transport: &value.transport,
        auth_type: &value.auth_type,
        custom_header_name: &value.custom_header_name,
        status: &value.status,
    })
}
#[derive(serde::Serialize, utoipa::ToSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum CredentialEvent<'a> {
    Bearer,
    CustomHeader { header_name: &'a str },
}
fn credential(value: &SaveMcpUpstreamCredentialBody) -> CredentialEvent<'_> {
    match value {
        SaveMcpUpstreamCredentialBody::Bearer { token: _ } => CredentialEvent::Bearer,
        SaveMcpUpstreamCredentialBody::CustomHeader {
            header_name,
            header_value: _,
        } => CredentialEvent::CustomHeader { header_name },
    }
}
#[derive(serde::Serialize, utoipa::ToSchema)]
struct DraftEvent<'a> {
    connection_id: &'a Option<String>,
    endpoint: String,
    transport: &'a str,
    auth_type: &'a str,
    custom_header_name: &'a Option<String>,
    credential: Option<CredentialEvent<'a>>,
}
#[derive(serde::Serialize, utoipa::ToSchema)]
struct ConnectionEvent<'a> {
    connection_id: &'a str,
    workspace_id: &'a str,
    name: &'a str,
    endpoint: String,
    transport: &'a str,
    auth_type: &'a str,
    custom_header_name: &'a Option<String>,
    status: &'a str,
    credentials_status: &'a str,
    last_connected_at: &'a Option<String>,
    last_discovered_at: &'a Option<String>,
    created_at: &'a str,
    updated_at: &'a str,
}
fn connection(value: &McpUpstreamConnectionResponse) -> Result<ConnectionEvent<'_>, Arc<str>> {
    Ok(ConnectionEvent {
        connection_id: &value.connection_id,
        workspace_id: &value.workspace_id,
        name: &value.name,
        endpoint: endpoint_value(&value.endpoint)?,
        transport: &value.transport,
        auth_type: &value.auth_type,
        custom_header_name: &value.custom_header_name,
        status: &value.status,
        credentials_status: &value.credentials_status,
        last_connected_at: &value.last_connected_at,
        last_discovered_at: &value.last_discovered_at,
        created_at: &value.created_at,
        updated_at: &value.updated_at,
    })
}
#[derive(serde::Serialize, utoipa::ToSchema)]
struct TestEvent<'a> {
    connection_id: &'a str,
    ok: bool,
    server_name: &'a Option<String>,
    server_version: &'a Option<String>,
    protocol_version: &'a Option<String>,
    tested_at: &'a str,
}
#[derive(serde::Serialize, utoipa::ToSchema)]
struct DraftTestEvent<'a> {
    ok: bool,
    server_name: &'a Option<String>,
    server_version: &'a Option<String>,
    protocol_version: &'a Option<String>,
    tested_at: &'a str,
}
impl ApiEventInput for McpUpstreamInput {
    fn document_input_schema(operation: &ApiEventOperationContext) -> Option<Value> {
        if operation.interface_id == "mcp.upstream_credentials.update" {
            Some(
                json!({"type":"object","properties":{"path":{"type":"object","properties":{"connection_id":{"type":"string"}},"required":["connection_id"]},"body":event::schema::<CredentialEvent<'static>>()},"required":["path","body"],"additionalProperties":false}),
            )
        } else if operation.path == "/api/console/mcp/upstream-connections/test" {
            Some(
                json!({"type":"object","properties":{"body":event::schema::<DraftEvent<'static>>()},"required":["body"],"additionalProperties":false}),
            )
        } else {
            None
        }
    }
    fn project_document_input(&self, _: &ApiEventOperationContext) -> Result<Value, Arc<str>> {
        Ok(match self {
            Self::List => json!({}),
            Self::Create(body) => json!({"body":event::to_value(&connection_config(body)?)?}),
            Self::Update {
                connection_id,
                body,
            } => {
                json!({"path":{"connection_id":connection_id},"body":event::to_value(&connection_config(body)?)?})
            }
            Self::Delete(connection_id)
            | Self::DeleteCredentials(connection_id)
            | Self::Test(connection_id)
            | Self::Discover(connection_id) => json!({"path":{"connection_id":connection_id}}),
            Self::SaveCredentials {
                connection_id,
                body,
            } => {
                json!({"path":{"connection_id":connection_id},"body":event::to_value(&credential(body))?})
            }
            Self::TestDraft(body) => json!({"body":event::to_value(&DraftEvent {
                connection_id:&body.connection_id, endpoint:endpoint_value(&body.endpoint)?, transport:&body.transport,
                auth_type:&body.auth_type, custom_header_name:&body.custom_header_name,
                credential:body.credential.as_ref().map(credential),
            })?}),
            Self::Import {
                connection_id,
                body,
            } => json!({"path":{"connection_id":connection_id},"body":event::to_value(body)?}),
            Self::Debug { tool_id, body } => {
                json!({"path":{"tool_id":tool_id},"body":event::to_value(body)?})
            }
        })
    }
}
impl ApiEventOutput for McpUpstreamOutput {
    fn document_output_schema(operation: &ApiEventOperationContext) -> Option<Value> {
        match operation.interface_id.as_str() {
            "mcp.upstream_connections.view" => {
                Some(event::schema::<Vec<ConnectionEvent<'static>>>())
            }
            "mcp.upstream_connections.create" | "mcp.upstream_connections.update" => {
                Some(event::schema::<ConnectionEvent<'static>>())
            }
            "mcp.upstream_connections.test"
                if operation.path == "/api/console/mcp/upstream-connections/test" =>
            {
                Some(event::schema::<DraftTestEvent<'static>>())
            }
            "mcp.upstream_connections.test" => Some(event::schema::<TestEvent<'static>>()),
            _ => None,
        }
    }
    fn project_document_output(&self, _: &ApiEventOperationContext) -> Result<Value, Arc<str>> {
        match self {
            Self::Connections(values) => event::to_value(
                &values
                    .iter()
                    .map(connection)
                    .collect::<Result<Vec<_>, _>>()?,
            ),
            Self::Created(value) | Self::Connection(value) => event::to_value(&connection(value)?),
            Self::DraftTest(value) => event::to_value(&DraftTestEvent {
                ok: value.ok,
                server_name: &value.server_name,
                server_version: &value.server_version,
                protocol_version: &value.protocol_version,
                tested_at: &value.tested_at,
            }),
            Self::Test(value) => event::to_value(&TestEvent {
                connection_id: &value.connection_id,
                ok: value.ok,
                server_name: &value.server_name,
                server_version: &value.server_version,
                protocol_version: &value.protocol_version,
                tested_at: &value.tested_at,
            }),
            Self::Discovery(value) => event::to_value(value),
            Self::Imported(value) => event::to_value(value),
            Self::Debug(value) => event::to_value(value),
            Self::NoContent => Ok(Value::Null),
        }
    }
}

#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
#[path = "_tests/event_projection.rs"]
mod event_tests;
