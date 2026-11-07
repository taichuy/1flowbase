use super::*;
use crate::event_center::api_projection::{
    self as event, ApiEventInput, ApiEventOperationContext, ApiEventOutput,
};
use serde_json::{json, Value};
use std::sync::Arc;

impl ApiEventInput for ProviderDiscoveryInput {
    fn project_document_input(&self, _: &ApiEventOperationContext) -> Result<Value, Arc<str>> {
        Ok(match self {
            Self::Models { id } | Self::RefreshModels { id } => json!({"path":{"id":id}}),
            Self::Options {
                query,
                locale: _,
                settings: _,
            } => json!({"query":event::to_value(query)?}),
        })
    }
}
impl ApiEventOutput for ProviderDiscoveryOutput {
    fn project_document_output(&self, _: &ApiEventOperationContext) -> Result<Value, Arc<str>> {
        match self {
            Self::Models(value) => event::to_value(value),
            Self::Options(value) => event::to_value(value),
        }
    }
}

#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
#[path = "_tests/event_projection.rs"]
mod event_tests;
