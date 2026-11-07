//! Read adapters expose route arguments and the real HTTP response DTO.
use super::*;
use crate::event_center::api_projection::{
    self as ep, ApiEventInput, ApiEventOperationContext, ApiEventOutput,
};
use crate::routes::application_public_api::native_interface::document_projection as native;
use serde_json::json;
use std::sync::Arc;

impl ApiEventInput for NativeModelsInput {
    fn project_document_input(&self, _: &ApiEventOperationContext) -> Result<Value, Arc<str>> {
        Ok(json!({}))
    }
}
impl ApiEventOutput for NativeModelsOutput {
    fn project_document_output(&self, _: &ApiEventOperationContext) -> Result<Value, Arc<str>> {
        ep::to_value(&self.0)
    }
}
macro_rules! run_input {
    ($ty:ty) => {
        impl ApiEventInput for $ty {
            fn project_document_input(&self, _: &ApiEventOperationContext) -> Result<Value, Arc<str>> {
                Ok(json!({"path":{"run_id":self.0}}))
            }
        }
    };
}
run_input!(NativeGetRunInput);
run_input!(NativeCancelRunInput);
impl ApiEventInput for NativeResumeRunInput {
    fn project_document_input(&self, _: &ApiEventOperationContext) -> Result<Value, Arc<str>> {
        Ok(json!({"path":{"run_id":self.run_id},"body":{
            "callback_task_id":self.callback_task_id,
            "response_payload":self.response_payload,"response_mode":self.response_mode
        }}))
    }
}
macro_rules! run_output {
    ($ty:ty) => {
        impl ApiEventOutput for $ty {
            fn document_output_schema(_: &ApiEventOperationContext) -> Option<Value> {
                Some(native::result_schema())
            }
            fn project_document_output(
                &self,
                _: &ApiEventOperationContext,
            ) -> Result<Value, Arc<str>> {
                native::response_document(&self.0)
            }
        }
    };
}
run_output!(NativeGetRunOutput);
run_output!(NativeCancelRunOutput);
run_output!(NativeResumeRunOutput);
impl ApiEventInput for NativeUploadFileInput {
    fn document_input_unavailable(
        _: &ApiEventOperationContext,
    ) -> Option<domain::event_center::ApiEventUnavailableReason> {
        Some(domain::event_center::ApiEventUnavailableReason::BinaryRequest)
    }
    fn project_document_input(&self, _: &ApiEventOperationContext) -> Result<Value, Arc<str>> {
        Err(Arc::from("binary_request"))
    }
}
impl ApiEventOutput for NativeUploadFileOutput {
    fn project_document_output(&self, _: &ApiEventOperationContext) -> Result<Value, Arc<str>> {
        ep::to_value(&self.0)
    }
}

#[cfg_attr(test, api_test_selection::group(public_api))]
#[cfg(test)]
#[path = "_tests/document_projection.rs"]
mod tests;
