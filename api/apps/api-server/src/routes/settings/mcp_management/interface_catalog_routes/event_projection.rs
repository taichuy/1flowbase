use super::*;
use crate::event_center::api_projection::{
    self as event, ApiEventInput, ApiEventOperationContext, ApiEventOutput,
};
use serde_json::{json, Value};
use std::sync::Arc;

impl ApiEventInput for McpCatalogInput {
    fn project_document_input(&self, _: &ApiEventOperationContext) -> Result<Value, Arc<str>> {
        Ok(match self {
            Self::Catalog | Self::Export => json!({}),
            Self::Interfaces(query) => json!({"query":event::to_value(query)?}),
            Self::List(query) => json!({"query":event::to_value(query)?}),
        })
    }
}
impl ApiEventOutput for McpCatalogOutput {
    fn project_document_output(&self, _: &ApiEventOperationContext) -> Result<Value, Arc<str>> {
        match self {
            Self::Catalog(value) => event::to_value(value),
            Self::Interfaces(value) => event::to_value(value),
            Self::List(value) => event::to_value(value),
            Self::Export(value) => event::to_value(value),
        }
    }
}

#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
#[path = "_tests/event_projection.rs"]
mod event_tests;
