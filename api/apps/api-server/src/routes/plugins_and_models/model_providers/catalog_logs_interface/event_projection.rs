use super::*;
use crate::event_center::api_projection::{
    self as event, ApiEventInput, ApiEventOperationContext, ApiEventOutput,
};
use serde_json::{json, Value};
use std::sync::Arc;

impl ApiEventInput for ProviderCatalogLogsInput {
    fn project_document_input(&self, _: &ApiEventOperationContext) -> Result<Value, Arc<str>> {
        Ok(match self {
            Self::Catalog { query, locale: _ } => json!({"query":event::to_value(query)?}),
            Self::ListLogs(query) => json!({"query":event::to_value(query)?}),
            Self::DeleteLogs(body) => json!({"body":event::to_value(body)?}),
            // This is a workspace-bound pagination cursor; authentication and
            // delete authorization are independent of the continuation proof.
            Self::ClearLogs(body) => json!({"body":body}),
        })
    }
}
impl ApiEventOutput for ProviderCatalogLogsOutput {
    fn project_document_output(&self, _: &ApiEventOperationContext) -> Result<Value, Arc<str>> {
        match self {
            Self::Catalog(value) => event::to_value(value),
            Self::Logs(value) => event::to_value(value),
            Self::Deleted(value) => event::to_value(value),
            Self::Cleared(value) => event::to_value(value),
        }
    }
}
