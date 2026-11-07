use super::*;
use crate::event_center::api_projection::{
    ApiEventInput, ApiEventOperationContext, ApiEventOutput,
};
use serde_json::{json, Value};
use std::sync::Arc;
impl ApiEventInput for DataModelOpenApiInput {
    fn project_document_input(&self, _: &ApiEventOperationContext) -> Result<Value, Arc<str>> {
        Ok(json!({"path":{"model_id":self.model_id}}))
    }
}
impl ApiEventOutput for DataModelOpenApiOutput {
    fn project_document_output(&self, _: &ApiEventOperationContext) -> Result<Value, Arc<str>> {
        Ok(self.0.clone())
    }
}
