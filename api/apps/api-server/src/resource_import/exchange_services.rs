//! Composition adapters for the two fixed, caller-bound Host service families.
use super::ApiFileImportPort;
use base64::Engine;
use extension_contracts::{
    ApplicationLogsRequest, ApplicationLogsResponse, HostServiceError, HttpEgressRequest,
    HttpEgressResponse,
};
use runtime_core::runtime_backend::{
    ApplicationLogsBinding, ApplicationLogsPort, HostServiceFuture, HttpEgressBinding,
    HttpEgressPort,
};

impl ApiFileImportPort {
    async fn exchange_caller(
        &self,
        binding: &ApplicationLogsBinding,
        permission: &str,
    ) -> Result<
        (
            std::sync::Arc<crate::app_state::ApiState>,
            domain::ActorContext,
            Box<dyn control_plane_contracts::ports::ContributionAuthorityLease>,
        ),
        HostServiceError,
    > {
        let state = self
            .current_state()
            .ok_or_else(|| HostServiceError::new("host_service_unavailable"))?;
        let (actor, _) = self
            .current_caller(&state, binding)
            .await
            .map_err(|_| HostServiceError::new("host_service_caller_invalid"))?;
        let lease = control_plane::file_management::authorize_file_service(
            &state.store,
            binding,
            permission,
        )
        .await
        .map_err(|_| HostServiceError::new("host_service_authority_denied"))?;
        Ok((state, actor, lease))
    }
}
impl ApplicationLogsPort for ApiFileImportPort {
    fn execute<'a>(
        &'a self,
        binding: &'a ApplicationLogsBinding,
        request: &'a ApplicationLogsRequest,
    ) -> HostServiceFuture<'a, ApplicationLogsResponse> {
        Box::pin(async move {
            request.validate()?;
            let (state, actor, lease) = self.exchange_caller(binding, request.permission()).await?;
            let result =
                crate::application_log_exchange::execute(&state, &actor, request.clone()).await;
            let result = match result {
                Ok(value) => Ok(value),
                Err(error) => Err(safe_api_error(error).await),
            };
            finish_authority(result, lease.release().await)
        })
    }
}
async fn safe_api_error(error: crate::error_response::ApiError) -> HostServiceError {
    use axum::response::IntoResponse;
    let response = error.into_response();
    let status = response.status().as_u16();
    // Use the API's established safe projection, retaining partial import receipts.
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .ok();
    let value: serde_json::Value = body
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default();
    HostServiceError {
        code: value
            .get("code")
            .and_then(|v| v.as_str())
            .unwrap_or("application_logs_failed")
            .into(),
        status: Some(status),
        details: value.get("details").filter(|v| !v.is_null()).cloned(),
    }
}
impl HttpEgressPort for ApiFileImportPort {
    fn execute<'a>(
        &'a self,
        binding: &'a HttpEgressBinding,
        request: &'a HttpEgressRequest,
    ) -> HostServiceFuture<'a, HttpEgressResponse> {
        Box::pin(async move {
            request.validate()?;
            let (state, workspace_id, lease) = if let Some(caller) = &binding.caller {
                let caller_binding = runtime_core::runtime_backend::FileImportBinding {
                    subject: binding.subject.clone(),
                    publisher_namespace: binding.publisher_namespace.clone(),
                    plugin_code: binding.plugin_code.clone(),
                    plugin_version: binding.plugin_version.clone(),
                    caller: caller.clone(),
                    deadline_unix_ms: binding.deadline_unix_ms,
                };
                let (state, actor, lease) = self
                    .exchange_caller(&caller_binding, "network.http.request")
                    .await?;
                (state, actor.current_workspace_id, lease)
            } else {
                let state = self
                    .current_state()
                    .ok_or_else(|| HostServiceError::new("host_service_unavailable"))?;
                let lease = control_plane::plugin_management::authorize_authentication_egress(
                    &state.store,
                    binding,
                )
                .await?;
                (state, domain::SYSTEM_SCOPE_ID, lease)
            };
            let result = http_request(&state, workspace_id, binding, request).await;
            finish_authority(result, lease.release().await)
        })
    }
}
async fn http_request(
    state: &crate::app_state::ApiState,
    workspace_id: uuid::Uuid,
    binding: &HttpEgressBinding,
    request: &HttpEgressRequest,
) -> Result<HttpEgressResponse, HostServiceError> {
    let remaining = binding.deadline_unix_ms.saturating_sub(
        (time::OffsetDateTime::now_utc().unix_timestamp_nanos() / 1_000_000) as i64,
    );
    if remaining <= 0 {
        return Err(HostServiceError::new("host_service_deadline_exceeded"));
    }
    let method = reqwest::Method::from_bytes(request.method.as_bytes())
        .map_err(|_| HostServiceError::new("http_egress_method_invalid"))?;
    let body = request
        .body_base64
        .as_ref()
        .map(|value| base64::engine::general_purpose::STANDARD.decode(value))
        .transpose()
        .map_err(|_| HostServiceError::new("http_egress_body_invalid"))?;
    let installation_id = uuid::Uuid::parse_str(binding.subject.installation_id().as_str())
        .map_err(|_| HostServiceError::new("host_service_authority_denied"))?;
    let timeout = (binding.deadline_unix_ms != i64::MAX)
        .then(|| std::time::Duration::from_millis(remaining as u64));
    let scope = state
        .network_egress_http_clients()
        .resolve_managed_plugin_request(workspace_id, installation_id, timeout)
        .await
        .map_err(|_| HostServiceError::new("http_egress_unavailable"))?;
    let result: anyhow::Result<HttpEgressResponse> = async {
        let mut builder = scope.http_client().request(method, &request.url);
        for (name, value) in &request.headers {
            builder = builder.header(name.as_str(), value.as_str());
        }
        if let Some(body) = body {
            builder = builder.body(body);
        }
        let mut response = builder.send().await?;
        let status = response.status().as_u16();
        let headers: std::collections::BTreeMap<String, String> = response
            .headers()
            .iter()
            .map(|(name, value)| Ok((name.to_string(), value.to_str()?.to_owned())))
            .collect::<anyhow::Result<_>>()?;
        let mut body = Vec::new();
        while let Some(chunk) = response.chunk().await? {
            body.extend_from_slice(&chunk);
        }
        Ok(HttpEgressResponse {
            status,
            headers,
            body_base64: base64::engine::general_purpose::STANDARD.encode(body),
        })
    }
    .await;
    scope
        .finish(result)
        .await
        .map_err(|_| HostServiceError::new("http_egress_request_failed"))
}

#[cfg(test)]
#[path = "../_tests/exchange_host_service_errors.rs"]
mod exchange_host_service_errors;

// Cleanup failure must never hide a business error or a persisted partial receipt.
fn finish_authority<T>(
    result: Result<T, HostServiceError>,
    released: anyhow::Result<()>,
) -> Result<T, HostServiceError> {
    match (result, released) {
        (Err(error), _) => Err(error),
        (Ok(value), Ok(())) => Ok(value),
        (Ok(_), Err(_)) => Err(HostServiceError::new("host_service_authority_unavailable")),
    }
}
