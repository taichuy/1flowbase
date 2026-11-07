use super::*;
use crate::event_center::api_projection::{
    schema, to_value, ApiEventInput, ApiEventOperationContext, ApiEventOutput,
};
use crate::routes::settings::document_event as de;
use serde_json::{json, Value};
use std::sync::Arc;

// Driver configuration and provider errors can contain credentials; the event owner excludes them.
const HIDDEN: &[&str] = &["config_json", "last_health_error"];
impl ApiEventInput for FileStoragesInput {
    fn document_input_schema(c: &ApiEventOperationContext) -> Option<Value> {
        match c.interface_id.as_str() {
            "file_storages.create" => Some(de::body_schema(de::without_fields(
                schema::<CreateFileStorageBody>(),
                HIDDEN,
            ))),
            "file_storages.update" => Some(de::path_body_schema(
                json!({"type":"object","properties":{"id":{"type":"string"}},"required":["id"]}),
                de::without_fields(schema::<UpdateFileStorageBody>(), HIDDEN),
            )),
            _ => None,
        }
    }
    fn project_document_input(&self, _: &ApiEventOperationContext) -> Result<Value, Arc<str>> {
        Ok(match self {
            Self::List { .. } => json!({}),
            Self::Create(b) => {
                json!({"body":{"code":b.code,"title":b.title,"driver_type":b.driver_type,"enabled":b.enabled,"is_default":b.is_default,"rule_json":b.rule_json}})
            }
            Self::Update {
                file_storage_id,
                body: b,
            } => {
                json!({"path":{"id":file_storage_id},"body":{"title":b.title,"enabled":b.enabled,"is_default":b.is_default,"rule_json":b.rule_json}})
            }
            Self::Delete { file_storage_id } => json!({"path":{"id":file_storage_id}}),
        })
    }
}
impl ApiEventOutput for FileStoragesOutput {
    fn document_output_schema(c: &ApiEventOperationContext) -> Option<Value> {
        match c.interface_id.as_str() {
            "file_storages.list" => Some(de::array_schema(de::without_fields(
                schema::<FileStorageResponse>(),
                HIDDEN,
            ))),
            "file_storages.create" | "file_storages.update" => {
                Some(de::without_fields(schema::<FileStorageResponse>(), HIDDEN))
            }
            _ => None,
        }
    }
    fn project_document_output(&self, _: &ApiEventOperationContext) -> Result<Value, Arc<str>> {
        Ok(match self {
            Self::List(v) => Value::Array(
                v.iter()
                    .map(|v| to_value(v).map(|v| de::without_values(v, HIDDEN)))
                    .collect::<Result<_, _>>()?,
            ),
            Self::Item(v) => de::without_values(to_value(v)?, HIDDEN),
            Self::Deleted => Value::Null,
        })
    }
}

#[cfg_attr(test, api_test_selection::group(core))]
#[cfg(test)]
#[path = "_tests/document_event.rs"]
mod tests;
