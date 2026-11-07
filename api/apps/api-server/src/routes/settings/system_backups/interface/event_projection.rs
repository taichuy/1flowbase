use super::*;
use crate::event_center::api_projection::{
    schema, to_value, ApiEventInput, ApiEventOperationContext, ApiEventOutput,
};
use crate::routes::settings::document_event::{self as de, query_fields as q};
use serde_json::{json, Value};
use std::sync::Arc;

use domain::event_center::ApiEventUnavailableReason;
const CREDENTIALS: &[&str] = &["password", "backup_password", "challenge_token"];
fn backup_path_schema() -> Value {
    json!({"type":"object","properties":{"backup_set_id":{"type":"string","format":"uuid"}},"required":["backup_set_id"]})
}
impl ApiEventInput for SystemBackupsInput {
    fn document_input_schema(c: &ApiEventOperationContext) -> Option<Value> {
        match c.interface_id.as_str() {
            "system_backups.create" => Some(de::body_schema(de::without_fields(
                schema::<crate::routes::settings::system_backups::CreateBackupRequest>(),
                CREDENTIALS,
            ))),
            "system_backups.recovery.reauth" => Some(de::body_schema(de::without_fields(
                schema::<RecoveryReauthRequest>(),
                CREDENTIALS,
            ))),
            "system_backups.recovery.intent" => Some(de::path_body_schema(
                backup_path_schema(),
                de::without_fields(schema::<CreateRecoveryIntentRequest>(), CREDENTIALS),
            )),
            _ => None,
        }
    }
    fn document_input_unavailable(
        c: &ApiEventOperationContext,
    ) -> Option<ApiEventUnavailableReason> {
        (c.interface_id == "system_backups.import")
            .then_some(ApiEventUnavailableReason::BinaryRequest)
    }
    fn project_document_input(&self, _: &ApiEventOperationContext) -> Result<Value, Arc<str>> {
        Ok(match self {
            Self::Catalog | Self::List => json!({}),
            Self::Create { selection, .. } => {
                json!({"body":{"selection":{"features":selection.features,"include_file_bytes":selection.include_file_bytes}}})
            }
            Self::Import { .. } => return Err(Arc::from("backup-import-binary-request")),
            Self::GetJobStatus { backup_job_id } => json!({"path":{"backup_job_id":backup_job_id}}),
            Self::GetDetail { backup_set_id }
            | Self::Delete { backup_set_id }
            | Self::Verify { backup_set_id, .. }
            | Self::Download { backup_set_id }
            | Self::RecoveryPreflight { backup_set_id, .. } => {
                json!({"path":{"backup_set_id":backup_set_id}})
            }
            Self::GetRecoveryStatus { recovery_job_id } => {
                json!({"query":q(json!({"recovery_job_id":recovery_job_id}))})
            }
            Self::RecoveryReauth { request: r } => {
                json!({"body":{"backup_set_id":r.backup_set_id,"exact_backup_name":r.exact_backup_name,"plan_digest":r.plan_digest}})
            }
            Self::RecoveryIntent {
                backup_set_id,
                request: r,
            } => {
                json!({"path":{"backup_set_id":backup_set_id},"body":{"confirm_missing_plugins":r.confirm_missing_plugins,"exact_backup_name":r.exact_backup_name,"plan_digest":r.plan_digest}})
            }
        })
    }
}
impl ApiEventOutput for SystemBackupsOutput {
    fn document_output_schema(c: &ApiEventOperationContext) -> Option<Value> {
        (c.interface_id == "system_backups.recovery.reauth")
            .then(|| de::without_fields(schema::<RecoveryReauthResponse>(), CREDENTIALS))
    }
    fn document_output_unavailable(
        c: &ApiEventOperationContext,
    ) -> Option<ApiEventUnavailableReason> {
        (c.interface_id == "system_backups.download")
            .then_some(ApiEventUnavailableReason::BinaryResponse)
    }
    fn project_document_output(&self, _: &ApiEventOperationContext) -> Result<Value, Arc<str>> {
        match self {
            Self::Catalog(v) => to_value(v),
            Self::Listed(v) => to_value(v),
            Self::Created(v) => to_value(v),
            Self::Imported(v) => to_value(v),
            Self::JobStatus(v) => to_value(v),
            Self::Detail(v) => to_value(v),
            Self::Deleted => Ok(Value::Null),
            Self::Verified(v) => to_value(v),
            Self::Download(_) => Err(Arc::from("backup-download-binary-response")),
            Self::RecoveryStatus(v) => to_value(v),
            Self::RecoveryPreflight(v) => to_value(v),
            Self::RecoveryReauth(v) => Ok(json!({"expires_at":v.expires_at})),
            Self::RecoveryIntent(v) => to_value(v),
        }
    }
}

#[cfg_attr(test, api_test_selection::group(core))]
#[cfg(test)]
#[path = "_tests/document_event.rs"]
mod tests;
