use super::*;
use crate::event_center::api_projection::{
    self as event, ApiEventInput, ApiEventOperationContext, ApiEventOutput,
};
use serde_json::{json, Value};
use std::sync::Arc;

fn source_value(source: &McpBundleSourceBody) -> Result<Value, Arc<str>> {
    match source {
        McpBundleSourceBody::OfficialCatalog(value) => event::to_value(value),
        McpBundleSourceBody::BuiltinTemplate(value) => event::to_value(value),
        McpBundleSourceBody::InstalledExtension(value) => Ok(json!({
            "extension_installation_id":value.extension_installation_id,
            "instance_id":value.instance_id,
            "integrity_override":value.integrity_override.as_ref().map(|value|json!({"reason":value.reason,"acknowledged_warnings":value.acknowledged_warnings})),
        })),
    }
}
impl ApiEventInput for McpBundlesInput {
    fn document_input_unavailable(
        operation: &ApiEventOperationContext,
    ) -> Option<domain::event_center::ApiEventUnavailableReason> {
        // Match the compiled route: preview/import also have ordinary JSON bindings.
        matches!(
            operation.path.as_str(),
            "/api/console/mcp/bundles/preview-upload" | "/api/console/mcp/bundles/import-upload"
        )
        .then_some(domain::event_center::ApiEventUnavailableReason::BinaryRequest)
    }
    fn project_document_input(&self, _: &ApiEventOperationContext) -> Result<Value, Arc<str>> {
        Ok(match self {
            Self::ListOfficial { locale: _ } | Self::ExportDefaults => json!({}),
            Self::PreviewOfficial(body) | Self::ImportOfficial(body) => {
                json!({"body":source_value(body)?})
            }
            Self::Export(body) => json!({"body":event::to_value(body)?}),
            Self::ExportInstance { instance_id, body } => {
                json!({"path":{"instance_id":instance_id},"body":event::to_value(body)?})
            }
            Self::PreviewUploaded { .. } | Self::ImportUploaded { .. } => {
                return Err(Arc::from("uploaded ZIP has no JSON request event view"))
            }
            Self::ListLibrary { refresh_remote } => {
                json!({"query":{"refresh_remote":refresh_remote}})
            }
            Self::SyncLibrary {
                organization,
                bundle_id,
                body,
            }
            | Self::PreviewLibrary {
                organization,
                bundle_id,
                body,
            }
            | Self::ImportLibrary {
                organization,
                bundle_id,
                body,
            } => {
                json!({"path":{"organization":organization,"bundle_id":bundle_id},"body":event::to_value(body)?})
            }
            Self::SwitchLibrary {
                organization,
                bundle_id,
                bundle_version,
            }
            | Self::DeleteLibraryRelease {
                organization,
                bundle_id,
                bundle_version,
            }
            | Self::RepairLibraryRelease {
                organization,
                bundle_id,
                bundle_version,
            } => {
                json!({"path":{"organization":organization,"bundle_id":bundle_id,"bundle_version":bundle_version}})
            }
        })
    }
}
impl ApiEventOutput for McpBundlesOutput {
    fn document_output_unavailable(
        operation: &ApiEventOperationContext,
    ) -> Option<domain::event_center::ApiEventUnavailableReason> {
        // Export defaults are JSON despite sharing the export interface identity.
        (operation.method == "POST"
            && (operation.path == "/api/console/mcp/bundles/export"
                || operation.path.ends_with("/bundles/export")))
        .then_some(domain::event_center::ApiEventUnavailableReason::BinaryResponse)
    }
    fn project_document_output(&self, _: &ApiEventOperationContext) -> Result<Value, Arc<str>> {
        match self {
            Self::OfficialCatalog(value) => event::to_value(value),
            Self::PreviewOfficial(value) => event::to_value(value),
            Self::ImportOfficial(value) => event::to_value(value),
            Self::IntegrityChallenge(value) => event::to_value(value),
            Self::Archive(_) => Err(Arc::from("ZIP archive has no JSON response event view")),
            Self::ExportDefaults(value) => event::to_value(value),
            Self::Preview(value) => event::to_value(value),
            Self::Import(value) => event::to_value(value),
            Self::Library(value) => event::to_value(value),
            Self::LibraryReceipt(value) => event::to_value(value),
            Self::Deleted => Ok(Value::Null),
        }
    }
}

#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
#[path = "_tests/event_projection.rs"]
mod event_tests;
