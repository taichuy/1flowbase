use super::*;
use crate::openapi_interface::contract::{self as event, ApiInput, ApiOperationContext, ApiOutput};
use serde_json::{json, Value};
use std::sync::Arc;

// Managed state is serializable but its port has no OpenAPI derive. These
// exact schema-only DTOs preserve every public field, including contract identity.
#[derive(utoipa::ToSchema)]
#[allow(dead_code)]
struct ManagedTargetEvent {
    graph_fingerprint: String,
    handler_id: String,
    handler_version: String,
}
#[derive(utoipa::ToSchema)]
#[allow(dead_code)]
struct ManagedReferenceEvent {
    frozen_reference_count: usize,
    target: ManagedTargetEvent,
    current: bool,
}
#[derive(utoipa::ToSchema)]
#[allow(dead_code)]
struct ManagedDeliveryEvent {
    event_id: Uuid,
    contract_id: String,
    contract_version: String,
    subscriber_id: String,
    target: ManagedTargetEvent,
    status: String,
    pause_reason: Option<String>,
    ownership: String,
}
#[derive(utoipa::ToSchema)]
#[allow(dead_code)]
struct ManagedStateEvent {
    installation_id: Uuid,
    workspace_id: Uuid,
    executions: Vec<ManagedReferenceEvent>,
    deliveries: Vec<ManagedDeliveryEvent>,
    deliveries_truncated: bool,
    next_cursor: Option<String>,
}
#[derive(serde::Serialize, utoipa::ToSchema)]
#[serde(untagged)]
enum InstallEvent<'a> {
    Installed(&'a ExtensionInstallResponse),
    Challenge {
        status: u16,
        code: &'static str,
        message: &'static str,
        risk_challenge: &'a ExtensionRiskChallengeResponse,
    },
}
impl ApiInput for ExtensionCenterInput {
    fn document_input_unavailable(
        operation: &ApiOperationContext,
    ) -> Option<domain::event_center::ApiEventUnavailableReason> {
        (operation.interface_id == "extension_center.install.upload")
            .then_some(domain::event_center::ApiEventUnavailableReason::BinaryRequest)
    }
    fn project_document_input(&self, _: &ApiOperationContext) -> Result<Value, Arc<str>> {
        Ok(match self {
            Self::QueryManagedExecution(installation_id, cursor) => {
                json!({"path":{"installation_id":installation_id},"query":cursor.as_ref().map(|cursor|json!({"cursor":cursor})).unwrap_or_else(||json!({}))})
            }
            Self::ResumeManagedDelivery(installation_id, body) => {
                json!({"path":{"installation_id":installation_id},"body":event::to_value(body)?})
            }
            Self::RetireManagedExecution(installation_id, body) => {
                json!({"path":{"installation_id":installation_id},"body":event::to_value(body)?})
            }
            Self::GrantContributionPermission(installation_id, body) => {
                json!({"path":{"installation_id":installation_id},"body":event::to_value(body)?})
            }
            Self::RevokeContributionPermission(installation_id, body) => {
                json!({"path":{"installation_id":installation_id},"body":event::to_value(body)?})
            }
            Self::QueryContributionAuthorizations(installation_id)
            | Self::Select(installation_id)
            | Self::Enable(installation_id)
            | Self::Disable(installation_id)
            | Self::Delete(installation_id) => json!({"path":{"installation_id":installation_id}}),
            Self::ListInstalled(query) => json!({"query":event::to_value(query)?}),
            Self::ListCatalog { category, query } => {
                json!({"path":{"category":category},"query":event::to_value(query)?})
            }
            Self::GetCatalog {
                category,
                catalog_id,
            } => json!({"path":{"category":category,"catalog_id":catalog_id}}),
            Self::CheckUpdates(body) => json!({"body":event::to_value(body)?}),
            Self::InstallOfficial(body) | Self::UpdateOfficial(body) => {
                json!({"body":event::to_value(body)?})
            }
            Self::InstallUploaded(_) => {
                return Err(Arc::from("extension upload has no JSON request event view"))
            }
        })
    }
}
impl ApiOutput for ExtensionCenterOutput {
    fn document_output_schema(operation: &ApiOperationContext) -> Option<Value> {
        match operation.interface_id.as_str() {
            "extension_center.managed_execution.view"
            | "extension_center.lifecycle_deliveries.resume"
            | "extension_center.managed_executions.retire" => {
                Some(event::schema::<ManagedStateEvent>())
            }
            "extension_center.install"
            | "extension_center.update"
            | "extension_center.install.upload" => Some(event::schema::<InstallEvent<'static>>()),
            _ => None,
        }
    }
    fn project_document_output(&self, _: &ApiOperationContext) -> Result<Value, Arc<str>> {
        match self {
            Self::ManagedExecution(value) => event::to_value(value),
            Self::ContributionAuthorizations(value) => event::to_value(value),
            Self::Installed(value) => event::to_value(value),
            Self::Installation(value) => event::to_value(value),
            Self::Task(value) => event::to_value(value),
            Self::Catalog(value) => event::to_value(value),
            Self::CatalogEntry(value) => event::to_value(value),
            Self::Updates(value) => event::to_value(value),
            Self::Install(value) => match value {
                ExtensionInstallOutcome::Existing(value)
                | ExtensionInstallOutcome::Created(value) => {
                    event::to_value(&InstallEvent::Installed(value))
                }
                ExtensionInstallOutcome::Challenge(value) => {
                    event::to_value(&InstallEvent::Challenge {
                        status: 409,
                        code: "extension_risk_confirmation_required",
                        message: "Extension installation requires risk confirmation.",
                        risk_challenge: value,
                    })
                }
            },
        }
    }
}

#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
#[path = "_tests/event_projection.rs"]
mod event_tests;
