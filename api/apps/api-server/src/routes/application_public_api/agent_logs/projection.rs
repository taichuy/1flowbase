//! Host business fields are independent of the managed kind-only summary.
use super::*;
use crate::event_center::api_projection::{
    ApiEventInput, ApiEventOperationContext, ApiEventOutput,
};
use serde_json::Value;

impl Input {
    pub(super) fn event_schema() -> Value {
        event_projection::batch_schema()
    }
    pub(super) fn event_value(&self) -> Value {
        event_projection::batch_value(&self.0)
    }
}
impl Output {
    pub(super) fn event_schema() -> Value {
        event_projection::receipt_schema()
    }
    pub(super) fn event_value(&self) -> Value {
        event_projection::receipt_value(&self.0)
    }
}

impl ApiEventInput for Input {
    fn document_input_schema(_: &ApiEventOperationContext) -> Option<Value> {
        Some(event_projection::object_schema([(
            "body",
            event_projection::batch_schema(),
        )]))
    }
    fn project_document_input(&self, _: &ApiEventOperationContext) -> Result<Value, Arc<str>> {
        // `raw` is the original source-fact/credential carrier, excluded by the
        // ingestion owner's explicit safe business view.
        Ok(serde_json::json!({"body":event_projection::batch_value(&self.0)}))
    }
}
impl ApiEventOutput for Output {
    fn document_output_schema(_: &ApiEventOperationContext) -> Option<Value> {
        Some(event_projection::receipt_schema())
    }
    fn project_document_output(&self, _: &ApiEventOperationContext) -> Result<Value, Arc<str>> {
        Ok(event_projection::receipt_value(&self.0))
    }
}
