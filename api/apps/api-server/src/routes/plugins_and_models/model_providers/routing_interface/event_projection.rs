use super::*;
use crate::event_center::api_projection::{
    self as event, ApiEventInput, ApiEventOperationContext, ApiEventOutput,
};
use serde_json::{json, Value};
use std::sync::Arc;

impl ApiEventInput for ProviderRoutingInput {
    fn project_document_input(&self, _: &ApiEventOperationContext) -> Result<Value, Arc<str>> {
        Ok(match self {
            Self::Get { provider_code } => json!({"path":{"provider_code":provider_code}}),
            Self::Update {
                provider_code,
                body,
            } => json!({"path":{"provider_code":provider_code},"body":event::to_value(body)?}),
        })
    }
}
impl ApiEventOutput for ProviderRoutingOutput {
    fn project_document_output(&self, _: &ApiEventOperationContext) -> Result<Value, Arc<str>> {
        event::to_value(&self.0)
    }
}

#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
#[path = "_tests/event_projection.rs"]
mod event_tests;
