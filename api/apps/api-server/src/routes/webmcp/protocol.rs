use super::interface::{WebMcpAuthorization, WebMcpInput, WebMcpOutput, WebMcpTargetError};
use crate::{
    app_state::ApiState, error_response::ApiError, extension_bus::ConsoleAuthenticationCredential,
    response::ApiSuccess,
};
use axum::{
    response::{IntoResponse, Response},
    Json,
};
use interface_runtime::{
    BindingId, InterfaceInvocationError, InterfaceInvocationKernel, InterfaceProtocol,
    UserPrincipal,
};
use std::sync::Arc;

pub(super) async fn invoke(
    state: Arc<ApiState>,
    binding: &'static str,
    credential: ConsoleAuthenticationCredential,
    input: WebMcpInput,
) -> Result<Response, ApiError> {
    let boot = state
        .extension_boot_snapshot
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("WebMCP boot snapshot unavailable"))?;
    let snapshot = boot
        .interface_registry()
        .ok_or_else(|| anyhow::anyhow!("WebMCP registry unavailable"))?
        .snapshot();
    let binding_id = BindingId::new(binding).expect("static WebMCP binding");
    let _activated = snapshot
        .authentication(&binding_id)
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("WebMCP authentication unavailable"))?;
    let authenticated = boot
        .authenticate_invocation::<_, UserPrincipal>(
            Arc::clone(&snapshot),
            &binding_id,
            InterfaceProtocol::Http,
            credential,
        )
        .await
        .map_err(ApiError::from)?;
    let kernel = InterfaceInvocationKernel::new(Arc::new(WebMcpAuthorization));
    let result = kernel
        .invoke::<WebMcpInput, WebMcpOutput, WebMcpTargetError>(
            snapshot,
            authenticated.into_envelope(input),
        )
        .await;
    let (mut response, receipt) = match result {
        Ok(outcome) => {
            let receipt = outcome.receipt().clone().projected();
            let response = match outcome.into_value() {
                WebMcpOutput::Registrations(registrations) => {
                    Json(ApiSuccess::new(registrations)).into_response()
                }
                WebMcpOutput::Tool(tool) => Json(ApiSuccess::new(tool)).into_response(),
            };
            (response, receipt)
        }
        Err(failure) => {
            let receipt = failure.receipt().clone().projected();
            (
                invocation_error(failure.into_error()).into_response(),
                receipt,
            )
        }
    };
    // Server-side observation only; wire DTOs and headers remain the WebMCP contract.
    response.extensions_mut().insert(receipt);
    Ok(response)
}

fn invocation_error(error: InterfaceInvocationError) -> ApiError {
    match error {
        InterfaceInvocationError::TargetFailed(error) => error
            .into_source::<WebMcpTargetError>()
            .map(|error| error.0)
            .unwrap_or_else(|| anyhow::anyhow!("WebMCP target contract mismatch").into()),
        InterfaceInvocationError::AuthorizationRejected(error) => error
            .into_source::<ApiError>()
            .unwrap_or_else(|| anyhow::anyhow!("WebMCP authorization failed").into()),
        error => anyhow::anyhow!("WebMCP invocation failed: {error}").into(),
    }
}
