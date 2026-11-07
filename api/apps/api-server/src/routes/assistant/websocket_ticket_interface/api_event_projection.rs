use super::*;
use crate::event_center::api_projection::{
    self as event, ApiEventInput, ApiEventOperationContext, ApiEventOutput,
};
use serde_json::{json, Value};
use std::sync::Arc;

#[derive(serde::Serialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
struct TicketEvent {
    protocol: &'static str,
    expires_in_seconds: i64,
}
impl ApiEventInput for AssistantWebSocketTicketInput {
    fn project_document_input(&self, _: &ApiEventOperationContext) -> Result<Value, Arc<str>> {
        match self {
            Self::Create { body, origin: _ } => {
                Ok(json!({"body":{"application_id":body.application_id}}))
            }
        }
    }
}
impl ApiEventOutput for AssistantWebSocketTicketOutput {
    fn document_output_schema(_: &ApiEventOperationContext) -> Option<Value> {
        Some(event::schema::<TicketEvent>())
    }
    fn project_document_output(&self, _: &ApiEventOperationContext) -> Result<Value, Arc<str>> {
        match self {
            Self::Ticket(value) => event::to_value(&TicketEvent {
                protocol: value.protocol,
                expires_in_seconds: value.expires_in_seconds,
            }),
        }
    }
}

#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
#[path = "_tests/api_event_projection.rs"]
mod api_event_tests;
