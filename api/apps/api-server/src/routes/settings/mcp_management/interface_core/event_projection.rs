use super::*;
use crate::openapi_interface::contract::{self as event, ApiInput, ApiOperationContext, ApiOutput};
use serde_json::{json, Value};
use std::sync::Arc;

#[derive(serde::Serialize, utoipa::ToSchema)]
struct CredentialEvent {
    saved: bool,
}
impl ApiInput for McpCoreInput {
    fn document_input_schema(operation: &ApiOperationContext) -> Option<Value> {
        (operation.interface_id == "mcp.client_credential.save").then(|| json!({"type":"object","properties":{"path":{"type":"object","properties":{"instance_id":{"type":"string"}},"required":["instance_id"]},"body":{"type":"object","properties":{},"additionalProperties":false}},"required":["path","body"],"additionalProperties":false}))
    }
    fn project_document_input(&self, _: &ApiOperationContext) -> Result<Value, Arc<str>> {
        Ok(match self {
            Self::GetCredential(instance_id)
            | Self::DeleteCredential(instance_id)
            | Self::DeleteInstance(instance_id)
            | Self::GetDiscoveryPolicy(instance_id) => json!({"path":{"instance_id":instance_id}}),
            Self::SaveCredential(instance_id, _) => {
                json!({"path":{"instance_id":instance_id},"body":{}})
            }
            Self::ListInstances => json!({}),
            Self::CreateInstance(body) => json!({"body":event::to_value(body)?}),
            Self::CopyInstance(instance_id, body) => {
                json!({"path":{"instance_id":instance_id},"body":event::to_value(body)?})
            }
            Self::UpdateInstance(instance_id, body) => {
                json!({"path":{"instance_id":instance_id},"body":event::to_value(body)?})
            }
            Self::UpsertGroup(instance_id, body) => {
                json!({"path":{"instance_id":instance_id},"body":event::to_value(body)?})
            }
            Self::MoveGroup(instance_id, body) => {
                json!({"path":{"instance_id":instance_id},"body":event::to_value(body)?})
            }
            Self::DeleteGroup(instance_id, query) => {
                json!({"path":{"instance_id":instance_id},"query":event::to_value(query)?})
            }
            Self::CreateBinding(instance_id, body) => {
                json!({"path":{"instance_id":instance_id},"body":event::to_value(body)?})
            }
            Self::UpdateBinding(binding_id, body) => {
                json!({"path":{"binding_id":binding_id},"body":event::to_value(body)?})
            }
            Self::DeleteBinding(binding_id) => json!({"path":{"binding_id":binding_id}}),
            Self::UpdateDiscoveryPolicy(instance_id, body) => {
                json!({"path":{"instance_id":instance_id},"body":event::to_value(body)?})
            }
        })
    }
}
impl ApiOutput for McpCoreOutput {
    fn document_output_schema(operation: &ApiOperationContext) -> Option<Value> {
        matches!(
            operation.interface_id.as_str(),
            "mcp.client_credential.reveal" | "mcp.client_credential.save"
        )
        .then(|| event::schema::<CredentialEvent>())
    }
    fn project_document_output(&self, _: &ApiOperationContext) -> Result<Value, Arc<str>> {
        match self {
            Self::Credential(value) => event::to_value(&CredentialEvent { saved: value.saved }),
            Self::Instances(value) => event::to_value(value),
            Self::Instance(value) => event::to_value(value),
            Self::Group(value) => event::to_value(value),
            Self::Binding(value) => event::to_value(value),
            Self::DiscoveryPolicy(value) => event::to_value(value),
            Self::NoContent => Ok(Value::Null),
        }
    }
}

#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
#[path = "_tests/event_projection.rs"]
mod event_tests;
