use super::*;
use crate::openapi_interface::contract::{self as event, ApiInput, ApiOperationContext, ApiOutput};
use serde_json::{json, Value};
use uuid::Uuid;

#[derive(serde::Serialize, utoipa::ToSchema)]
struct LoginEvent<'a> {
    effective_display_role: &'a str,
    current_workspace_id: String,
}
#[derive(serde::Serialize, utoipa::ToSchema)]
struct SignUpEvent<'a> {
    login_entry_id: Uuid,
    account: &'a str,
    email: &'a str,
}
impl ApiInput for PublicSignUpInput {
    fn document_input_schema(_: &ApiOperationContext) -> Option<Value> {
        Some(
            json!({"type":"object","properties":{"body":event::schema::<SignUpEvent<'static>>()},"required":["body"],"additionalProperties":false}),
        )
    }
    fn project_document_input(&self, _: &ApiOperationContext) -> Result<Value, Arc<str>> {
        Ok(
            json!({"body":event::to_value(&SignUpEvent { login_entry_id:self.0.login_entry_id, account:&self.0.account, email:&self.0.email })?}),
        )
    }
}
impl ApiOutput for PublicSignUpOutput {
    fn document_output_schema(_: &ApiOperationContext) -> Option<Value> {
        Some(event::schema::<LoginEvent<'static>>())
    }
    fn project_document_output(&self, _: &ApiOperationContext) -> Result<Value, Arc<str>> {
        event::to_value(&LoginEvent {
            effective_display_role: &self.0.actor.effective_display_role,
            current_workspace_id: self.0.session.current_workspace_id.to_string(),
        })
    }
}
