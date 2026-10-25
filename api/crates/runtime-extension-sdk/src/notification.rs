//! Notification handlers reuse the capability envelope and the shared worker dispatcher.
use extension_contracts::{
    CapabilityStdioError, CapabilityStdioMethod, CapabilityStdioRequest, CapabilityStdioResponse,
    NotificationOperation, NotificationRuntimeInput, NotificationRuntimeResult,
    NotificationSendOutcome, NotificationTypeDescriptor,
};
use serde_json::Value;

fn failed(message: &str) -> CapabilityStdioResponse {
    CapabilityStdioResponse {
        ok: false,
        result: Value::Null,
        error: Some(CapabilityStdioError {
            message: message.into(),
            provider_summary: None,
        }),
    }
}

/// Validate managed receive/send input before provider I/O. This helper owns no stdio transport.
pub fn prepare_authorizable_notification(
    request: CapabilityStdioRequest,
    descriptor: &NotificationTypeDescriptor,
) -> Result<(Value, NotificationRuntimeInput), Box<CapabilityStdioResponse>> {
    if request.method != CapabilityStdioMethod::Execute {
        return Err(Box::new(failed("notification requires the execute method")));
    }
    if descriptor.validate().is_err() {
        return Err(Box::new(failed(
            "notification descriptor violates its contract",
        )));
    }
    let config = request.input["config_payload"].clone();
    let input: NotificationRuntimeInput =
        serde_json::from_value(request.input["input_payload"].clone())
            .map_err(|_| Box::new(failed("notification operation violates its contract")))?;
    if descriptor.validate_config(&config).is_err() || input.validate(descriptor).is_err() {
        return Err(Box::new(failed(
            "notification configuration or operation violates its contract",
        )));
    }
    Ok((config, input))
}

/// Complete a provider operation on the existing capability envelope. Provider errors, invalid
/// responses or output loss after a send are ambiguous and must remain unknown. Patches are
/// Host-only; the Host validates again, encrypts them and projects the public authorization DTO.
pub fn complete_authorizable_notification<E>(
    input: &NotificationRuntimeInput,
    descriptor: &NotificationTypeDescriptor,
    result: Result<NotificationRuntimeResult, E>,
) -> CapabilityStdioResponse {
    let result = match result {
        Ok(result) if result.validate_for(input, descriptor).is_ok() => result,
        _ if matches!(input.operation, NotificationOperation::Send { .. }) => {
            NotificationRuntimeResult::Send {
                outcome: NotificationSendOutcome::unknown(),
                authorization: None,
                managed_secret_patch: Default::default(),
            }
        }
        _ => return failed("notification provider operation failed"),
    };
    match serde_json::to_value(result) {
        Ok(result) => CapabilityStdioResponse {
            ok: true,
            result,
            error: None,
        },
        Err(_) => failed("notification response violates its contract"),
    }
}

/// Async supplier callback reusable inside `serve`/`serve_duplex` capability dispatch.
/// The supplier implements QR/provider I/O, never a second worker framing or dispatcher.
pub async fn execute_authorizable_notification<E, F>(
    request: CapabilityStdioRequest,
    descriptor: &NotificationTypeDescriptor,
    handler: impl FnOnce(Value, NotificationRuntimeInput) -> F,
) -> CapabilityStdioResponse
where
    F: std::future::Future<Output = Result<NotificationRuntimeResult, E>>,
{
    let (config, input) = match prepare_authorizable_notification(request, descriptor) {
        Ok(validated) => validated,
        Err(response) => return *response,
    };
    let result = handler(config, input.clone()).await;
    complete_authorizable_notification(&input, descriptor, result)
}

/// Apply one provider send without owning transport, workspace policy or process lifecycle.
/// Provider errors are projected to a safe message; configuration never enters a response.
pub fn execute_notification<E>(
    request: CapabilityStdioRequest,
    descriptor: &NotificationTypeDescriptor,
    handler: impl FnOnce(&Value, &Value) -> Result<Value, E>,
) -> CapabilityStdioResponse {
    if request.method != CapabilityStdioMethod::Execute {
        return failed("notification requires the execute method");
    }
    let config = &request.input["config_payload"];
    let input = &request.input["input_payload"];
    if descriptor.validate_config(config).is_err() || descriptor.validate_input(input).is_err() {
        return failed("notification configuration or input violates its schema");
    }
    match handler(config, input) {
        Ok(result) if descriptor.validate_output(&result).is_ok() => CapabilityStdioResponse {
            ok: true,
            result,
            error: None,
        },
        Ok(_) => failed("notification output violates its schema"),
        Err(_) => failed("notification provider send failed"),
    }
}

/// Canonical communications entrypoint for receive, attachment download, send and authorization.
/// Uses the shared bounded dispatcher; a handler must not commit/ack a receive cursor itself.
/// `config` and the trusted runtime envelope are validated before any provider I/O.
pub async fn execute_communication<E, F>(
    request: CapabilityStdioRequest,
    descriptor: &NotificationTypeDescriptor,
    handler: impl FnOnce(Value, NotificationRuntimeInput) -> F,
) -> CapabilityStdioResponse
where
    F: std::future::Future<Output = Result<NotificationRuntimeResult, E>>,
{
    execute_authorizable_notification(request, descriptor, handler).await
}
