use super::*;
use crate::openapi_interface::contract::{self as event, ApiInput, ApiOperationContext, ApiOutput};
use serde_json::{json, Value};
use std::sync::Arc;

fn query(mut value: Value) -> Value {
    if let Some(object) = value.as_object_mut() {
        object.retain(|_, value| !value.is_null());
    }
    value
}
#[derive(serde::Serialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
struct CreateEvent<'a> {
    type_code: &'a str,
    installation_id: Option<Uuid>,
    display_name: &'a str,
    status: Option<&'static str>,
}
#[derive(serde::Serialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
struct PatchEvent<'a> {
    display_name: &'a Option<String>,
    status: Option<&'static str>,
}
#[derive(serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
struct DeliveryWindowEvent {
    status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    last_received_at_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    expires_at_ms: Option<u64>,
}
#[derive(serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
struct UserActionEvent {
    kind: String,
    poll_interval_seconds: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    expires_at: Option<String>,
}
#[derive(serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
struct AuthorizationEvent {
    #[serde(skip_serializing_if = "Option::is_none")]
    delivery_window: Option<DeliveryWindowEvent>,
    status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    user_action: Option<UserActionEvent>,
}
#[derive(serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
struct InstanceEvent {
    id: Uuid,
    workspace_id: Uuid,
    type_code: String,
    installation_id: Option<Uuid>,
    display_name: String,
    status: String,
    config_json: Value,
    configured_secret_fields: Vec<String>,
    created_at: String,
    updated_at: String,
    available: bool,
    unavailable_reason: Option<String>,
    authorization: Option<AuthorizationEvent>,
}
fn safe<T: serde::de::DeserializeOwned + serde::Serialize>(
    value: &Value,
) -> Result<Value, Arc<str>> {
    let value: T = serde_json::from_value(value.clone())
        .map_err(|_| Arc::<str>::from("invalid notification public event view"))?;
    event::to_value(&value)
}
impl ApiInput for NotificationInput {
    fn document_input_schema(operation: &ApiOperationContext) -> Option<Value> {
        match operation.interface_id.as_str() {
            "notifications.instances.create" => Some(
                json!({"type":"object","properties":{"body":event::schema::<CreateEvent<'static>>()},"required":["body"],"additionalProperties":false}),
            ),
            "notifications.instances.update" => Some(
                json!({"type":"object","properties":{"path":{"type":"object","properties":{"instance_id":{"type":"string","format":"uuid"}},"required":["instance_id"]},"body":event::schema::<PatchEvent<'static>>()},"required":["path","body"],"additionalProperties":false}),
            ),
            _ => None,
        }
    }
    fn project_document_input(&self, _: &ApiOperationContext) -> Result<Value, Arc<str>> {
        Ok(match self {
            Self::Types | Self::UnreadCount | Self::ReadAll => json!({}),
            Self::List(value) => {
                json!({"query":query(json!({"display_name":value.display_name,"type_code":value.type_code,"status":value.status}))})
            }
            Self::Get(instance_id) | Self::Delete(instance_id) | Self::ReadChannel(instance_id) => {
                json!({"path":{"instance_id":instance_id}})
            }
            // Descriptor-owned config_json is split into public and encrypted
            // fields by the service; the raw Before carrier lacks that descriptor.
            Self::Create(body) => {
                json!({"body":event::to_value(&CreateEvent { type_code:&body.type_code, installation_id:body.installation_id, display_name:&body.display_name, status:body.status.map(|value|value.as_str()) })?})
            }
            Self::Patch(instance_id, body) => {
                json!({"path":{"instance_id":instance_id},"body":event::to_value(&PatchEvent { display_name:&body.display_name,status:body.status.map(|value|value.as_str()) })?})
            }
            Self::Send(instance_id, input) => {
                json!({"path":{"instance_id":instance_id},"body":{"input":input}})
            }
            Self::Authorize(instance_id, operation) => {
                json!({"path":{"instance_id":instance_id},"body":event::to_value(operation)?})
            }
            Self::Messages(value) => {
                json!({"query":query(json!({"cursor":value.cursor,"page_size":value.page_size,"unread_only":value.unread_only,"instance_id":value.instance_id,"read_status":value.read_status}))})
            }
            Self::Read(notification_id) | Self::MarkUnread(notification_id) => {
                json!({"path":{"notification_id":notification_id}})
            }
            Self::Logs(value) => {
                json!({"query":query(json!({"page":value.page,"page_size":value.page_size,"instance_id":value.instance_id,"type_code":value.type_code,"status":value.status,"source":value.source,"created_from":value.created_from,"created_to":value.created_to}))})
            }
            Self::Log(log_id) => json!({"path":{"log_id":log_id}}),
            Self::Channels(value) => {
                json!({"query":query(json!({"cursor":value.cursor,"page_size":value.page_size,"read_status":value.read_status}))})
            }
        })
    }
}
impl ApiOutput for NotificationOutput {
    fn document_output_schema(operation: &ApiOperationContext) -> Option<Value> {
        match operation.interface_id.as_str() {
            "notifications.instances.list" => Some(event::schema::<Vec<InstanceEvent>>()),
            "notifications.instances.get"
            | "notifications.instances.create"
            | "notifications.instances.update" => Some(event::schema::<InstanceEvent>()),
            "notifications.instances.authorization" => Some(event::schema::<AuthorizationEvent>()),
            _ => None,
        }
    }
    fn project_document_output(&self, operation: &ApiOperationContext) -> Result<Value, Arc<str>> {
        match operation.interface_id.as_str() {
            "notifications.instances.list" => safe::<Vec<InstanceEvent>>(&self.0),
            "notifications.instances.get"
            | "notifications.instances.create"
            | "notifications.instances.update" => safe::<InstanceEvent>(&self.0),
            "notifications.instances.authorization" => safe::<AuthorizationEvent>(&self.0),
            _ => Ok(self.0.clone()),
        }
    }
}

#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
#[path = "_tests/api_event_projection.rs"]
mod api_event_tests;
