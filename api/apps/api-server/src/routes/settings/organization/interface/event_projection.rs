use super::*;
use crate::event_center::api_projection::{
    to_value, ApiEventInput, ApiEventOperationContext, ApiEventOutput,
};
use crate::routes::document_event::query_fields as q;
use serde_json::{json, Value};
use std::sync::Arc;
impl ApiEventInput for OrganizationInput {
    fn project_document_input(&self, _: &ApiEventOperationContext) -> Result<Value, Arc<str>> {
        Ok(match self {
            Self::Access => json!({}),
            Self::List(v) => {
                json!({"query":q(json!({"parent_id":v.parent_id,"prefix":v.prefix,"ids":v.ids.as_ref().map(|ids|ids.iter().map(ToString::to_string).collect::<Vec<_>>().join(",")),"limit":v.limit,"cursor":v.cursor}))})
            }
            Self::Create(v) => {
                json!({"body":{"name":v.name,"parent_id":v.parent_id,"role_codes":v.role_codes}})
            }
            Self::Update { id, body } => {
                json!({"path":{"id":id},"body":{"name":body.name,"parent_id":body.parent_id,"role_codes":body.role_codes}})
            }
            Self::Delete { id } => json!({"path":{"id":id}}),
            Self::Replace { id, body } => {
                json!({"path":{"id":id},"body":{"department_ids":body.department_ids,"primary_department_id":body.primary_department_id}})
            }
        })
    }
}
impl ApiEventOutput for OrganizationOutput {
    fn project_document_output(&self, _: &ApiEventOperationContext) -> Result<Value, Arc<str>> {
        match self {
            Self::Access(v) => to_value(v),
            Self::Departments(v) => to_value(v),
            Self::Department(v) => to_value(v),
            Self::Empty => Ok(Value::Null),
        }
    }
}
