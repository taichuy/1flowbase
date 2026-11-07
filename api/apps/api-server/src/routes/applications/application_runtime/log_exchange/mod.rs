//! Actor-authorized standard log file exchange through one Console owner.
use crate::{
    app_state::ApiState,
    error_response::ApiError,
    response::ApiSuccess,
    routes::console_interface::{
        self, ConsoleInterfaceDeclaration, ConsoleInterfaceFuture, ConsoleInterfacePort,
        ConsoleInterfaceTargetError,
    },
};
use control_plane::application::ApplicationNonCrudConsoleOperation;
use control_plane_contracts::ports::AgentLogsReceipt;
use interface_runtime::{InterfaceContract, UserPrincipal};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;
use uuid::Uuid;

pub(crate) mod event_projection;
mod interface;
mod projection;
mod routes;

pub(crate) use interface::{compile_registry, OPERATIONS};
pub(crate) use routes::*;

#[derive(Deserialize, utoipa::ToSchema, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct OtlpExportBody {
    pub record_ids: Option<Vec<Uuid>>,
    pub log_conversation_ids: Option<Vec<Uuid>>,
}
#[derive(Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct OtlpImportBody {
    pub otlp: Value,
}
#[derive(Serialize, utoipa::ToSchema)]
pub(crate) struct ImportReceipt {
    pub accepted_events: usize,
    pub duplicate_events: usize,
    pub record_ids: Vec<Uuid>,
}
impl From<AgentLogsReceipt> for ImportReceipt {
    fn from(receipt: AgentLogsReceipt) -> Self {
        Self {
            accepted_events: receipt.accepted_events,
            duplicate_events: receipt.duplicate_events,
            record_ids: receipt.record_ids,
        }
    }
}

pub(crate) enum ExchangeInput {
    OtlpExport {
        application_id: Uuid,
        body: OtlpExportBody,
    },
    OtlpImport {
        application_id: Uuid,
        body: OtlpImportBody,
    },
}
impl ExchangeInput {
    fn identity(&self) -> (Uuid, ApplicationNonCrudConsoleOperation) {
        match self {
            Self::OtlpExport { application_id, .. } => (
                *application_id,
                ApplicationNonCrudConsoleOperation::LogsOtlpExport,
            ),
            Self::OtlpImport { application_id, .. } => (
                *application_id,
                ApplicationNonCrudConsoleOperation::LogsOtlpImport,
            ),
        }
    }
    fn action(&self) -> &'static str {
        match self {
            Self::OtlpExport { .. } => "otlp_export",
            Self::OtlpImport { .. } => "otlp_import",
        }
    }
}
pub(crate) enum ExchangeOutput {
    Otlp(Value),
    Imported(ImportReceipt),
}
struct ExchangeAdapter {
    store: storage_durable_postgres::MainDurableStore,
    files: Arc<storage_object::FileStorageDriverRegistry>,
}
impl ConsoleInterfacePort<ExchangeInput, ExchangeOutput> for ExchangeAdapter {
    fn execute<'a>(
        &'a self,
        principal: &'a UserPrincipal,
        input: ExchangeInput,
    ) -> ConsoleInterfaceFuture<'a, ExchangeOutput> {
        Box::pin(async move {
            use extension_contracts::{
                ApplicationLogsRequest as Request, ApplicationLogsResponse as Output,
            };
            let request = match input {
                ExchangeInput::OtlpExport {
                    application_id,
                    body,
                } => Request::ExportOtlp {
                    application_id: application_id.to_string(),
                    record_ids: body
                        .record_ids
                        .map(|ids| ids.iter().map(ToString::to_string).collect()),
                    log_conversation_ids: body
                        .log_conversation_ids
                        .map(|ids| ids.iter().map(ToString::to_string).collect()),
                },
                ExchangeInput::OtlpImport {
                    application_id,
                    body,
                } => Request::ImportOtlp {
                    application_id: application_id.to_string(),
                    otlp: body.otlp,
                },
            };
            let response = crate::application_log_exchange::execute_with(
                self.store.clone(),
                self.files.clone(),
                principal.actor(),
                request,
            )
            .await
            .map_err(ConsoleInterfaceTargetError)?;
            Ok(match response {
                Output::Exported { otlp, .. } => ExchangeOutput::Otlp(otlp),
                Output::Imported {
                    accepted_events,
                    duplicate_events,
                    record_ids,
                } => ExchangeOutput::Imported(ImportReceipt {
                    accepted_events,
                    duplicate_events,
                    record_ids: record_ids
                        .iter()
                        .map(|id| Uuid::parse_str(id))
                        .collect::<Result<_, _>>()
                        .map_err(|error| ConsoleInterfaceTargetError(error.into()))?,
                }),
                _ => unreachable!("typed standard exchange response"),
            })
        })
    }
}

#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
mod _tests;
