use super::*;

struct RpcResponse {
    pub(super) body: Value,
    pub(super) session_id: Option<String>,
    pub(super) response_bytes: usize,
}

pub(super) fn authentication_headers(
    auth_type: domain::McpUpstreamAuthType,
    custom_header_name: Option<&str>,
    secret: Option<&Value>,
) -> Result<HeaderMap, McpUpstreamClientError> {
    let mut headers = HeaderMap::new();
    match auth_type {
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
            let name = custom_header_name.ok_or(McpUpstreamClientError::InvalidAuthentication)?;
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

pub(super) async fn bounded_body(
    response: reqwest::Response,
) -> Result<Vec<u8>, McpUpstreamClientError> {
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

pub(super) async fn upstream_http_status_error(
    response: reqwest::Response,
) -> McpUpstreamClientError {
    let status = response.status();
    match bounded_error_body(response).await {
        Ok((bytes, truncated)) => {
            let detail = String::from_utf8_lossy(&bytes).trim().to_string();
            let suffix = if truncated { " [truncated]" } else { "" };
            if detail.is_empty() {
                McpUpstreamClientError::Protocol(format!("HTTP {status}"))
            } else {
                McpUpstreamClientError::Protocol(format!("HTTP {status}: {detail}{suffix}"))
            }
        }
        Err(error) => error,
    }
}

pub(super) async fn bounded_error_body(
    response: reqwest::Response,
) -> Result<(Vec<u8>, bool), McpUpstreamClientError> {
    let mut stream = response.bytes_stream();
    let mut bytes = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|error| McpUpstreamClientError::Request(error.to_string()))?;
        let remaining = MAX_ERROR_RESPONSE_BYTES.saturating_sub(bytes.len());
        if chunk.len() > remaining {
            bytes.extend_from_slice(&chunk[..remaining]);
            return Ok((bytes, true));
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok((bytes, false))
}

pub(super) async fn bounded_sse_rpc_body(
    response: reqwest::Response,
    expected_id: &Value,
) -> Result<(Value, usize), McpUpstreamClientError> {
    if response
        .content_length()
        .is_some_and(|length| length > MAX_RESPONSE_BYTES as u64)
    {
        return Err(McpUpstreamClientError::ResponseTooLarge);
    }
    let mut stream = response.bytes_stream();
    let mut pending = Vec::new();
    let mut event_data = Vec::new();
    let mut response_bytes = 0usize;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|error| McpUpstreamClientError::Request(error.to_string()))?;
        response_bytes = response_bytes
            .checked_add(chunk.len())
            .ok_or(McpUpstreamClientError::ResponseTooLarge)?;
        if response_bytes > MAX_RESPONSE_BYTES {
            return Err(McpUpstreamClientError::ResponseTooLarge);
        }
        pending.extend_from_slice(&chunk);

        let mut consumed = 0usize;
        while let Some(offset) = pending[consumed..].iter().position(|byte| *byte == b'\n') {
            let line_end = consumed + offset;
            let line = pending[consumed..line_end]
                .strip_suffix(b"\r")
                .unwrap_or(&pending[consumed..line_end]);
            if let Some(body) = observe_sse_line(line, &mut event_data, expected_id)? {
                return Ok((body, response_bytes));
            }
            consumed = line_end + 1;
        }
        if consumed > 0 {
            pending.drain(..consumed);
        }
    }

    if !pending.is_empty() {
        let line = pending.strip_suffix(b"\r").unwrap_or(&pending);
        if let Some(body) = observe_sse_line(line, &mut event_data, expected_id)? {
            return Ok((body, response_bytes));
        }
    }
    if let Some(body) = finish_sse_event(&mut event_data, expected_id)? {
        return Ok((body, response_bytes));
    }
    Err(McpUpstreamClientError::Protocol(
        "missing JSON-RPC response in SSE stream".into(),
    ))
}

pub(super) fn observe_sse_line(
    line: &[u8],
    event_data: &mut Vec<u8>,
    expected_id: &Value,
) -> Result<Option<Value>, McpUpstreamClientError> {
    if line.is_empty() {
        return finish_sse_event(event_data, expected_id);
    }
    if line.starts_with(b":") {
        return Ok(None);
    }
    if let Some(data) = line.strip_prefix(b"data:") {
        let data = data.strip_prefix(b" ").unwrap_or(data);
        event_data.extend_from_slice(data);
        event_data.push(b'\n');
    }
    Ok(None)
}

pub(super) fn finish_sse_event(
    event_data: &mut Vec<u8>,
    expected_id: &Value,
) -> Result<Option<Value>, McpUpstreamClientError> {
    if event_data.is_empty() {
        return Ok(None);
    }
    if event_data.last() == Some(&b'\n') {
        event_data.pop();
    }
    let body = serde_json::from_slice::<Value>(event_data)
        .map_err(|_| McpUpstreamClientError::NonJsonResponse)?;
    event_data.clear();
    Ok((body.get("id") == Some(expected_id)).then_some(body))
}

pub(super) fn rpc_error(body: &Value) -> McpUpstreamClientError {
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

pub(super) fn definition_hash(
    name: &str,
    description: Option<&str>,
    input: &Value,
    output: &Value,
) -> String {
    let definition = canonicalize_json(&json!({
        "name": name,
        "description": description,
        "inputSchema": input,
        "outputSchema": output,
    }));
    let digest = Sha256::digest(definition.to_string().as_bytes());
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub(super) fn canonicalize_json(value: &Value) -> Value {
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

pub(super) fn public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => public_ipv4(ip),
        IpAddr::V6(ip) => public_ipv6(ip),
    }
}

pub(super) fn public_ipv4(ip: Ipv4Addr) -> bool {
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

pub(super) fn public_ipv6(ip: Ipv6Addr) -> bool {
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
