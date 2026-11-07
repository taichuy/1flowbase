use super::*;
use crate::event_center::api_projection::{
    to_value, ApiEventInput, ApiEventOperationContext, ApiEventOutput,
};
use serde_json::{json, Value};
use std::sync::Arc;
impl ApiEventInput for FileTablesInput {
    fn project_document_input(&self, _: &ApiEventOperationContext) -> Result<Value, Arc<str>> {
        Ok(match self {
            Self::List { .. } => json!({}),
            Self::Create(v) => {
                json!({"body":{"bound_storage_id":v.bound_storage_id,"code":v.code,"title":v.title}})
            }
            Self::Bind {
                file_table_id,
                body,
            } => {
                json!({"path":{"id":file_table_id},"body":{"bound_storage_id":body.bound_storage_id}})
            }
            Self::Delete { file_table_id } => json!({"path":{"id":file_table_id}}),
        })
    }
}
impl ApiEventOutput for FileTablesOutput {
    fn project_document_output(&self, _: &ApiEventOperationContext) -> Result<Value, Arc<str>> {
        match self {
            Self::List(v) => to_value(v),
            Self::Item(v) => to_value(v),
            Self::Deleted => Ok(Value::Null),
        }
    }
}
