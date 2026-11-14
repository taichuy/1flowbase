use super::*;
use crate::openapi_interface::contract::{self as event, ApiInput, ApiOperationContext, ApiOutput};
use serde_json::{json, Value};
use std::sync::Arc;

#[derive(serde::Serialize, utoipa::ToSchema)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
enum AuthOperationEvent {
    Begin { action: String },
    Poll,
    Submit,
    Cancel,
    Maintain,
}
fn auth_operation(value: &Value) -> Result<AuthOperationEvent, Arc<str>> {
    let operation = serde_json::from_value::<ProviderAuthOperation>(value.clone())
        .map_err(|_| Arc::<str>::from("invalid provider authentication operation event view"))?;
    Ok(match operation {
        ProviderAuthOperation::Begin { action } => AuthOperationEvent::Begin { action },
        ProviderAuthOperation::Poll => AuthOperationEvent::Poll,
        ProviderAuthOperation::Submit { value: _ } => AuthOperationEvent::Submit,
        ProviderAuthOperation::Cancel => AuthOperationEvent::Cancel,
        ProviderAuthOperation::Maintain => AuthOperationEvent::Maintain,
    })
}
#[derive(serde::Serialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
struct AuthInputEvent {
    operation: AuthOperationEvent,
}
#[derive(serde::Serialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
struct PreviewInputEvent<'a> {
    installation_id: &'a Option<String>,
    instance_id: &'a Option<String>,
}
#[derive(serde::Serialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
struct UserActionEvent<'a> {
    kind: &'a str,
    expires_at: &'a Option<String>,
    poll_interval_seconds: Option<u64>,
    prompt: &'a Option<String>,
}
#[derive(serde::Serialize, utoipa::ToSchema)]
struct AuthenticationEvent<'a> {
    instance: &'a ModelProviderInstanceResponse,
    status: &'a str,
    message: &'a Option<String>,
    user_action: Option<UserActionEvent<'a>>,
}
#[derive(serde::Serialize, utoipa::ToSchema)]
struct PreviewEvent<'a> {
    models: &'a [ProviderModelDescriptorResponse],
    expires_at: &'a str,
}
#[derive(serde::Serialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
struct SecretEvent<'a> {
    key: &'a str,
}
impl ApiInput for ProviderInstanceOperationsInput {
    fn document_input_schema(operation: &ApiOperationContext) -> Option<Value> {
        match operation.interface_id.as_str() {
            "model_providers.instances.authenticate" => Some(
                json!({"type":"object","properties":{"path":{"type":"object","properties":{"id":{"type":"string"}},"required":["id"]},"body":event::schema::<AuthInputEvent>()},"required":["path","body"],"additionalProperties":false}),
            ),
            "model_providers.preview.view" => Some(
                json!({"type":"object","properties":{"body":event::schema::<PreviewInputEvent<'static>>()},"required":["body"],"additionalProperties":false}),
            ),
            _ => None,
        }
    }
    fn project_document_input(&self, _: &ApiOperationContext) -> Result<Value, Arc<str>> {
        Ok(match self {
            Self::Authenticate { id, body } => {
                json!({"path":{"id":id},"body":event::to_value(&AuthInputEvent { operation:auth_operation(&body.operation)? })?})
            }
            Self::Usage { id } | Self::ResetCredits { id } | Self::Balance { id } => {
                json!({"path":{"id":id}})
            }
            Self::ConsumeResetCredit { id, body } => {
                json!({"path":{"id":id},"body":event::to_value(body)?})
            }
            Self::Preview(body) => {
                json!({"body":event::to_value(&PreviewInputEvent { installation_id:&body.installation_id, instance_id:&body.instance_id })?})
            }
            Self::Reveal { id, body } => json!({"path":{"id":id},"body":event::to_value(body)?}),
        })
    }
}
impl ApiOutput for ProviderInstanceOperationsOutput {
    fn document_output_schema(operation: &ApiOperationContext) -> Option<Value> {
        match operation.interface_id.as_str() {
            "model_providers.instances.authenticate" => {
                Some(event::schema::<AuthenticationEvent<'static>>())
            }
            "model_providers.preview.view" => Some(event::schema::<PreviewEvent<'static>>()),
            "model_providers.instances.secrets.reveal" => {
                Some(event::schema::<SecretEvent<'static>>())
            }
            _ => None,
        }
    }
    fn project_document_output(&self, _: &ApiOperationContext) -> Result<Value, Arc<str>> {
        match self {
            Self::Authentication(value) => event::to_value(&AuthenticationEvent {
                instance: &value.instance,
                status: &value.status,
                message: &value.message,
                user_action: value.user_action.as_ref().map(|action| UserActionEvent {
                    kind: &action.kind,
                    expires_at: &action.expires_at,
                    poll_interval_seconds: action.poll_interval_seconds,
                    prompt: &action.prompt,
                }),
            }),
            Self::Usage(value) => event::to_value(value),
            Self::ResetCredits(value) => event::to_value(value),
            Self::Consumed(value) => event::to_value(value),
            Self::Balance(value) => event::to_value(value),
            Self::Preview(value) => event::to_value(&PreviewEvent {
                models: &value.models,
                expires_at: &value.expires_at,
            }),
            Self::Secret(value) => event::to_value(&SecretEvent { key: &value.key }),
        }
    }
}

#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
#[path = "_tests/event_projection.rs"]
mod event_tests;
