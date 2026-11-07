use super::*;
use crate::event_center::api_projection::{
    self as event, ApiEventInput, ApiEventOperationContext, ApiEventOutput,
};
use serde_json::{json, Value};

impl ApiEventInput for Input {
    fn document_input_schema(operation: &ApiEventOperationContext) -> Option<Value> {
        (operation.interface_id == OPERATIONS[3]).then(|| json!({"type":"object","properties":{"body":{"type":"object","properties":{},"additionalProperties":false}},"required":["body"],"additionalProperties":false}))
    }
    fn project_document_input(&self, _: &ApiEventOperationContext) -> Result<Value, Arc<str>> {
        Ok(match self {
            Self::Read => json!({}),
            Self::Unlink(identity_id) => json!({"path":{"identity_id":identity_id}}),
            Self::AuthorizePassword(login_entry_id) => {
                json!({"body":{"login_entry_id":login_entry_id}})
            }
            Self::InitializePassword(_) => json!({"body":{}}),
        })
    }
}
impl ApiEventOutput for Output {
    fn document_output_unavailable(
        operation: &ApiEventOperationContext,
    ) -> Option<domain::event_center::ApiEventUnavailableReason> {
        (operation.interface_id == OPERATIONS[2])
            .then_some(domain::event_center::ApiEventUnavailableReason::CredentialBoundary)
    }
    fn project_document_output(&self, _: &ApiEventOperationContext) -> Result<Value, Arc<str>> {
        match self {
            Self::Methods(value) => event::to_value(value),
            Self::Authorization(_, _) => Err(Arc::from(
                "authorization URL and cookie are host-only credentials",
            )),
            Self::Complete => Ok(Value::Null),
        }
    }
}
