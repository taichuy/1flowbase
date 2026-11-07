use super::*;
use crate::event_center::api_projection::{
    to_value, ApiEventInput, ApiEventOperationContext, ApiEventOutput,
};
use crate::routes::document_event::query_fields as q;
use serde_json::{json, Value};
use std::sync::Arc;
impl ApiEventInput for DocsInput {
    fn project_document_input(&self, _: &ApiEventOperationContext) -> Result<Value, Arc<str>> {
        Ok(match self {
            Self::Catalog => json!({}),
            Self::CategoryOperations { category_id, query } => {
                json!({"path":{"category_id":category_id},"query":q(json!({"offset":query.offset,"limit":query.limit,"q":query.q}))})
            }
            Self::CategoryOpenApi { category_id } => json!({"path":{"category_id":category_id}}),
            Self::OperationOpenApi { operation_id } => {
                json!({"path":{"operation_id":operation_id}})
            }
        })
    }
}
impl ApiEventOutput for DocsOutput {
    fn project_document_output(&self, _: &ApiEventOperationContext) -> Result<Value, Arc<str>> {
        match self {
            Self::Catalog(v) => to_value(v),
            Self::CategoryOperations(v) => to_value(v),
            Self::OpenApi(v) => v
                .document_value()
                .map_err(|_| Arc::<str>::from("api-document-json-invalid")),
        }
    }
}

#[cfg_attr(test, api_test_selection::group(core))]
#[cfg(test)]
#[path = "_tests/document_event.rs"]
mod tests;
