use super::*;
use crate::event_center::api_projection::{
    to_value, ApiEventInput, ApiEventOperationContext, ApiEventOutput,
};
use crate::routes::document_event::query_fields as q;
use serde_json::{json, Value};
use std::sync::Arc;

impl ApiEventInput for TemplateInput {
    fn project_document_input(&self, _: &ApiEventOperationContext) -> Result<Value, Arc<str>> {
        Ok(match self {
            Self::Catalog => json!({}),
            Self::CatalogQuery(v) | Self::Library(v) => {
                json!({"query":q(json!({"category":v.category,"cursor":v.cursor,"q":v.q}))})
            }
            Self::Download(v) => {
                json!({"body":{"catalog_id":v.catalog_id,"release_version":v.release_version}})
            }
            Self::ExportArchive(v) => json!({"query":{"format":"archive"},"body":to_value(v)?}),
            Self::ExportQuery { selection, format } => {
                json!({"query":q(json!({"format":format})),"body":to_value(selection)?})
            }
            Self::Export(v) => json!({"body":to_value(v)?}),
            Self::Preview(v) | Self::Install(v) => json!({"body":to_value(v)?}),
            Self::ResolvePreview(v) | Self::ResolveInstall(v) => match v {
                super::super::catalog::TemplateRequest::Archive(v) => {
                    json!({"body":{"archive_base64":v.archive_base64}})
                }
                super::super::catalog::TemplateRequest::Package(v) => json!({"body":to_value(v)?}),
            },
        })
    }
}
impl ApiEventOutput for TemplateOutput {
    fn project_document_output(&self, _: &ApiEventOperationContext) -> Result<Value, Arc<str>> {
        Ok(self.0.clone())
    }
}

#[cfg_attr(test, api_test_selection::group(core))]
#[cfg(test)]
#[path = "_tests/document_event.rs"]
mod tests;
