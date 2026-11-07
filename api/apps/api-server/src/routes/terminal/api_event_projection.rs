use super::*;
use crate::event_center::api_projection::{
    self as event, ApiEventInput, ApiEventOperationContext, ApiEventOutput,
};
use serde_json::{json, Value};
use std::sync::Arc;

impl ApiEventInput for TerminalInput {
    fn project_document_input(&self, _: &ApiEventOperationContext) -> Result<Value, Arc<str>> {
        let body = match self {
            Self::Info(_) => json!({}),
            Self::List(value) => {
                json!({"path":value.path,"timeout_ms":value.timeout_ms,"cursor":value.cursor,"limit":value.limit})
            }
            Self::Stat(value) => json!({"path":value.path,"timeout_ms":value.timeout_ms}),
            Self::Text(value) => {
                json!({"path":value.path,"timeout_ms":value.timeout_ms,"offset":value.offset,"max_bytes":value.max_bytes,"expected_version":value.expected_version})
            }
            Self::Range(value) => {
                json!({"path":value.path,"timeout_ms":value.timeout_ms,"offset":value.offset,"length":value.length,"expected_version":value.expected_version})
            }
            Self::Search(value) => {
                json!({"path":value.path,"timeout_ms":value.timeout_ms,"pattern":value.pattern,"max_scan_bytes":value.max_scan_bytes,"max_matches":value.max_matches,"max_depth":value.max_depth,"max_line_bytes":value.max_line_bytes})
            }
            Self::Image(value) | Self::Audio(value) => {
                json!({"path":value.path,"timeout_ms":value.timeout_ms,"max_file_bytes":value.max_file_bytes})
            }
            Self::Resource(value) => {
                json!({"path":value.path,"timeout_ms":value.timeout_ms,"max_file_bytes":value.max_file_bytes,"mime_type":value.mime_type,"link_only":value.link_only})
            }
            Self::Import(value) => {
                json!({"resource_uri":value.resource_uri,"file_table_id":value.file_table_id})
            }
            Self::Download(value) => {
                json!({"file_table_id":value.file_table_id,"record_id":value.record_id,"target_path":value.target_path,"timeout_ms":value.timeout_ms,"overwrite":value.overwrite,"expected_sha256":value.expected_sha256})
            }
            Self::Start(value) => event::to_value(value)?,
            Self::Get(value) | Self::Cancel(value) => json!({"execution_id":value.execution_id}),
            Self::Read(value) => {
                json!({"execution_id":value.execution_id,"stream":value.stream,"offset":value.offset,"max_bytes":value.max_bytes})
            }
        };
        Ok(json!({"body":body}))
    }
}
impl ApiEventOutput for TerminalOutput {
    fn project_document_output(&self, _: &ApiEventOperationContext) -> Result<Value, Arc<str>> {
        match self {
            Self::Info(value) => event::to_value(value),
            Self::List(value) => event::to_value(value),
            Self::Stat(value) => event::to_value(value),
            Self::Text(value) => event::to_value(value),
            Self::Range(value) => event::to_value(value),
            Self::Search(value) => event::to_value(value),
            Self::Media(value) => event::to_value(value),
            Self::Resource(value) => event::to_value(value),
            Self::Import(value) => event::to_value(value),
            Self::Download(value) => event::to_value(value),
            Self::Job(value) => event::to_value(value),
            Self::Read(value) => event::to_value(value),
        }
    }
}

#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
#[path = "_tests/api_event_projection.rs"]
mod api_event_tests;
