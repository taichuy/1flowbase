use super::*;
use crate::openapi_interface::contract::{self as event, ApiInput, ApiOperationContext, ApiOutput};
use serde_json::{json, Value};
use std::sync::Arc;

impl ApiInput for TemporaryFileInput {
    fn document_input_schema(_: &ApiOperationContext) -> Option<Value> {
        Some(
            json!({"type":"object","properties":{"query":{"type":"object","properties":{"preview":{"type":"boolean"}},"required":["preview"],"additionalProperties":false}},"required":["query"],"additionalProperties":false}),
        )
    }
    fn project_document_input(&self, _: &ApiOperationContext) -> Result<Value, Arc<str>> {
        Ok(json!({"query":{"preview":self.1}}))
    }
}
impl ApiOutput for TemporaryFileOutput {
    fn document_output_unavailable(
        _: &ApiOperationContext,
    ) -> Option<domain::event_center::ApiEventUnavailableReason> {
        Some(domain::event_center::ApiEventUnavailableReason::BinaryResponse)
    }
    fn project_document_output(&self, _: &ApiOperationContext) -> Result<Value, Arc<str>> {
        Err(Arc::from(
            "temporary file output is owned file content, not JSON",
        ))
    }
}

#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
#[path = "_tests/api_event_projection.rs"]
mod api_event_tests;
