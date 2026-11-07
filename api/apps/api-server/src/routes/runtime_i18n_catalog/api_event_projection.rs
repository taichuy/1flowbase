use super::*;
use crate::event_center::api_projection::{
    self as event, ApiEventInput, ApiEventOperationContext, ApiEventOutput,
};
use serde_json::{json, Value};
use std::sync::Arc;

impl ApiEventInput for RuntimeI18nInput {
    fn project_document_input(&self, _: &ApiEventOperationContext) -> Result<Value, Arc<str>> {
        Ok(json!({"query":{"locale":self.locale}}))
    }
}
impl ApiEventOutput for RuntimeI18nOutput {
    fn document_output_schema(_: &ApiEventOperationContext) -> Option<Value> {
        Some(json!({"anyOf":[event::schema::<RuntimeI18nCatalogResponse>(),{"type":"null"}]}))
    }
    fn project_document_output(&self, _: &ApiEventOperationContext) -> Result<Value, Arc<str>> {
        match &self.payload {
            Some(value) => event::to_value(value),
            None => Ok(Value::Null),
        }
    }
}

#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
#[path = "_tests/api_event_projection.rs"]
mod api_event_tests;
