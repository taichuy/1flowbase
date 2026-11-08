use super::*;
use crate::openapi_interface::contract::{self as event, ApiInput, ApiOperationContext, ApiOutput};
use serde_json::{json, Value};
use std::sync::Arc;

impl ApiOutput for RuntimeI18nOutput {
    fn document_output_schema(_: &ApiOperationContext) -> Option<Value> {
        Some(json!({"anyOf":[event::schema::<RuntimeI18nCatalogResponse>(),{"type":"null"}]}))
    }
    fn project_document_output(&self, _: &ApiOperationContext) -> Result<Value, Arc<str>> {
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
