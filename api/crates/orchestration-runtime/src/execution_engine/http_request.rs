use std::{collections::BTreeMap, time::Duration};

use anyhow::{anyhow, bail, Context, Result};
use async_trait::async_trait;
use futures_util::StreamExt;
use reqwest::{
    header::{HeaderMap, HeaderName, HeaderValue, CONTENT_TYPE},
    multipart::{Form, Part},
    Client, Method,
};
use serde_json::{json, Map, Value};

use crate::{binding_runtime::render_template, compiled_plan::CompiledNode};

const DEFAULT_TIMEOUT_MS: u64 = 30_000;
const DEFAULT_MAX_RESPONSE_BYTES: u64 = 6 * 1024 * 1024;
const MAX_RESPONSE_BYTES_LIMIT: u64 = 10 * 1024 * 1024;
const INLINE_RESPONSE_STORAGE_ID: &str = "runtime-inline";

#[derive(Debug, Clone, PartialEq)]
pub struct HttpRequestNodeExecution {
    pub output_payload: Value,
    pub error_payload: Option<Value>,
    pub metrics_payload: Value,
    pub debug_payload: Value,
}

pub struct HttpResponseFilePersistInput<'a> {
    pub node_id: &'a str,
    pub filename: &'a str,
    pub content_type: &'a str,
    pub bytes: &'a [u8],
}

#[async_trait]
pub trait HttpResponseFilePersister: Send + Sync {
    async fn persist_http_response_file(
        &self,
        input: HttpResponseFilePersistInput<'_>,
    ) -> Result<Value>;
}

/// A short-lived host-created HTTP client. Node execution can use the client but never receives
/// the provider lease or its cleanup capability.
pub struct HttpRequestClientLease {
    client: Client,
    releaser: Box<dyn HttpRequestClientLeaseReleaser>,
}

impl HttpRequestClientLease {
    pub fn new(client: Client, releaser: Box<dyn HttpRequestClientLeaseReleaser>) -> Self {
        Self { client, releaser }
    }

    fn client(&self) -> &Client {
        &self.client
    }

    async fn release(self) -> Result<()> {
        self.releaser.release().await
    }
}

#[async_trait]
pub trait HttpRequestClientLeaseReleaser: Send + Sync {
    async fn release(self: Box<Self>) -> Result<()>;
}

#[derive(Debug, Clone)]
struct HttpFileDescriptor {
    filename: String,
    mimetype: String,
    url: Option<String>,
}

impl HttpFileDescriptor {
    fn from_value(value: &Value) -> Result<Self> {
        let object = value
            .as_object()
            .ok_or_else(|| anyhow!("file descriptor must be an object"))?;
        let filename = required_string(object, "filename")?.to_string();
        let _size = object.get("size").and_then(Value::as_u64).unwrap_or(0);
        let mimetype = required_string(object, "mimetype")?.to_string();
        let _path = required_string(object, "path")?;
        let _storage_id = required_string(object, "storage_id")?;
        let url = object
            .get("url")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .map(str::to_string);
        let _meta = object.get("meta").unwrap_or(&Value::Null);

        Ok(Self {
            filename,
            mimetype,
            url,
        })
    }
}

#[derive(Debug, Clone)]
struct HttpFileBytes {
    descriptor: HttpFileDescriptor,
    bytes: Vec<u8>,
}

struct HttpResponseBodyBytes {
    stored_bytes: Vec<u8>,
    response_file_bytes: Option<Vec<u8>>,
    response_bytes_observed: u64,
    body_truncated: bool,
}

pub async fn execute_http_request_node(
    node: &CompiledNode,
    resolved_inputs: &Map<String, Value>,
    variable_pool: &Map<String, Value>,
    file_persister: Option<&dyn HttpResponseFilePersister>,
) -> Result<HttpRequestNodeExecution> {
    execute_http_request_node_with_provider_invoker::<dyn super::ProviderInvoker>(
        node,
        resolved_inputs,
        variable_pool,
        file_persister,
        None,
    )
    .await
}

pub async fn execute_http_request_node_with_provider_invoker<I>(
    node: &CompiledNode,
    resolved_inputs: &Map<String, Value>,
    variable_pool: &Map<String, Value>,
    file_persister: Option<&dyn HttpResponseFilePersister>,
    provider_invoker: Option<&I>,
) -> Result<HttpRequestNodeExecution>
where
    I: super::ProviderInvoker + ?Sized,
{
    let timeout = Duration::from_millis(config_u64(&node.config, "timeout_ms", DEFAULT_TIMEOUT_MS));
    let verify_ssl = node
        .config
        .get("verify_ssl")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let lease = match provider_invoker {
        Some(invoker) => match invoker.acquire_http_node_client(timeout, verify_ssl).await {
            Ok(lease) => lease,
            Err(error) => return http_request_execution_result(Err(error)),
        },
        None => None,
    };
    let result = execute_http_request_node_inner(
        node,
        resolved_inputs,
        variable_pool,
        file_persister,
        lease.as_ref().map(HttpRequestClientLease::client),
    )
    .await;
    let release = match lease {
        Some(lease) => lease.release().await,
        None => Ok(()),
    };
    match (result, release) {
        (Err(error), _) | (Ok(_), Err(error)) => http_request_execution_result(Err(error)),
        (Ok(execution), Ok(())) => Ok(execution),
    }
}

fn http_request_execution_result(
    result: Result<HttpRequestNodeExecution>,
) -> Result<HttpRequestNodeExecution> {
    match result {
        Ok(execution) => Ok(execution),
        Err(error) => Ok(HttpRequestNodeExecution {
            output_payload: json!({}),
            error_payload: Some(http_request_error_payload(error.to_string())),
            metrics_payload: json!({
                "preview_mode": true,
                "error": true
            }),
            debug_payload: json!({}),
        }),
    }
}

async fn execute_http_request_node_inner(
    node: &CompiledNode,
    resolved_inputs: &Map<String, Value>,
    variable_pool: &Map<String, Value>,
    file_persister: Option<&dyn HttpResponseFilePersister>,
    routed_client: Option<&Client>,
) -> Result<HttpRequestNodeExecution> {
    let timeout_ms = config_u64(&node.config, "timeout_ms", DEFAULT_TIMEOUT_MS);
    let max_response_bytes = config_u64(
        &node.config,
        "max_response_bytes",
        DEFAULT_MAX_RESPONSE_BYTES,
    )
    .min(MAX_RESPONSE_BYTES_LIMIT);
    let verify_ssl = node
        .config
        .get("verify_ssl")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let client = match routed_client {
        Some(client) => client.clone(),
        None => Client::builder()
            .timeout(Duration::from_millis(timeout_ms))
            .danger_accept_invalid_certs(!verify_ssl)
            .build()
            .context("failed to build HTTP client")?,
    };
    let method_text = node
        .config
        .get("method")
        .and_then(Value::as_str)
        .unwrap_or("GET")
        .to_uppercase();
    let method = Method::from_bytes(method_text.as_bytes()).context("HTTP method is invalid")?;
    let url_template = node
        .config
        .get("url")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("HTTP URL is required"))?;
    let url = render_template(url_template, variable_pool)?;
    if url.trim().is_empty() {
        bail!("HTTP URL is required");
    }

    let mut request = client.request(method.clone(), url.clone());
    let headers = resolved_object(resolved_inputs.get("headers"));
    if !headers.is_empty() {
        request = request.headers(build_header_map(&headers)?);
    }
    let params = resolved_object(resolved_inputs.get("params"));
    if !params.is_empty() {
        request = request.query(&string_pairs(&params));
    }

    let body_type = node
        .config
        .get("body_type")
        .and_then(Value::as_str)
        .unwrap_or("none");
    request = apply_request_body(
        request,
        &client,
        node,
        resolved_inputs,
        body_type,
        max_response_bytes,
    )
    .await?;

    let response = request.send().await.context("HTTP request failed")?;
    let status_code = response.status().as_u16();
    let response_headers = headers_to_json(response.headers());
    let response_content_type = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("application/octet-stream")
        .to_string();
    let content_length = response.content_length();
    let store_response_as_file = node
        .config
        .get("store_response_as_file")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let capture_response_file_bytes =
        store_response_as_file && !response_body_is_inline_text(&response_content_type);
    let body_bytes = read_response_body(
        response,
        content_length,
        max_response_bytes,
        capture_response_file_bytes.then_some(MAX_RESPONSE_BYTES_LIMIT),
    )
    .await?;
    let body = String::from_utf8_lossy(&body_bytes.stored_bytes).to_string();
    let stored_body_bytes = body_bytes.stored_bytes.len() as u64;
    let response_file_bytes = body_bytes
        .response_file_bytes
        .as_deref()
        .unwrap_or(&body_bytes.stored_bytes);
    let response_file_bytes_truncated =
        body_bytes.response_file_bytes.is_none() && body_bytes.body_truncated;
    let files = response_files(
        node,
        &response_content_type,
        response_file_bytes,
        response_file_bytes_truncated,
        file_persister,
    )
    .await?;
    let output_payload = json!({
        "body": body,
        "status_code": status_code,
        "headers": response_headers,
        "files": files,
    });

    Ok(HttpRequestNodeExecution {
        output_payload,
        error_payload: None,
        metrics_payload: json!({
            "preview_mode": true,
            "status_code": status_code,
            "response_bytes": body_bytes.response_bytes_observed,
            "response_bytes_observed": body_bytes.response_bytes_observed,
            "stored_body_bytes": stored_body_bytes,
            "body_truncated": body_bytes.body_truncated,
            "max_response_bytes": max_response_bytes,
            "error": false
        }),
        debug_payload: json!({
            "request": {
                "method": method.as_str(),
                "url": url,
                "body_type": body_type
            },
            "response": {
                "status_code": status_code,
                "content_type": response_content_type,
                "file_count": files.as_array().map(Vec::len).unwrap_or(0),
                "response_bytes_observed": body_bytes.response_bytes_observed,
                "stored_body_bytes": stored_body_bytes,
                "body_truncated": body_bytes.body_truncated,
                "max_response_bytes": max_response_bytes
            }
        }),
    })
}

async fn read_response_body(
    response: reqwest::Response,
    content_length: Option<u64>,
    max_response_bytes: u64,
    response_file_bytes_limit: Option<u64>,
) -> Result<HttpResponseBodyBytes> {
    let max_stored_bytes = usize::try_from(max_response_bytes)
        .context("max_response_bytes does not fit this platform")?;
    let max_file_bytes = response_file_bytes_limit
        .map(usize::try_from)
        .transpose()
        .context("response file byte limit does not fit this platform")?;
    if let (Some(content_length), Some(response_file_bytes_limit)) =
        (content_length, response_file_bytes_limit)
    {
        if content_length > response_file_bytes_limit {
            bail!("HTTP response file exceeds response file byte limit");
        }
    }
    let mut stored_bytes = Vec::with_capacity(
        content_length
            .unwrap_or(max_response_bytes)
            .min(max_response_bytes) as usize,
    );
    let mut response_file_bytes = response_file_bytes_limit
        .map(|limit| Vec::with_capacity(content_length.unwrap_or(limit).min(limit) as usize));
    let mut response_bytes_observed = 0_u64;
    let mut body_truncated = false;
    let mut stream = response.bytes_stream();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.context("failed to read HTTP response")?;
        let chunk_len = chunk.len();
        response_bytes_observed = response_bytes_observed.saturating_add(chunk_len as u64);

        if let Some(file_bytes) = response_file_bytes.as_mut() {
            let max_file_bytes = max_file_bytes.expect("file byte limit");
            if file_bytes.len().saturating_add(chunk_len) > max_file_bytes {
                bail!("HTTP response file exceeds response file byte limit");
            }
            file_bytes.extend_from_slice(&chunk);
        }

        let remaining_capacity = max_stored_bytes.saturating_sub(stored_bytes.len());
        if remaining_capacity > 0 {
            let copied_bytes = remaining_capacity.min(chunk_len);
            stored_bytes.extend_from_slice(&chunk[..copied_bytes]);
            if copied_bytes < chunk_len {
                body_truncated = true;
                if response_file_bytes.is_none() {
                    break;
                }
            }
        } else if chunk_len > 0 {
            body_truncated = true;
            if response_file_bytes.is_none() {
                break;
            }
        }

        if content_length.is_some_and(|length| length > max_response_bytes)
            && stored_bytes.len() >= max_stored_bytes
        {
            body_truncated = true;
            if response_file_bytes.is_none() {
                response_bytes_observed = content_length.unwrap_or(response_bytes_observed);
                break;
            }
        }
    }

    if let Some(content_length) = content_length {
        if content_length > max_response_bytes {
            body_truncated = true;
            response_bytes_observed = content_length;
        }
    }

    Ok(HttpResponseBodyBytes {
        stored_bytes,
        response_file_bytes,
        response_bytes_observed,
        body_truncated,
    })
}

async fn apply_request_body(
    request: reqwest::RequestBuilder,
    client: &Client,
    node: &CompiledNode,
    resolved_inputs: &Map<String, Value>,
    body_type: &str,
    max_response_bytes: u64,
) -> Result<reqwest::RequestBuilder> {
    match body_type {
        "none" => Ok(request),
        "json" => Ok(request
            .header(CONTENT_TYPE, "application/json")
            .body(string_input(resolved_inputs.get("body")))),
        "raw" => Ok(request.body(string_input(resolved_inputs.get("body")))),
        "x-www-form-urlencoded" => {
            let values = resolved_object(resolved_inputs.get("urlencoded"));
            Ok(request.form(&string_pairs(&values)))
        }
        "form-data" => {
            let form =
                build_multipart_form(client, node, resolved_inputs, max_response_bytes).await?;
            Ok(request.multipart(form))
        }
        "binary" => {
            let file_value = resolved_inputs
                .get("binary")
                .ok_or_else(|| anyhow!("binary body requires a file selector"))?;
            let file = first_file_bytes(client, file_value, max_response_bytes).await?;
            Ok(request
                .header(CONTENT_TYPE, file.descriptor.mimetype)
                .body(file.bytes))
        }
        other => bail!("unsupported HTTP body_type: {other}"),
    }
}

async fn build_multipart_form(
    client: &Client,
    node: &CompiledNode,
    resolved_inputs: &Map<String, Value>,
    max_response_bytes: u64,
) -> Result<Form> {
    let mut form = Form::new();
    let resolved_form_data = resolved_object(resolved_inputs.get("form_data"));
    let binding_entries = node
        .bindings
        .get("form_data")
        .and_then(|binding| binding.raw_value.as_array())
        .cloned()
        .unwrap_or_default();

    for entry in binding_entries {
        let Some(name) = entry.get("name").and_then(Value::as_str) else {
            continue;
        };
        let value = resolved_form_data.get(name).cloned().unwrap_or(Value::Null);
        if entry.get("valueType").and_then(Value::as_str) == Some("file") {
            for file in file_bytes_from_value(client, &value, max_response_bytes).await? {
                let part = Part::bytes(file.bytes)
                    .file_name(file.descriptor.filename)
                    .mime_str(&file.descriptor.mimetype)
                    .context("multipart file mimetype is invalid")?;
                form = form.part(name.to_string(), part);
            }
        } else {
            form = form.text(name.to_string(), value_to_string(&value));
        }
    }

    Ok(form)
}

async fn first_file_bytes(
    client: &Client,
    value: &Value,
    max_response_bytes: u64,
) -> Result<HttpFileBytes> {
    file_bytes_from_value(client, value, max_response_bytes)
        .await?
        .into_iter()
        .next()
        .ok_or_else(|| anyhow!("file selector resolved to no files"))
}

async fn file_bytes_from_value(
    client: &Client,
    value: &Value,
    max_response_bytes: u64,
) -> Result<Vec<HttpFileBytes>> {
    let descriptors = if let Some(items) = value.as_array() {
        items
            .iter()
            .map(HttpFileDescriptor::from_value)
            .collect::<Result<Vec<_>>>()?
    } else {
        vec![HttpFileDescriptor::from_value(value)?]
    };
    let mut files = Vec::with_capacity(descriptors.len());

    for descriptor in descriptors {
        let url = descriptor
            .url
            .as_deref()
            .ok_or_else(|| anyhow!("file descriptor {} has no url", descriptor.filename))?;
        let response = client
            .get(url)
            .send()
            .await
            .with_context(|| format!("failed to fetch file {}", descriptor.filename))?;
        if !response.status().is_success() {
            bail!(
                "failed to fetch file {}: HTTP {}",
                descriptor.filename,
                response.status().as_u16()
            );
        }
        if response.content_length().unwrap_or(0) > max_response_bytes {
            bail!("file {} exceeds max_response_bytes", descriptor.filename);
        }
        let bytes = response
            .bytes()
            .await
            .with_context(|| format!("failed to read file {}", descriptor.filename))?;
        if bytes.len() as u64 > max_response_bytes {
            bail!("file {} exceeds max_response_bytes", descriptor.filename);
        }
        files.push(HttpFileBytes {
            descriptor,
            bytes: bytes.to_vec(),
        });
    }

    Ok(files)
}

async fn response_files(
    node: &CompiledNode,
    content_type: &str,
    bytes: &[u8],
    body_truncated: bool,
    file_persister: Option<&dyn HttpResponseFilePersister>,
) -> Result<Value> {
    let store_response_as_file = node
        .config
        .get("store_response_as_file")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    if bytes.is_empty()
        || body_truncated
        || !store_response_as_file
        || response_body_is_inline_text(content_type)
    {
        return Ok(json!([]));
    }

    let filename = "response.bin";
    if let Some(file_persister) = file_persister {
        let record = file_persister
            .persist_http_response_file(HttpResponseFilePersistInput {
                node_id: &node.node_id,
                filename,
                content_type,
                bytes,
            })
            .await?;
        return Ok(json!([record]));
    }

    Ok(json!([
        {
            "filename": filename,
            "extname": "bin",
            "size": bytes.len(),
            "mimetype": content_type,
            "path": format!("inline://http_request/{}/{}", node.node_id, filename),
            "storage_id": INLINE_RESPONSE_STORAGE_ID,
            "meta": {
                "source": "http_request_response",
                "persisted": false
            }
        }
    ]))
}

fn response_body_is_inline_text(content_type: &str) -> bool {
    let normalized = content_type.to_ascii_lowercase();

    normalized.starts_with("text/")
        || normalized.contains("json")
        || normalized.contains("xml")
        || normalized.contains("form-urlencoded")
        || normalized.contains("javascript")
        || normalized.contains("ecmascript")
}

fn build_header_map(headers: &Map<String, Value>) -> Result<HeaderMap> {
    let mut header_map = HeaderMap::new();

    for (name, value) in headers {
        let header_name = HeaderName::from_bytes(name.as_bytes())
            .with_context(|| format!("invalid HTTP header name: {name}"))?;
        let header_value = HeaderValue::from_str(&value_to_string(value))
            .with_context(|| format!("invalid HTTP header value for {name}"))?;
        header_map.insert(header_name, header_value);
    }

    Ok(header_map)
}

fn headers_to_json(headers: &HeaderMap) -> Value {
    let mut grouped = BTreeMap::<String, Vec<String>>::new();

    for (name, value) in headers {
        grouped
            .entry(name.as_str().to_ascii_lowercase())
            .or_default()
            .push(value.to_str().unwrap_or_default().to_string());
    }

    json!(grouped)
}

fn string_pairs(values: &Map<String, Value>) -> Vec<(String, String)> {
    values
        .iter()
        .map(|(key, value)| (key.clone(), value_to_string(value)))
        .collect()
}

fn resolved_object(value: Option<&Value>) -> Map<String, Value> {
    value
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default()
}

fn string_input(value: Option<&Value>) -> String {
    value.map(value_to_string).unwrap_or_default()
}

fn value_to_string(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

fn config_u64(config: &Value, key: &str, fallback: u64) -> u64 {
    config
        .get(key)
        .and_then(Value::as_u64)
        .filter(|value| *value > 0)
        .unwrap_or(fallback)
}

fn required_string<'a>(object: &'a Map<String, Value>, key: &str) -> Result<&'a str> {
    object
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| anyhow!("file descriptor missing {key}"))
}

fn http_request_error_payload(message: String) -> Value {
    json!({
        "kind": "http_request_error",
        "message": message,
    })
}
