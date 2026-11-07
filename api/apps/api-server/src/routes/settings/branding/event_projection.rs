use super::*;
use crate::event_center::api_projection::{
    to_value, ApiEventInput, ApiEventOperationContext, ApiEventOutput,
};
use serde_json::{json, Value};
use std::sync::Arc;
impl ApiEventInput for BrandingInput {
    fn project_document_input(&self, _: &ApiEventOperationContext) -> Result<Value, Arc<str>> {
        Ok(match self {
            Self::Read => json!({}),
            Self::Save(v) => json!({"body":to_value(v)?}),
        })
    }
}
impl ApiEventOutput for BrandingOutput {
    fn project_document_output(&self, _: &ApiEventOperationContext) -> Result<Value, Arc<str>> {
        to_value(&self.0)
    }
}
