use super::*;
use crate::event_center::api_projection::{
    self as event, ApiEventInput, ApiEventOperationContext, ApiEventOutput,
};
use serde_json::{json, Value};
use std::sync::Arc;

impl ApiEventInput for McpToolsInput {
    fn project_document_input(&self, _: &ApiEventOperationContext) -> Result<Value, Arc<str>> {
        Ok(match self {
            Self::List => json!({}),
            Self::Create(body) => json!({"body":event::to_value(body)?}),
            Self::Get(tool_id) | Self::Delete(tool_id) | Self::RefreshDescription(tool_id) => {
                json!({"path":{"tool_id":tool_id}})
            }
            Self::Update(tool_id, body) => {
                json!({"path":{"tool_id":tool_id},"body":event::to_value(body)?})
            }
            Self::CheckDescription(tool_id, body) => {
                json!({"path":{"tool_id":tool_id},"body":event::to_value(body)?})
            }
        })
    }
}
impl ApiEventOutput for McpToolsOutput {
    fn project_document_output(&self, _: &ApiEventOperationContext) -> Result<Value, Arc<str>> {
        match self {
            Self::Tools(value) => event::to_value(value),
            Self::Tool(value) => event::to_value(value),
            Self::Check(value) => event::to_value(value),
            Self::NoContent => Ok(Value::Null),
        }
    }
}

#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
#[path = "_tests/event_projection.rs"]
mod event_tests;
