use super::*;

// Export keeps its query format before the handler normalizes the command.
// The archive variant is the public archive protocol, not the plain export default.
pub(super) fn export_query<S: serde::Serializer>(
    selection: &PortableTemplateSelection,
    format: &Option<String>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    json_contract::query_body(
        &super::super::catalog::ExportQuery {
            format: format.clone(),
        },
        selection,
        serializer,
    )
}
pub(super) fn export_archive<S: serde::Serializer>(
    selection: &PortableTemplateSelection,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    json_contract::query_body(
        &super::super::catalog::ExportQuery {
            format: Some("archive".to_owned()),
        },
        selection,
        serializer,
    )
}

#[cfg(test)]
use crate::openapi_interface::contract::{ApiInput, ApiOperationContext, ApiOutput};

#[cfg_attr(test, api_test_selection::group(core))]
#[cfg(test)]
#[path = "_tests/document_event.rs"]
mod tests;
