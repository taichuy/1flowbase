use super::*;
use crate::event_center::api_projection::{
    self as event, ApiEventInput, ApiEventOperationContext, ApiEventOutput,
};
use serde_json::{json, Value};
use std::sync::Arc;

fn query(mut value: Value) -> Value {
    if let Some(object) = value.as_object_mut() {
        object.retain(|_, value| !value.is_null());
    }
    value
}
fn page(value: &PageQuery) -> Value {
    query(json!({"cursor":value.cursor,"limit":value.limit}))
}
impl ApiEventInput for Input {
    fn project_document_input(&self, _: &ApiEventOperationContext) -> Result<Value, Arc<str>> {
        Ok(match self {
            Self::ManagementConversations(value) | Self::ManagementMessages(value) => {
                json!({"query":query(event::to_value(value)?)})
            }
            Self::ManagementHistory(conversation_id, value) => {
                json!({"path":{"conversation_id":conversation_id},"query":page(value)})
            }
            Self::ManagementMessage(message_id) => json!({"path":{"message_id":message_id}}),
            Self::Options(application_id, instance_id) => {
                json!({"query":query(json!({"application_id":application_id,"instance_id":instance_id}))})
            }
            Self::Binding(instance_id) => json!({"path":{"instance_id":instance_id}}),
            Self::SaveBinding(instance_id, body) => {
                json!({"path":{"instance_id":instance_id},"body":event::to_value(body)?})
            }
            Self::Conversations(instance_id, value) => {
                json!({"path":{"instance_id":instance_id},"query":page(value)})
            }
            Self::Messages(instance_id, conversation_id, value)
            | Self::Files(instance_id, conversation_id, value) => {
                json!({"path":{"instance_id":instance_id,"conversation_id":conversation_id},"query":page(value)})
            }
            Self::File(instance_id, message_id, value) => {
                json!({"path":{"instance_id":instance_id,"message_id":message_id},"query":query(json!({"offset":value.offset,"max_bytes":value.max_bytes}))})
            }
            Self::Retry(instance_id, message_id) => {
                json!({"path":{"instance_id":instance_id,"message_id":message_id}})
            }
        })
    }
}
impl ApiEventOutput for Output {
    // Service DTOs exclude provider download credentials; the file endpoint's
    // base64 data and authenticated console URL are real JSON business output.
    fn project_document_output(&self, _: &ApiEventOperationContext) -> Result<Value, Arc<str>> {
        Ok(self.0.clone())
    }
}

#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
#[path = "_tests/api_event_projection.rs"]
mod api_event_tests;
