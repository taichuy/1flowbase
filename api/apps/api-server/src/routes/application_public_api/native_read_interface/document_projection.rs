//! Read adapters expose route arguments and the real HTTP response DTO.
use super::*;
use crate::openapi_interface::contract::{self as ep, ApiInput, ApiOperationContext, ApiOutput};
use crate::routes::application_public_api::native_interface::document_projection as native;
use serde_json::json;
use std::sync::Arc;

impl ApiInput for NativeResumeRunInput {
    fn project_document_input(&self, _: &ApiOperationContext) -> Result<Value, Arc<str>> {
        Ok(json!({"path":{"run_id":self.run_id},"body":{
            "callback_task_id":self.callback_task_id,
            "response_payload":self.response_payload,"response_mode":self.response_mode
        }}))
    }
}
macro_rules! run_output {
    ($ty:ty) => {
        impl ApiOutput for $ty {
            fn document_output_schema(_: &ApiOperationContext) -> Option<Value> {
                Some(native::result_schema())
            }
            fn project_document_output(&self, _: &ApiOperationContext) -> Result<Value, Arc<str>> {
                native::response_document(&self.0)
            }
        }
    };
}
run_output!(NativeGetRunOutput);
run_output!(NativeCancelRunOutput);
run_output!(NativeResumeRunOutput);
impl ApiInput for NativeUploadFileInput {
    fn document_input_unavailable(
        _: &ApiOperationContext,
    ) -> Option<domain::event_center::ApiEventUnavailableReason> {
        Some(domain::event_center::ApiEventUnavailableReason::BinaryRequest)
    }
    fn project_document_input(&self, _: &ApiOperationContext) -> Result<Value, Arc<str>> {
        Err(Arc::from("binary_request"))
    }
}
#[cfg_attr(test, api_test_selection::group(public_api))]
#[cfg(test)]
#[path = "_tests/document_projection.rs"]
mod tests;
