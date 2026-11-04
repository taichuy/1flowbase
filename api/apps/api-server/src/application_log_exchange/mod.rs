//! Actor-authorized standard log exchange shared by Console and managed services.
use crate::{agent_logs_protocol as protocol, app_state::ApiState, error_response::ApiError};
use control_plane::{
    agent_logs::AgentLogsService,
    application::{ApplicationNonCrudConsoleOperation as Operation, ApplicationService},
    errors::ControlPlaneError,
};
use control_plane_contracts::ports::{
    AgentLogsExportSelector, ApplicationLogRecordsQuery, OrchestrationRuntimeRepository,
};
use domain::{
    ActorContext, ApplicationRecord, ApplicationType, ResourceFilterExpr as Filter,
    ResourceFilterOperator as Operator,
};
use extension_contracts::{
    ApplicationLogsApplication, ApplicationLogsRecord, ApplicationLogsRequest,
    ApplicationLogsResponse,
};
use serde_json::{json, Value};
use std::collections::BTreeSet;
use time::{format_description::well_known::Rfc3339, OffsetDateTime};
use uuid::Uuid;
mod native;

pub(crate) async fn execute(
    state: &ApiState,
    actor: &ActorContext,
    request: ApplicationLogsRequest,
) -> Result<ApplicationLogsResponse, ApiError> {
    execute_with(
        state.store.clone(),
        state.file_storage_registry.clone(),
        actor,
        request,
    )
    .await
}
pub(crate) async fn execute_with(
    store: storage_durable_postgres::MainDurableStore,
    files: std::sync::Arc<storage_object::FileStorageDriverRegistry>,
    actor: &ActorContext,
    request: ApplicationLogsRequest,
) -> Result<ApplicationLogsResponse, ApiError> {
    let store = store.for_actor(actor.clone());
    let applications = ApplicationService::new(store.clone());
    match request {
        ApplicationLogsRequest::ListApplications { application_types } => {
            if application_types
                .iter()
                .any(|kind| !matches!(kind.as_str(), "agent_flow" | "agent_logs"))
            {
                return Err(ControlPlaneError::InvalidInput("application_type").into());
            }
            let visible = applications
                .list_readable_applications(actor.user_id, "query_log_records")
                .await?;
            Ok(ApplicationLogsResponse::Applications {
                items: visible
                    .into_iter()
                    .filter(|app| {
                        matches!(
                            app.application_type,
                            ApplicationType::AgentFlow | ApplicationType::AgentLogs
                        )
                    })
                    .filter(|app| {
                        application_types.is_empty()
                            || application_types
                                .iter()
                                .any(|kind| kind == app.application_type.as_str())
                    })
                    .map(|app| ApplicationLogsApplication {
                        application_id: app.id.to_string(),
                        name: app.name,
                        application_type: app.application_type.as_str().into(),
                    })
                    .collect(),
            })
        }
        ApplicationLogsRequest::ListRecords {
            application_id,
            started_at_from,
            started_at_to,
            cursor,
            limit,
        } => {
            let id = parse_id(&application_id)?;
            let app = applications
                .get_application_for_read_operation(actor.user_id, id, "query_log_records")
                .await?;
            ensure_source(&app)?;
            let filter = date_filter(started_at_from, started_at_to)?;
            let query = ApplicationLogRecordsQuery {
                filter,
                sort_field: "started_at".into(),
                descending: false,
                cursor,
                limit: i64::from(limit.unwrap_or(50)),
            };
            let page = store
                .query_application_log_records(actor.current_workspace_id, &[id], &query)
                .await?;
            Ok(ApplicationLogsResponse::Records {
                items: page
                    .items
                    .into_iter()
                    .map(|record| ApplicationLogsRecord {
                        record_id: record.record_id.to_string(),
                        log_conversation_id: record.log_conversation_id.map(|id| id.to_string()),
                        title: record.title,
                        started_at: record.started_at,
                        source_kind: record.source_kind,
                    })
                    .collect(),
                next_cursor: page.next_cursor,
            })
        }
        ApplicationLogsRequest::ExportOtlp {
            application_id,
            record_ids,
            log_conversation_ids,
        } => {
            let id = parse_id(&application_id)?;
            let app = applications
                .load_application_for_non_crud_console_operation_for_actor(
                    actor,
                    id,
                    Operation::LogsOtlpExport,
                )
                .await?;
            ensure_source(&app)?;
            let selector = AgentLogsExportSelector::from_ids(
                parse_ids(record_ids)?,
                parse_ids(log_conversation_ids)?,
            )
            .map_err(|_| ControlPlaneError::InvalidInput("agent_logs_export_selection"))?;
            let facts = if app.application_type == ApplicationType::AgentLogs {
                store
                    .read_imported_agent_logs_selection(id, actor.current_workspace_id, &selector)
                    .await
                    .map_err(selection_error)?
            } else {
                native::read(store.clone(), files, actor, &app, &selector).await?
            };
            let request = protocol::batches_to_traces(&facts.batches)
                .map_err(|_| ControlPlaneError::Conflict("agent_logs_source_facts"))?;
            let spans: Vec<_> = request
                .resource_spans
                .iter()
                .flat_map(|resource| &resource.scope_spans)
                .flat_map(|scope| &scope.spans)
                .collect();
            let trace_ids = spans
                .iter()
                .map(|span| {
                    span.trace_id
                        .iter()
                        .map(|byte| format!("{byte:02x}"))
                        .collect::<String>()
                })
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect();
            let exported_spans = spans.len();
            Ok(ApplicationLogsResponse::Exported {
                otlp: serde_json::to_value(request)?,
                record_ids: facts.record_ids.iter().map(ToString::to_string).collect(),
                trace_ids,
                exported_spans,
            })
        }
        ApplicationLogsRequest::ImportOtlp {
            application_id,
            otlp,
        } => {
            let id = parse_id(&application_id)?;
            let app = applications
                .load_application_for_non_crud_console_operation_for_actor(
                    actor,
                    id,
                    Operation::LogsOtlpImport,
                )
                .await?;
            if app.application_type != ApplicationType::AgentLogs {
                return Err(ControlPlaneError::InvalidInput("agent_logs_application_type").into());
            }
            let request: protocol::ExportTraceServiceRequest = serde_json::from_value(otlp)
                .map_err(|_| ControlPlaneError::InvalidInput("agent_logs_otlp_payload"))?;
            let batches = protocol::traces_to_batches(&request)
                .map_err(|_| ControlPlaneError::InvalidInput("agent_logs_otlp_payload"))?;
            // Keep the service's reception recheck and truthful partial-commit error.
            let receipt = AgentLogsService::new(store)
                .import_console_batches(actor, id, Operation::LogsOtlpImport, batches)
                .await?;
            Ok(ApplicationLogsResponse::Imported {
                accepted_events: receipt.accepted_events,
                duplicate_events: receipt.duplicate_events,
                record_ids: receipt.record_ids.iter().map(ToString::to_string).collect(),
            })
        }
    }
}
fn parse_id(value: &str) -> Result<Uuid, ApiError> {
    Uuid::parse_str(value).map_err(|_| ControlPlaneError::InvalidInput("application_log_id").into())
}
fn parse_ids(values: Option<Vec<String>>) -> Result<Option<Vec<Uuid>>, ApiError> {
    values
        .map(|values| values.iter().map(|value| parse_id(value)).collect())
        .transpose()
}
fn ensure_source(app: &ApplicationRecord) -> Result<(), ApiError> {
    if !matches!(
        app.application_type,
        ApplicationType::AgentFlow | ApplicationType::AgentLogs
    ) {
        return Err(ControlPlaneError::InvalidInput("agent_logs_application_type").into());
    }
    Ok(())
}
fn selection_error(error: anyhow::Error) -> ApiError {
    match error.to_string().as_str() {
        "agent_logs.read_selection_empty" => {
            ControlPlaneError::InvalidInput("agent_logs_record_ids").into()
        }
        "agent_logs.read_selection_invalid" => {
            ControlPlaneError::NotFound("agent_logs_record").into()
        }
        _ => ControlPlaneError::Conflict("agent_logs_source_facts").into(),
    }
}
fn date_filter(from: Option<String>, to: Option<String>) -> Result<Filter, ApiError> {
    let parse = |text: &str| {
        OffsetDateTime::parse(text, &Rfc3339)
            .map_err(|_| ApiError::from(ControlPlaneError::InvalidInput("agent_logs_started_at")))
    };
    let start = from.as_deref().map(parse).transpose()?;
    let end = to.as_deref().map(parse).transpose()?;
    if start.zip(end).is_some_and(|(start, end)| start >= end) {
        return Err(ControlPlaneError::InvalidInput("agent_logs_started_at").into());
    }
    Ok(Filter::all(
        [(from, Operator::Gte), (to, Operator::Lt)]
            .into_iter()
            .filter_map(|(value, operator)| {
                value.map(|value| Filter::Field {
                    field: "started_at".into(),
                    operator,
                    value: json!(value),
                })
            })
            .collect(),
    ))
}
api_nonroute_tests! {
#[cfg(test)]
mod _tests;
}
