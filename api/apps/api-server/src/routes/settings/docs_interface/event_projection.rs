use super::*;
use crate::openapi_interface::contract::{to_value, ApiInput, ApiOperationContext, ApiOutput};
use crate::openapi_interface::schema_projection::query_fields as q;
use serde_json::{json, Value};
use std::sync::Arc;
impl ApiOutput for DocsOutput {
    fn project_document_output(&self, _: &ApiOperationContext) -> Result<Value, Arc<str>> {
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
