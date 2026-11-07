use super::*;
use crate::event_center::api_projection::{
    self as event, ApiEventInput, ApiEventOperationContext, ApiEventOutput,
};
use serde_json::{json, Value};
use std::sync::Arc;

impl ApiEventInput for LocalCommandBody {
    fn project_document_input(&self, _: &ApiEventOperationContext) -> Result<Value, Arc<str>> {
        Ok(json!({"body":{"command":self.command}}))
    }
}
impl ApiEventOutput for LocalCommandExecution {
    fn project_document_output(&self, _: &ApiEventOperationContext) -> Result<Value, Arc<str>> {
        let (stdout, stderr) = self
            .0
            .output_text()
            .map_err(|_| Arc::<str>::from("local command event spool read failed"))?;
        let mut value = event::to_value(&self.0.metadata)?;
        value["stdout"] = json!(stdout);
        value["stderr"] = json!(stderr);
        Ok(value)
    }
}

#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
#[path = "_tests/api_event_projection.rs"]
mod api_event_tests;
