use super::*;
use crate::event_center::api_projection::{
    self as event, ApiEventInput, ApiEventOperationContext, ApiEventOutput,
};
use serde_json::{json, Value};
use std::sync::Arc;

impl ApiEventInput for ModelDefinitionsInput {
    fn project_document_input(&self, _: &ApiEventOperationContext) -> Result<Value, Arc<str>> {
        Ok(match self {
            Self::List { query, locale: _ } => json!({"query":event::to_value(query)?}),
            Self::ListCompatibleTemplates(query) => json!({"query":event::to_value(query)?}),
            Self::ListAgentFlowOptions => json!({}),
            Self::Create(body) => json!({"body":event::to_value(body)?}),
            Self::AdvisorFindings { model_id } | Self::ListScopeGrants { model_id } => {
                json!({"path":{"id":model_id}})
            }
            Self::Update { model_id, body } => {
                json!({"path":{"id":model_id},"body":event::to_value(body)?})
            }
            Self::Delete {
                model_id,
                confirmed,
            } => json!({"path":{"id":model_id},"query":{"confirmed":confirmed}}),
            Self::BatchDelete(body) => json!({"body":event::to_value(body)?}),
            Self::CreateField { model_id, body } => {
                json!({"path":{"id":model_id},"body":event::to_value(body)?})
            }
            Self::UpdateField {
                model_id,
                field_id,
                body,
            } => json!({"path":{"id":model_id,"field_id":field_id},"body":event::to_value(body)?}),
            Self::DeleteField {
                model_id,
                field_id,
                confirmed,
            } => {
                json!({"path":{"id":model_id,"field_id":field_id},"query":{"confirmed":confirmed}})
            }
            Self::CreateScopeGrant { model_id, body } => {
                json!({"path":{"id":model_id},"body":event::to_value(body)?})
            }
            Self::UpdateScopeGrant {
                model_id,
                grant_id,
                body,
            } => json!({"path":{"id":model_id,"grant_id":grant_id},"body":event::to_value(body)?}),
        })
    }
}
impl ApiEventOutput for ModelDefinitionsOutput {
    fn project_document_output(&self, _: &ApiEventOperationContext) -> Result<Value, Arc<str>> {
        match self {
            Self::Models(value) => event::to_value(value),
            Self::Templates(value) => event::to_value(value),
            Self::AgentFlowOptions(value) => event::to_value(value),
            Self::Model(value) => event::to_value(value),
            Self::AdvisorFindings(value) => event::to_value(value),
            Self::ScopeGrants(value) => event::to_value(value),
            Self::ScopeGrant(value) => event::to_value(value),
            Self::Field(value) => event::to_value(value),
            Self::Deleted => Ok(json!({"deleted":true})),
            Self::BatchDeleted(value) => event::to_value(value),
        }
    }
}

#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
#[path = "_tests/event_projection.rs"]
mod event_tests;
