//! Observe the actual host-owned record permission API values after authorization.
use super::*;
use crate::event_center::api_projection::{
    self as event, ApiEventInput, ApiEventOperationContext, ApiEventOutput,
};

impl ApiEventInput for Input {
    fn project_document_input(&self, _: &ApiEventOperationContext) -> Result<Value, Arc<str>> {
        Ok(match self {
            Self::Get(query) => json!({"query":event::to_value(query)?}),
            Self::Replace(query, body) => {
                json!({"query":event::to_value(query)?,"body":event::to_value(body)?})
            }
            Self::Capabilities(body) => json!({"body":event::to_value(body)?}),
        })
    }
}
impl ApiEventOutput for Output {
    fn project_document_output(&self, _: &ApiEventOperationContext) -> Result<Value, Arc<str>> {
        Ok(self.0.clone())
    }
}
