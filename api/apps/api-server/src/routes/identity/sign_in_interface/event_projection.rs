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
struct SignInEvent<'a> {
    login_entry_id: Uuid,
    identifier: &'a str,
}
impl ApiInput for PublicSignInInput {
    fn document_input_schema(_: &ApiOperationContext) -> Option<Value> {
        Some(
            json!({"type":"object","properties":{"body":event::schema::<SignInEvent<'static>>()},"required":["body"],"additionalProperties":false}),
        )
    }
    fn project_document_input(&self, _: &ApiOperationContext) -> Result<Value, Arc<str>> {
        Ok(
            json!({"body":event::to_value(&SignInEvent { login_entry_id:self.0.login_entry_id, identifier:&self.0.identifier })?}),
        )
    }
}
impl ApiOutput for PublicSignInOutput {
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

#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
#[path = "_tests/security.rs"]
mod tests;
