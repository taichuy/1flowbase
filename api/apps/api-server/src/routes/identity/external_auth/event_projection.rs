use super::*;
use crate::event_center::api_projection::{
    self as event, ApiEventInput, ApiEventOperationContext, ApiEventOutput,
};
use serde_json::{json, Value};

impl ApiEventInput for PublicInput {
    fn document_input_unavailable(
        operation: &ApiEventOperationContext,
    ) -> Option<domain::event_center::ApiEventUnavailableReason> {
        (operation.interface_id == OPERATIONS[1])
            .then_some(domain::event_center::ApiEventUnavailableReason::CredentialBoundary)
    }
    fn project_document_input(&self, _: &ApiEventOperationContext) -> Result<Value, Arc<str>> {
        match self {
            Self::Start { login_entry_id } => {
                Ok(json!({"query":{"login_entry_id":login_entry_id}}))
            }
            Self::Complete(_) => Err(Arc::from(
                "verified OAuth callback is a sealed authentication carrier",
            )),
        }
    }
}
impl ApiEventOutput for PublicOutput {
    fn document_output_unavailable(
        _: &ApiEventOperationContext,
    ) -> Option<domain::event_center::ApiEventUnavailableReason> {
        // Browser redirects and issued cookies do not provide a JSON business result.
        Some(domain::event_center::ApiEventUnavailableReason::CredentialBoundary)
    }
    fn project_document_output(&self, _: &ApiEventOperationContext) -> Result<Value, Arc<str>> {
        match self {
            Self::Authorization(_) => Err(Arc::from("OAuth authorization redirect is host-only")),
            Self::Login(_) => Err(Arc::from(
                "external login returns a session cookie and browser redirect",
            )),
            Self::Bound => Err(Arc::from("external binding returns a browser redirect")),
            Self::PasswordVerified => Err(Arc::from(
                "password verification returns a browser redirect",
            )),
        }
    }
}
impl ApiEventInput for AuthorizeInput {
    fn project_document_input(&self, _: &ApiEventOperationContext) -> Result<Value, Arc<str>> {
        Ok(json!({"body":{"login_entry_id":self.0}}))
    }
}
impl ApiEventOutput for AuthorizeOutput {
    fn document_output_unavailable(
        _: &ApiEventOperationContext,
    ) -> Option<domain::event_center::ApiEventUnavailableReason> {
        Some(domain::event_center::ApiEventUnavailableReason::CredentialBoundary)
    }
    fn project_document_output(&self, _: &ApiEventOperationContext) -> Result<Value, Arc<str>> {
        Err(Arc::from(
            "authorization URL and cookie are host-only credentials",
        ))
    }
}

#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
#[path = "_tests/security.rs"]
mod tests;
