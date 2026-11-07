use super::*;
use crate::event_center::api_projection::{
    self as event, ApiEventInput, ApiEventOperationContext, ApiEventOutput,
};
use serde_json::{json, Value};
use std::sync::Arc;

impl ApiEventInput for CollectorAssetInput {
    fn project_document_input(&self, _: &ApiEventOperationContext) -> Result<Value, Arc<str>> {
        Ok(
            json!({"path":{"organization":self.organization,"artifact_id":self.artifact_id,"version":self.version,"asset":self.asset}}),
        )
    }
}
impl ApiEventOutput for CollectorAssetOutput {
    fn document_output_unavailable(
        _: &ApiEventOperationContext,
    ) -> Option<domain::event_center::ApiEventUnavailableReason> {
        Some(domain::event_center::ApiEventUnavailableReason::BinaryResponse)
    }
    fn project_document_output(&self, _: &ApiEventOperationContext) -> Result<Value, Arc<str>> {
        Err(Arc::from(
            "collector asset is a byte response without a JSON business view",
        ))
    }
}

#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
#[path = "_tests/api_event_projection.rs"]
mod api_event_tests;
