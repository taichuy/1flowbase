use super::*;
use crate::execution_engine::{ProviderInvoker, WorkflowInternalHttpRequest};

#[cfg(test)]
#[path = "_tests/workflow_credentials.rs"]
mod tests;

pub(super) fn credential_reference(inputs: &Map<String, Value>) -> Option<String> {
    let headers = resolved_object(inputs.get("headers"));
    headers.iter().find_map(|(name, value)| {
        if !name.eq_ignore_ascii_case("authorization") {
            return None;
        }
        let value = value.as_str()?.trim();
        let reference = value.strip_prefix("Bearer ").unwrap_or(value);
        reference
            .starts_with("wfcred:")
            .then(|| reference.to_owned())
    })
}

pub(super) async fn execute<I: ProviderInvoker + ?Sized>(
    node: &CompiledNode,
    inputs: &Map<String, Value>,
    pool: &Map<String, Value>,
    file_persister: Option<&dyn HttpResponseFilePersister>,
    invoker: Option<&I>,
    reference: &str,
) -> Result<HttpRequestNodeExecution> {
    let invoker = invoker.ok_or_else(|| anyhow!("workflow credential host is not configured"))?;
    let template = node
        .config
        .get("url")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("HTTP URL is required"))?;
    let path = render_template(template, pool)?;
    if !path.starts_with("/api/console/") || path.contains(['\\', '#']) {
        bail!("workflow credentials require a relative /api/console/ URL");
    }
    let mut url = reqwest::Url::parse(&format!("http://workflow.internal{path}"))?;
    let params = resolved_object(inputs.get("params"));
    if !params.is_empty() {
        url.query_pairs_mut().extend_pairs(string_pairs(&params));
    }
    let mut headers = string_pairs(&resolved_object(inputs.get("headers")));
    // The reference selects a host-owned principal; never forward it as a
    // network credential or let it compete with the Console authentication.
    headers.retain(|(name, _)| !name.eq_ignore_ascii_case("authorization"));
    let body_type = node
        .config
        .get("body_type")
        .and_then(Value::as_str)
        .unwrap_or("none");
    let body = match body_type {
        "none" => Vec::new(),
        "json" => {
            headers.retain(|(key, _)| !key.eq_ignore_ascii_case("content-type"));
            headers.push(("Content-Type".into(), "application/json".into()));
            string_input(inputs.get("body")).into_bytes()
        }
        "raw" => string_input(inputs.get("body")).into_bytes(),
        "x-www-form-urlencoded" => {
            let mut encoded = reqwest::Url::parse("http://workflow.internal/")?;
            encoded
                .query_pairs_mut()
                .extend_pairs(string_pairs(&resolved_object(inputs.get("urlencoded"))));
            headers.retain(|(key, _)| !key.eq_ignore_ascii_case("content-type"));
            headers.push((
                "Content-Type".into(),
                "application/x-www-form-urlencoded".into(),
            ));
            encoded.query().unwrap_or("").as_bytes().to_vec()
        }
        _ => bail!("workflow credential HTTP body type is not supported"),
    };
    let max_response_bytes = config_u64(
        &node.config,
        "max_response_bytes",
        DEFAULT_MAX_RESPONSE_BYTES,
    )
    .min(MAX_RESPONSE_BYTES_LIMIT) as usize;
    let method = node
        .config
        .get("method")
        .and_then(Value::as_str)
        .unwrap_or("GET")
        .to_uppercase();
    Method::from_bytes(method.as_bytes()).context("HTTP method is invalid")?;
    let request = WorkflowInternalHttpRequest {
        method: method.clone(),
        url: match url.query() {
            Some(query) => format!("{}?{query}", url.path()),
            None => url.path().to_owned(),
        },
        headers,
        body,
        max_response_bytes,
    };
    let timeout = Duration::from_millis(config_u64(&node.config, "timeout_ms", DEFAULT_TIMEOUT_MS));
    let response = tokio::time::timeout(
        timeout,
        invoker.invoke_workflow_internal_http(reference, request),
    )
    .await
    .context("workflow credential HTTP request timed out")??;
    if response.body.len() > max_response_bytes {
        bail!("workflow HTTP response exceeds byte limit");
    }
    let response_headers: Map<String, Value> = response
        .headers
        .iter()
        .map(|(name, value)| (name.to_lowercase(), Value::String(value.clone())))
        .collect();
    let content_type = response_headers
        .get("content-type")
        .and_then(Value::as_str)
        .unwrap_or("application/octet-stream");
    let files = response_files(node, content_type, &response.body, false, file_persister).await?;
    Ok(HttpRequestNodeExecution {
        output_payload: json!({"status_code":response.status,"body":String::from_utf8_lossy(&response.body),"headers":response_headers,"files":files}),
        error_payload: None,
        metrics_payload: json!({"preview_mode":true,"status_code":response.status,"response_bytes":response.body.len(),"body_truncated":false,"error":false}),
        debug_payload: json!({"request":{"method":method,"url":path,"body_type":body_type},"response":{"status_code":response.status}}),
    })
}
