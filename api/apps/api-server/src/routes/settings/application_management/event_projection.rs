use super::*;
use crate::event_center::api_projection::{
    to_value, ApiEventInput, ApiEventOperationContext, ApiEventOutput,
};
use crate::routes::settings::document_event::query_fields as q;
use serde_json::{json, Value};
use std::sync::Arc;
impl ApiEventInput for ApplicationManagementQueryParams {
    fn project_document_input(&self, _: &ApiEventOperationContext) -> Result<Value, Arc<str>> {
        Ok(
            json!({"query":q(json!({"filter":self.filter,"sort":self.sort,"page":self.page,"page_size":self.page_size}))}),
        )
    }
}
impl ApiEventOutput for ApplicationManagementPageResponse {
    fn project_document_output(&self, _: &ApiEventOperationContext) -> Result<Value, Arc<str>> {
        to_value(self)
    }
}
