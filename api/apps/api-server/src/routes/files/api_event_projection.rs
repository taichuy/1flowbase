use super::*;
use crate::openapi_interface::contract::{self as event, ApiInput, ApiOperationContext, ApiOutput};
use serde_json::{json, Value};
use std::sync::Arc;

#[derive(serde::Serialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
struct RemoteFileEvent<'a> {
    file_id: &'a str,
    mime_type: &'a Option<String>,
    file_name: &'a Option<String>,
}
#[derive(serde::Serialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
struct ImportEvent<'a> {
    file_table_id: &'a Option<Uuid>,
    file: RemoteFileEvent<'a>,
}
#[derive(serde::Serialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
struct ShareLinkEvent<'a> {
    expires_at: &'a str,
}
impl ApiInput for BusinessFilesInput {
    fn document_input_unavailable(
        operation: &ApiOperationContext,
    ) -> Option<domain::event_center::ApiEventUnavailableReason> {
        (operation.interface_id == "files.upload")
            .then_some(domain::event_center::ApiEventUnavailableReason::BinaryRequest)
    }
    fn document_input_schema(operation: &ApiOperationContext) -> Option<Value> {
        (operation.interface_id == "files.import_remote").then(||json!({"type":"object","properties":{"body":event::schema::<ImportEvent<'static>>()},"required":["body"],"additionalProperties":false}))
    }
    fn project_document_input(&self, _: &ApiOperationContext) -> Result<Value, Arc<str>> {
        Ok(match self {
            Self::Upload { .. } => return Err(Arc::from("file upload has no JSON request view")),
            Self::ImportRemote {
                file_table_id,
                file,
            } => {
                json!({"body":event::to_value(&ImportEvent { file_table_id, file:RemoteFileEvent { file_id:&file.file_id, mime_type:&file.mime_type, file_name:&file.file_name } })?})
            }
            Self::CreateShareLink {
                file_table_id,
                record_id,
                expires_in_seconds,
            } => {
                json!({"path":{"file_table_id":file_table_id,"record_id":record_id},"body":{"expires_in_seconds":expires_in_seconds}})
            }
            Self::ReadContent {
                file_table_id,
                record_id,
                preview,
            } => {
                json!({"path":{"file_table_id":file_table_id,"record_id":record_id},"query":{"preview":preview}})
            }
        })
    }
}
impl ApiOutput for BusinessFilesOutput {
    fn document_output_schema(operation: &ApiOperationContext) -> Option<Value> {
        (operation.interface_id == "files.access_links.create")
            .then(|| event::schema::<ShareLinkEvent<'static>>())
    }
    fn document_output_unavailable(
        operation: &ApiOperationContext,
    ) -> Option<domain::event_center::ApiEventUnavailableReason> {
        (operation.interface_id == "files.content.download")
            .then_some(domain::event_center::ApiEventUnavailableReason::BinaryResponse)
    }
    fn project_document_output(&self, _: &ApiOperationContext) -> Result<Value, Arc<str>> {
        match self {
            Self::Uploaded(value) => event::to_value(value),
            Self::Content(_) => Err(Arc::from("owned file stream is not a JSON event result")),
            Self::ShareLink(value) => event::to_value(&ShareLinkEvent {
                expires_at: &value.expires_at,
            }),
        }
    }
}

#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
#[path = "_tests/api_event_projection.rs"]
mod api_event_tests;
