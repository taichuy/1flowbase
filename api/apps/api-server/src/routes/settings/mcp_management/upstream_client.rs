use std::{
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
    time::Duration,
};

use futures_util::StreamExt;
use reqwest::{
    header::{HeaderMap, HeaderName, HeaderValue, ACCEPT, AUTHORIZATION, CONTENT_TYPE},
    Client, Url,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

const MAX_RESPONSE_BYTES: usize = 2 * 1024 * 1024;
const MCP_PROTOCOL_VERSION: &str = "2025-03-26";

#[derive(Debug, Clone)]
pub struct McpUpstreamServerInfo {
    pub name: Option<String>,
    pub version: Option<String>,
    pub protocol_version: String,
    pub session_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct McpUpstreamTool {
    pub name: String,
    pub description: Option<String>,
    pub input_schema: Value,
    pub output_schema: Value,
    pub schema_hash: String,
}

#[derive(Debug, Clone)]
pub struct McpDiscoveryResult {
    pub server: McpUpstreamServerInfo,
    pub tools: Vec<McpUpstreamTool>,
}

#[derive(Debug, Clone)]
pub struct McpProxyExecutionTrace {
    pub local_arguments: Value,
    pub remote_arguments: Value,
    pub upstream_result: domain::McpCallToolResult,
    pub mapped_result: domain::McpCallToolResult,
}

#[derive(Debug, thiserror::Error)]
pub enum McpUpstreamClientError {
    #[error("invalid upstream endpoint")]
    InvalidEndpoint,
    #[error("upstream address is not public")]
    UnsafeAddress,
    #[error("invalid upstream authentication")]
    InvalidAuthentication,
    #[error("upstream response exceeded size budget")]
    ResponseTooLarge,
    #[error("upstream response was not JSON")]
    NonJsonResponse,
    #[error("upstream protocol error: {0}")]
    Protocol(String),
    #[error("upstream request failed: {0}")]
    Request(String),
}

#[derive(Clone)]
pub struct McpStreamableHttpClient {
    client: Client,
    endpoint: Url,
    authentication_headers: HeaderMap,
}

#[derive(Clone, Copy)]
enum McpEgressPolicy {
    PublicHttps,
    #[cfg(test)]
    TestLoopbackHttp,
}

pub fn map_proxy_arguments(
    local_arguments: &Value,
    input_mapping: &Value,
) -> Result<Value, McpUpstreamClientError> {
    let mappings = parse_mapping_entries(input_mapping, "local_path", "remote_path")?;
    domain::mcp_management::apply_mcp_field_mapping(local_arguments, &mappings)
        .map_err(|error| McpUpstreamClientError::Protocol(error.to_string()))
}

pub fn map_proxy_result(
    upstream: &domain::McpCallToolResult,
    output_mapping: &Value,
) -> Result<domain::McpCallToolResult, McpUpstreamClientError> {
    let mappings = parse_mapping_entries(output_mapping, "remote_path", "local_path")?;
    upstream
        .map_structured_content(&mappings)
        .map_err(|error| McpUpstreamClientError::Protocol(error.to_string()))
}

pub async fn execute_proxy_call(
    client: &McpStreamableHttpClient,
    remote_tool_name: &str,
    local_arguments: Value,
    input_mapping: &Value,
    output_mapping: &Value,
) -> Result<McpProxyExecutionTrace, McpUpstreamClientError> {
    let remote_arguments = map_proxy_arguments(&local_arguments, input_mapping)?;
    let upstream_result = client
        .call_tool(remote_tool_name, remote_arguments.clone())
        .await?;
    let mapped_result = map_proxy_result(&upstream_result, output_mapping)?;
    Ok(McpProxyExecutionTrace {
        local_arguments,
        remote_arguments,
        upstream_result,
        mapped_result,
    })
}

fn parse_mapping_entries(
    value: &Value,
    source_field: &str,
    target_field: &str,
) -> Result<Vec<domain::McpFieldMapping>, McpUpstreamClientError> {
    let entries = value
        .get("mappings")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            McpUpstreamClientError::Protocol("mapping requires mappings array".into())
        })?;
    entries
        .iter()
        .map(|entry| {
            let source_path = entry
                .get(source_field)
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    McpUpstreamClientError::Protocol(format!("mapping requires {source_field}"))
                })?;
            let target_path = entry
                .get(target_field)
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    McpUpstreamClientError::Protocol(format!("mapping requires {target_field}"))
                })?;
            let required = entry
                .get("required")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            Ok(domain::McpFieldMapping {
                source_path: source_path.to_string(),
                target_path: target_path.to_string(),
                required,
            })
        })
        .collect()
}

impl McpStreamableHttpClient {
    pub async fn connect(
        connection: &domain::McpUpstreamConnectionRecord,
        secret: Option<&Value>,
    ) -> Result<Self, McpUpstreamClientError> {
        Self::connect_with_policy(connection, secret, McpEgressPolicy::PublicHttps).await
    }

    #[cfg(test)]
    async fn connect_test_loopback(
        connection: &domain::McpUpstreamConnectionRecord,
    ) -> Result<Self, McpUpstreamClientError> {
        Self::connect_with_policy(connection, None, McpEgressPolicy::TestLoopbackHttp).await
    }

    async fn connect_with_policy(
        connection: &domain::McpUpstreamConnectionRecord,
        secret: Option<&Value>,
        policy: McpEgressPolicy,
    ) -> Result<Self, McpUpstreamClientError> {
        let endpoint = Url::parse(&connection.endpoint)
            .map_err(|_| McpUpstreamClientError::InvalidEndpoint)?;
        let valid_scheme = match policy {
            McpEgressPolicy::PublicHttps => endpoint.scheme() == "https",
            #[cfg(test)]
            McpEgressPolicy::TestLoopbackHttp => endpoint.scheme() == "http",
        };
        if !valid_scheme || endpoint.username() != "" || endpoint.password().is_some() {
            return Err(McpUpstreamClientError::InvalidEndpoint);
        }
        let host = endpoint
            .host_str()
            .ok_or(McpUpstreamClientError::InvalidEndpoint)?;
        let port = endpoint
            .port_or_known_default()
            .ok_or(McpUpstreamClientError::InvalidEndpoint)?;
        let addresses = tokio::net::lookup_host((host, port))
            .await
            .map_err(|error| McpUpstreamClientError::Request(error.to_string()))?
            .collect::<Vec<_>>();
        let addresses_allowed = match policy {
            McpEgressPolicy::PublicHttps => addresses.iter().all(|address| public_ip(address.ip())),
            #[cfg(test)]
            McpEgressPolicy::TestLoopbackHttp => {
                addresses.iter().all(|address| address.ip().is_loopback())
            }
        };
        if addresses.is_empty() || !addresses_allowed {
            return Err(McpUpstreamClientError::UnsafeAddress);
        }

        let mut builder = Client::builder()
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(20))
            .redirect(reqwest::redirect::Policy::none());
        for address in addresses {
            builder = builder.resolve(host, SocketAddr::new(address.ip(), port));
        }
        let client = builder
            .build()
            .map_err(|error| McpUpstreamClientError::Request(error.to_string()))?;
        let authentication_headers = authentication_headers(connection, secret)?;
        Ok(Self {
            client,
            endpoint,
            authentication_headers,
        })
    }

    pub async fn initialize(&self) -> Result<McpUpstreamServerInfo, McpUpstreamClientError> {
        let response = self
            .post_rpc(
                json!({
                    "jsonrpc": "2.0",
                    "id": "initialize",
                    "method": "initialize",
                    "params": {
                        "protocolVersion": MCP_PROTOCOL_VERSION,
                        "capabilities": {},
                        "clientInfo": {"name": "1flowbase", "version": env!("CARGO_PKG_VERSION")}
                    }
                }),
                None,
            )
            .await?;
        let result = response
            .body
            .get("result")
            .ok_or_else(|| rpc_error(&response.body))?;
        let protocol_version = result
            .get("protocolVersion")
            .and_then(Value::as_str)
            .ok_or_else(|| McpUpstreamClientError::Protocol("missing protocolVersion".into()))?
            .to_string();
        let server_info = result.get("serverInfo");
        Ok(McpUpstreamServerInfo {
            name: server_info
                .and_then(|value| value.get("name"))
                .and_then(Value::as_str)
                .map(str::to_string),
            version: server_info
                .and_then(|value| value.get("version"))
                .and_then(Value::as_str)
                .map(str::to_string),
            protocol_version,
            session_id: response.session_id,
        })
    }

    pub async fn discover_tools(&self) -> Result<McpDiscoveryResult, McpUpstreamClientError> {
        let server = self.initialize().await?;
        self.post_notification(
            json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
            server.session_id.as_deref(),
        )
        .await?;
        let mut cursor: Option<String> = None;
        let mut tools = Vec::new();
        loop {
            let params = cursor
                .as_ref()
                .map(|cursor| json!({"cursor": cursor}))
                .unwrap_or_else(|| json!({}));
            let response = self.post_rpc(
                json!({"jsonrpc":"2.0","id":format!("tools-list-{}", tools.len()),"method":"tools/list","params":params}),
                server.session_id.as_deref(),
            ).await?;
            let result = response
                .body
                .get("result")
                .ok_or_else(|| rpc_error(&response.body))?;
            let page = result
                .get("tools")
                .and_then(Value::as_array)
                .ok_or_else(|| McpUpstreamClientError::Protocol("missing tools".into()))?;
            for tool in page {
                let name = tool.get("name").and_then(Value::as_str).ok_or_else(|| {
                    McpUpstreamClientError::Protocol("tool name is missing".into())
                })?;
                let input_schema = tool
                    .get("inputSchema")
                    .cloned()
                    .unwrap_or_else(|| json!({"type":"object"}));
                let output_schema = tool
                    .get("outputSchema")
                    .cloned()
                    .unwrap_or_else(|| json!({}));
                let description = tool
                    .get("description")
                    .and_then(Value::as_str)
                    .map(str::to_string);
                let schema_hash =
                    definition_hash(name, description.as_deref(), &input_schema, &output_schema);
                tools.push(McpUpstreamTool {
                    name: name.into(),
                    description,
                    input_schema,
                    output_schema,
                    schema_hash,
                });
            }
            cursor = result
                .get("nextCursor")
                .and_then(Value::as_str)
                .map(str::to_string);
            if cursor.is_none() {
                break;
            }
        }
        Ok(McpDiscoveryResult { server, tools })
    }

    pub async fn call_tool(
        &self,
        remote_tool_name: &str,
        arguments: Value,
    ) -> Result<domain::McpCallToolResult, McpUpstreamClientError> {
        let server = self.initialize().await?;
        self.post_notification(
            json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
            server.session_id.as_deref(),
        )
        .await?;
        let response = self.post_rpc(
            json!({"jsonrpc":"2.0","id":"tools-call","method":"tools/call","params":{"name":remote_tool_name,"arguments":arguments}}),
            server.session_id.as_deref(),
        ).await?;
        let result = response
            .body
            .get("result")
            .cloned()
            .ok_or_else(|| rpc_error(&response.body))?;
        serde_json::from_value(result)
            .map_err(|error| McpUpstreamClientError::Protocol(error.to_string()))
    }

    async fn post_notification(
        &self,
        body: Value,
        session_id: Option<&str>,
    ) -> Result<(), McpUpstreamClientError> {
        let response = self.send(body, session_id).await?;
        if !response.status().is_success() {
            return Err(McpUpstreamClientError::Protocol(format!(
                "HTTP {}",
                response.status()
            )));
        }
        Ok(())
    }

    async fn post_rpc(
        &self,
        body: Value,
        session_id: Option<&str>,
    ) -> Result<RpcResponse, McpUpstreamClientError> {
        let response = self.send(body, session_id).await?;
        if !response.status().is_success() {
            return Err(McpUpstreamClientError::Protocol(format!(
                "HTTP {}",
                response.status()
            )));
        }
        let content_type = response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or("");
        if !content_type
            .split(';')
            .next()
            .is_some_and(|value| value.trim().eq_ignore_ascii_case("application/json"))
        {
            return Err(McpUpstreamClientError::NonJsonResponse);
        }
        let session_id = response
            .headers()
            .get("mcp-session-id")
            .and_then(|value| value.to_str().ok())
            .map(str::to_string);
        let bytes = bounded_body(response).await?;
        let body =
            serde_json::from_slice(&bytes).map_err(|_| McpUpstreamClientError::NonJsonResponse)?;
        Ok(RpcResponse { body, session_id })
    }

    async fn send(
        &self,
        body: Value,
        session_id: Option<&str>,
    ) -> Result<reqwest::Response, McpUpstreamClientError> {
        let mut request = self
            .client
            .post(self.endpoint.clone())
            .header(ACCEPT, "application/json")
            .headers(self.authentication_headers.clone())
            .json(&body);
        if let Some(session_id) = session_id {
            request = request.header("mcp-session-id", session_id);
        }
        request
            .send()
            .await
            .map_err(|error| McpUpstreamClientError::Request(error.to_string()))
    }
}

struct RpcResponse {
    body: Value,
    session_id: Option<String>,
}

fn authentication_headers(
    connection: &domain::McpUpstreamConnectionRecord,
    secret: Option<&Value>,
) -> Result<HeaderMap, McpUpstreamClientError> {
    let mut headers = HeaderMap::new();
    match connection.auth_type {
        domain::McpUpstreamAuthType::None => {}
        domain::McpUpstreamAuthType::Bearer => {
            let token = secret
                .and_then(|value| value.get("token"))
                .and_then(Value::as_str)
                .ok_or(McpUpstreamClientError::InvalidAuthentication)?;
            let value = HeaderValue::from_str(&format!("Bearer {token}"))
                .map_err(|_| McpUpstreamClientError::InvalidAuthentication)?;
            headers.insert(AUTHORIZATION, value);
        }
        domain::McpUpstreamAuthType::CustomHeader => {
            let name = connection
                .custom_header_name
                .as_deref()
                .ok_or(McpUpstreamClientError::InvalidAuthentication)?;
            let value = secret
                .and_then(|value| value.get("header_value"))
                .and_then(Value::as_str)
                .ok_or(McpUpstreamClientError::InvalidAuthentication)?;
            let name = HeaderName::from_bytes(name.as_bytes())
                .map_err(|_| McpUpstreamClientError::InvalidAuthentication)?;
            let value = HeaderValue::from_str(value)
                .map_err(|_| McpUpstreamClientError::InvalidAuthentication)?;
            headers.insert(name, value);
        }
    }
    Ok(headers)
}

async fn bounded_body(response: reqwest::Response) -> Result<Vec<u8>, McpUpstreamClientError> {
    if response
        .content_length()
        .is_some_and(|length| length > MAX_RESPONSE_BYTES as u64)
    {
        return Err(McpUpstreamClientError::ResponseTooLarge);
    }
    let mut stream = response.bytes_stream();
    let mut bytes = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|error| McpUpstreamClientError::Request(error.to_string()))?;
        if bytes.len() + chunk.len() > MAX_RESPONSE_BYTES {
            return Err(McpUpstreamClientError::ResponseTooLarge);
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

fn rpc_error(body: &Value) -> McpUpstreamClientError {
    let error = body.get("error");
    let message = error
        .and_then(|error| error.get("message"))
        .and_then(Value::as_str)
        .unwrap_or("missing JSON-RPC result");
    let diagnostic = error
        .and_then(|error| error.get("data"))
        .map(|data| format!("{message}; data={data}"))
        .unwrap_or_else(|| message.to_string());
    McpUpstreamClientError::Protocol(diagnostic)
}

fn definition_hash(name: &str, description: Option<&str>, input: &Value, output: &Value) -> String {
    let definition = canonicalize_json(&json!({
        "name": name,
        "description": description,
        "inputSchema": input,
        "outputSchema": output,
    }));
    let digest = Sha256::digest(definition.to_string().as_bytes());
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn canonicalize_json(value: &Value) -> Value {
    match value {
        Value::Object(object) => {
            let mut keys = object.keys().collect::<Vec<_>>();
            keys.sort_unstable();
            Value::Object(
                keys.into_iter()
                    .map(|key| (key.clone(), canonicalize_json(&object[key])))
                    .collect(),
            )
        }
        Value::Array(items) => Value::Array(items.iter().map(canonicalize_json).collect()),
        _ => value.clone(),
    }
}

fn public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => public_ipv4(ip),
        IpAddr::V6(ip) => public_ipv6(ip),
    }
}

fn public_ipv4(ip: Ipv4Addr) -> bool {
    let octets = ip.octets();
    !(ip.is_private()
        || ip.is_loopback()
        || ip.is_link_local()
        || ip.is_unspecified()
        || ip.is_multicast()
        || ip == Ipv4Addr::BROADCAST
        || octets[0] == 0
        || octets[0] >= 224
        || (octets[0] == 100 && (64..=127).contains(&octets[1]))
        || (octets[0] == 192 && octets[1] == 0 && matches!(octets[2], 0 | 2))
        || (octets[0] == 198 && (octets[1] == 18 || octets[1] == 19 || octets[1] == 51))
        || (octets[0] == 203 && octets[1] == 0 && octets[2] == 113))
}

fn public_ipv6(ip: Ipv6Addr) -> bool {
    let segments = ip.segments();
    !(ip.is_loopback()
        || ip.is_unspecified()
        || ip.is_multicast()
        || (segments[0] & 0xfe00) == 0xfc00
        || (segments[0] & 0xffc0) == 0xfe80
        || (segments[0] == 0x2001 && segments[1] == 0x0db8)
        || ip
            .to_ipv4_mapped()
            .is_some_and(|mapped| !public_ipv4(mapped)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    use axum::{
        extract::State,
        http::{HeaderMap as AxumHeaderMap, StatusCode},
        response::IntoResponse,
        routing::post,
        Json, Router,
    };
    use time::OffsetDateTime;

    #[derive(Clone, Default)]
    struct StubState {
        methods: Arc<Mutex<Vec<String>>>,
        session_headers: Arc<Mutex<Vec<Option<String>>>>,
    }

    async fn mcp_stub(
        State(state): State<StubState>,
        headers: AxumHeaderMap,
        Json(body): Json<Value>,
    ) -> impl IntoResponse {
        let method = body
            .get("method")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        state.methods.lock().unwrap().push(method.clone());
        state.session_headers.lock().unwrap().push(
            headers
                .get("mcp-session-id")
                .and_then(|value| value.to_str().ok())
                .map(str::to_string),
        );
        let mut response_headers = AxumHeaderMap::new();
        if method == "initialize" {
            response_headers.insert("mcp-session-id", HeaderValue::from_static("session-1246"));
            return (
                StatusCode::OK,
                response_headers,
                Json(json!({
                    "jsonrpc":"2.0","id":"initialize","result":{
                        "protocolVersion":MCP_PROTOCOL_VERSION,
                        "serverInfo":{"name":"stub-mcp","version":"1.0.0"},"capabilities":{"tools":{}}
                    }
                })),
            );
        }
        if headers
            .get("mcp-session-id")
            .and_then(|value| value.to_str().ok())
            != Some("session-1246")
        {
            return (
                StatusCode::BAD_REQUEST,
                response_headers,
                Json(json!({"error":"missing session"})),
            );
        }
        match method.as_str() {
            "notifications/initialized" => {
                (StatusCode::ACCEPTED, response_headers, Json(json!({})))
            }
            "tools/list" => {
                let cursor = body.pointer("/params/cursor").and_then(Value::as_str);
                let result = if cursor.is_none() {
                    json!({"tools":[{"name":"weather.lookup","description":"Weather","inputSchema":{"type":"object","properties":{"city":{"type":"string"}}},"outputSchema":{"type":"object"}}],"nextCursor":"page-2"})
                } else {
                    json!({"tools":[{"name":"clock.now","inputSchema":{"type":"object"}}]})
                };
                (
                    StatusCode::OK,
                    response_headers,
                    Json(json!({"jsonrpc":"2.0","id":"list","result":result})),
                )
            }
            "tools/call" => (
                StatusCode::OK,
                response_headers,
                Json(json!({
                    "jsonrpc":"2.0","id":"tools-call","result":{
                        "content":[{"type":"text","text":"upstream warning"}],
                        "structuredContent":{"weather":{"temperature":28}},"isError":true
                    }
                })),
            ),
            _ => (
                StatusCode::BAD_REQUEST,
                response_headers,
                Json(json!({"error":"unknown method"})),
            ),
        }
    }

    fn loopback_connection(endpoint: String) -> domain::McpUpstreamConnectionRecord {
        let now = OffsetDateTime::now_utc();
        domain::McpUpstreamConnectionRecord {
            id: uuid::Uuid::now_v7(),
            workspace_id: uuid::Uuid::now_v7(),
            name: "Stub".into(),
            endpoint,
            transport: domain::McpUpstreamTransport::StreamableHttp,
            auth_type: domain::McpUpstreamAuthType::None,
            custom_header_name: None,
            status: domain::McpUpstreamConnectionStatus::Enabled,
            credentials_configured: false,
            last_connected_at: None,
            last_discovered_at: None,
            last_error: None,
            created_by: uuid::Uuid::now_v7(),
            updated_by: uuid::Uuid::now_v7(),
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn issue_1246_ac_006_rejects_private_and_metadata_addresses() {
        for address in [
            "127.0.0.1",
            "10.0.0.1",
            "169.254.169.254",
            "192.168.1.1",
            "100.64.0.1",
            "::1",
            "fe80::1",
            "fc00::1",
        ] {
            assert!(!public_ip(address.parse().unwrap()), "{address}");
        }
        assert!(public_ip("1.1.1.1".parse().unwrap()));
        assert!(public_ip("2606:4700:4700::1111".parse().unwrap()));
    }

    #[test]
    fn issue_1246_ac_015_definition_hash_ignores_object_key_order() {
        let first = json!({"type":"object","properties":{"city":{"type":"string"},"units":{"type":"string"}}});
        let second: Value = serde_json::from_str(
            r#"{"properties":{"units":{"type":"string"},"city":{"type":"string"}},"type":"object"}"#,
        ).unwrap();
        assert_eq!(
            definition_hash("weather", Some("Weather"), &first, &json!({})),
            definition_hash("weather", Some("Weather"), &second, &json!({}))
        );
    }

    #[test]
    fn issue_1246_ac_012_protocol_error_preserves_upstream_error_data() {
        let error = rpc_error(&json!({
            "jsonrpc":"2.0","id":"call","error":{"code":-32001,"message":"tool failed","data":{"reason":"quota"}}
        }));
        assert_eq!(
            error.to_string(),
            "upstream protocol error: tool failed; data={\"reason\":\"quota\"}"
        );
    }

    #[test]
    fn issue_1246_ac_011_ac_013_public_proxy_mapping_shapes_map_both_directions() {
        let remote_arguments = map_proxy_arguments(
            &json!({"request":{"city":"Shanghai","ignored":true}}),
            &json!({"mappings":[{"local_path":"request.city","remote_path":"location.name","required":true}]}),
        ).unwrap();
        assert_eq!(remote_arguments, json!({"location":{"name":"Shanghai"}}));

        let upstream = domain::McpCallToolResult {
            content: json!([{"type":"text","text":"ok"}]),
            structured_content: Some(json!({"weather":{"temperature":28}})),
            is_error: Some(false),
        };
        let mapped = map_proxy_result(
            &upstream,
            &json!({"mappings":[{"remote_path":"weather.temperature","local_path":"temperature_celsius","required":true}]}),
        ).unwrap();
        assert_eq!(
            mapped.structured_content,
            Some(json!({"temperature_celsius":28}))
        );
        assert_eq!(mapped.content, upstream.content);
        assert_eq!(mapped.is_error, upstream.is_error);
    }

    #[tokio::test]
    async fn issue_1246_ac_007_ac_012_ac_013_ac_014_streamable_http_session_pagination_call_and_trace(
    ) {
        let state = StubState::default();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let app = Router::new()
            .route("/mcp", post(mcp_stub))
            .with_state(state.clone());
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        let client = McpStreamableHttpClient::connect_test_loopback(&loopback_connection(format!(
            "http://{address}/mcp"
        )))
        .await
        .unwrap();

        let discovery = client.discover_tools().await.unwrap();
        assert_eq!(discovery.server.name.as_deref(), Some("stub-mcp"));
        assert_eq!(
            discovery
                .tools
                .iter()
                .map(|tool| tool.name.as_str())
                .collect::<Vec<_>>(),
            vec!["weather.lookup", "clock.now"]
        );

        let trace = execute_proxy_call(
            &client,
            "weather.lookup",
            json!({"city":"Shanghai"}),
            &json!({"mappings":[{"local_path":"city","remote_path":"location.name","required":true}]}),
            &json!({"mappings":[{"remote_path":"weather.temperature","local_path":"temperature_celsius","required":true}]}),
        ).await.unwrap();
        assert_eq!(trace.local_arguments, json!({"city":"Shanghai"}));
        assert_eq!(
            trace.remote_arguments,
            json!({"location":{"name":"Shanghai"}})
        );
        assert_eq!(
            trace.upstream_result.content,
            json!([{"type":"text","text":"upstream warning"}])
        );
        assert_eq!(trace.mapped_result.content, trace.upstream_result.content);
        assert_eq!(trace.mapped_result.is_error, Some(true));
        assert_eq!(
            trace.mapped_result.structured_content,
            Some(json!({"temperature_celsius":28}))
        );

        let methods = state.methods.lock().unwrap().clone();
        assert_eq!(
            methods,
            vec![
                "initialize",
                "notifications/initialized",
                "tools/list",
                "tools/list",
                "initialize",
                "notifications/initialized",
                "tools/call",
            ]
        );
        let sessions = state.session_headers.lock().unwrap().clone();
        assert_eq!(
            sessions
                .iter()
                .filter(|value| value.as_deref() == Some("session-1246"))
                .count(),
            5
        );
        server.abort();
    }
}
