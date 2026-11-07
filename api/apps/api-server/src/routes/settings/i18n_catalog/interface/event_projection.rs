use super::*;
use crate::event_center::api_projection::{
    to_value, ApiEventInput, ApiEventOperationContext, ApiEventOutput,
};
use crate::routes::document_event::query_fields as q;
use serde_json::{json, Value};
use std::sync::Arc;

impl ApiEventInput for I18nCatalogInput {
    fn project_document_input(&self, _: &ApiEventOperationContext) -> Result<Value, Arc<str>> {
        Ok(match self {
            Self::GetState | Self::CheckUpdate => json!({}),
            Self::ActivateOfficial(b) => json!({"body":{"expected_revision":b.expected_revision}}),
            Self::PreviewInstalled { installation_id } => {
                json!({"path":{"installation_id":installation_id}})
            }
            Self::ActivateInstalled {
                installation_id,
                body: b,
            } => {
                json!({"path":{"installation_id":installation_id},"body":{"expected_revision":b.expected_revision,"integrity_override":b.integrity_override}})
            }
            Self::ListEntries(v) => {
                json!({"query":q(json!({"key":v.key,"locale":v.locale,"search":v.search,"origin":v.origin,"offset":v.offset,"limit":v.limit}))})
            }
            Self::GetEntry(v) => json!({"query":{"key":v.key,"locale":v.locale}}),
            Self::UpsertOfficialOverride(b) | Self::UpsertCustomTranslation(b) => {
                json!({"body":{"key":b.key,"locale":b.locale,"translation":b.translation,"expected_revision":b.expected_revision}})
            }
            Self::RestoreOfficialOverride(b) => {
                json!({"body":{"key":b.key,"locale":b.locale,"expected_revision":b.expected_revision}})
            }
            Self::DeleteCustomKey(b) => {
                json!({"body":{"key":b.key,"expected_revision":b.expected_revision}})
            }
            Self::RestoreAllOfficialOverrides(b) => {
                json!({"body":{"expected_revision":b.expected_revision}})
            }
        })
    }
}
impl ApiEventOutput for I18nCatalogOutput {
    fn project_document_output(&self, _: &ApiEventOperationContext) -> Result<Value, Arc<str>> {
        match self {
            Self::State(v) => to_value(v),
            Self::UpdateStatus(v) => to_value(v),
            Self::Activation(v) => to_value(v),
            Self::InstalledPreview(v) => to_value(v),
            Self::Entries(v) => to_value(v),
            Self::Entry(v) => to_value(v),
            Self::EntryMutation(v) => to_value(v),
            Self::Revision(v) => to_value(v),
        }
    }
}
