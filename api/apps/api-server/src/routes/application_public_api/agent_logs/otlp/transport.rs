use super::*;
use axum::{
    extract::{DefaultBodyLimit, State},
    handler::Handler as AxumHandler,
    http::HeaderMap,
    response::{IntoResponse, Response},
};
use prost::Message;

pub struct OtlpError {
    error: ApiError,
    format: WireFormat,
    override_status: Option<StatusCode>,
}
impl OtlpError {
    fn new(error: ApiError, format: WireFormat) -> Self {
        let override_status = match error.0.downcast_ref::<ControlPlaneError>() {
            Some(ControlPlaneError::InvalidInput(
                "agent_logs_otlp_content_type" | "agent_logs_otlp_content_encoding",
            )) => Some(StatusCode::UNSUPPORTED_MEDIA_TYPE),
            _ => None,
        };
        Self {
            error,
            format,
            override_status,
        }
    }
}
impl IntoResponse for OtlpError {
    fn into_response(self) -> Response {
        // Reuse existing HTTP error semantics, but never copy the error body,
        // raw provider text or credentials into the protocol Status message.
        let message = match self.error.0.downcast_ref::<ControlPlaneError>() {
            Some(ControlPlaneError::NotAuthenticated) => "not_authenticated",
            Some(
                ControlPlaneError::PermissionDenied(reason)
                | ControlPlaneError::Conflict(reason)
                | ControlPlaneError::InvalidInput(reason)
                | ControlPlaneError::NotFound(reason),
            ) => *reason,
            _ => "internal_error",
        };
        let status = self
            .override_status
            .unwrap_or_else(|| self.error.into_response().status());
        let rpc = tonic_types::pb::Status {
            code: 0,
            message: message.into(),
            details: vec![],
        };
        let bytes = match self.format {
            WireFormat::Protobuf => rpc.encode_to_vec(),
            // Omit proto default fields; no arbitrary Any details are emitted.
            _ => serde_json::to_vec(&serde_json::json!({"message":rpc.message}))
                .expect("fixed RPC status serialization"),
        };
        (
            status,
            [(axum::http::header::CONTENT_TYPE, self.format.content_type())],
            bytes,
        )
            .into_response()
    }
}

/// Ingest OTLP Trace observations.
/// Accepts standard OTLP/HTTP JSON or protobuf with optional gzip using the application's Bearer key; full success is the standard OTLP acknowledgement, and partial acceptance reports rejected input spans.
#[utoipa::path(post,path="/api/logs/otlp/v1/traces",request_body(content((serde_json::Value = "application/json"),([u8] = "application/x-protobuf"))),responses((status=200,description="Standard ExportTraceServiceResponse; partial_success is absent only when every source batch completed",content((serde_json::Value = "application/json"),([u8] = "application/x-protobuf"))),(status=400,description="Invalid OTLP data; standard google.rpc.Status"),(status=401,description="Missing or invalid application API key; standard google.rpc.Status"),(status=403,description="Log reception disabled or application type does not support ingestion; standard google.rpc.Status"),(status=409,description="Immutable source event conflicts with an existing receipt; standard google.rpc.Status"),(status=415,description="Unsupported content type or encoding; standard google.rpc.Status")),security(("applicationApiKey"=[])))]
pub async fn ingest_traces(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, OtlpError> {
    let format = WireFormat::from_header(
        headers
            .get(axum::http::header::CONTENT_TYPE)
            .and_then(|header| header.to_str().ok()),
    );
    let run = async {
        let token = headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|header| header.to_str().ok())
            .and_then(|header| header.strip_prefix("Bearer "))
            .ok_or(ControlPlaneError::NotAuthenticated)?
            .to_owned();
        let boot = state
            .extension_boot_snapshot
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("interface_registry_unavailable"))?;
        let snapshot = boot
            .interface_registry()
            .ok_or_else(|| anyhow::anyhow!("interface_registry_unavailable"))?
            .snapshot();
        let binding = BindingId::new(BINDING).expect("static OTLP trace binding is valid");
        let authenticated = boot
            .authenticate_invocation::<_, ApplicationPrincipal>(
                Arc::clone(&snapshot),
                &binding,
                InterfaceProtocol::Http,
                crate::extension_bus::ApplicationApiKeyAuthenticationCredential {
                    state: Arc::clone(&state),
                    bearer_token: token,
                },
            )
            .await?;
        let encoding = match headers.get(axum::http::header::CONTENT_ENCODING) {
            Some(header) => header
                .to_str()
                .map(|value| ContentEncoding::from_header(Some(value)))
                .unwrap_or(ContentEncoding::Unsupported),
            None => ContentEncoding::Identity,
        };
        let result = InterfaceInvocationKernel::new(Arc::new(interface::Authorization))
            .invoke::<Input, Output, TargetError>(
                snapshot,
                authenticated.into_envelope(Input {
                    format,
                    encoding,
                    body,
                }),
            )
            .await
            .map_err(|failure| match failure.into_error() {
                InterfaceInvocationError::TargetFailed(error) => error
                    .into_source::<TargetError>()
                    .map(|error| error.0)
                    .unwrap_or_else(|| anyhow::anyhow!("agent_logs_otlp_target_failed").into()),
                _ => anyhow::anyhow!("agent_logs_otlp_invocation_failed").into(),
            })?;
        let response = result.into_value().response;
        let bytes = match format {
            WireFormat::Protobuf => response.encode_to_vec(),
            _ if response.partial_success.is_none() => b"{}".to_vec(),
            _ => serde_json::to_vec(&response).map_err(ApiError::from)?,
        };
        Ok((
            StatusCode::OK,
            [(axum::http::header::CONTENT_TYPE, format.content_type())],
            bytes,
        )
            .into_response())
    }
    .await;
    run.map_err(|error: ApiError| OtlpError::new(error, format))
}

pub(crate) fn route_assembly(
) -> crate::external_route_assembly::ExternalRouteAssembly<Arc<ApiState>> {
    crate::external_route_assembly::ExternalRouteAssembly::new().route(
        ROUTE,
        crate::external_route_assembly::post(ingest_traces.layer(DefaultBodyLimit::disable())),
    )
}
