//! Standard OTLP Trace ingress admitted through the sealed Application principal.
use crate::{agent_logs_protocol, app_state::ApiState, error_response::ApiError};
use axum::{body::Bytes, http::StatusCode};
use control_plane::{
    errors::ControlPlaneError,
    ports::{AgentLogsBatch, AgentLogsReceipt, BillingRepository, OrchestrationRuntimeRepository},
};
use interface_runtime::*;
use opentelemetry_proto::tonic::{
    collector::trace::v1::{ExportTracePartialSuccess, ExportTraceServiceResponse},
    trace::v1::{ResourceSpans, ScopeSpans},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

mod interface;
mod projection;
mod transport;

pub(crate) use interface::compile_registry;
pub(crate) use transport::route_assembly;
pub use transport::{__path_ingest_traces, ingest_traces};

pub(crate) const BINDING: &str = "http.application.logs.otlp.traces.ingest.v1";
pub(crate) const ID: &str = "application.logs.otlp.traces.ingest";
const OWNER: &str = "api-server.application-agent-logs-otlp";
const ROUTE: &str = "/api/logs/otlp/v1/traces";

#[derive(Clone, Copy)]
enum WireFormat {
    Json,
    Protobuf,
    Unsupported,
}
impl WireFormat {
    fn from_header(value: Option<&str>) -> Self {
        match value
            .unwrap_or("")
            .split(';')
            .next()
            .unwrap_or("")
            .trim()
            .to_ascii_lowercase()
            .as_str()
        {
            "application/json" => Self::Json,
            "application/x-protobuf" => Self::Protobuf,
            _ => Self::Unsupported,
        }
    }
    fn content_type(self) -> &'static str {
        match self {
            Self::Protobuf => "application/x-protobuf",
            _ => "application/json",
        }
    }
    fn tag(self) -> &'static str {
        match self {
            Self::Json => "json",
            Self::Protobuf => "protobuf",
            Self::Unsupported => "unsupported",
        }
    }
}
#[derive(Clone, Copy)]
enum ContentEncoding {
    Identity,
    Gzip,
    Unsupported,
}
impl ContentEncoding {
    fn from_header(value: Option<&str>) -> Self {
        match value
            .unwrap_or("identity")
            .trim()
            .to_ascii_lowercase()
            .as_str()
        {
            "" | "identity" => Self::Identity,
            "gzip" => Self::Gzip,
            _ => Self::Unsupported,
        }
    }
    fn tag(self) -> &'static str {
        match self {
            Self::Identity => "identity",
            Self::Gzip => "gzip",
            Self::Unsupported => "unsupported",
        }
    }
}
struct Input {
    format: WireFormat,
    encoding: ContentEncoding,
    byte_count: usize,
    prepared: Result<PreparedInput, &'static str>,
}
struct PreparedInput {
    batches: Vec<AgentLogsBatch>,
    counts: SourceSpanCounts,
    document: Result<crate::routes::applications_group::application_runtime::log_exchange::event_projection::OtlpDocument, Arc<str>>,
}
impl Input {
    fn new(format: WireFormat, encoding: ContentEncoding, body: Bytes) -> Self {
        let prepared = (|| {
            if matches!(format, WireFormat::Unsupported) {
                return Err("agent_logs_otlp_content_type");
            }
            if matches!(encoding, ContentEncoding::Unsupported) {
                return Err("agent_logs_otlp_content_encoding");
            }
            let request =
                agent_logs_protocol::decode(format.content_type(), Some(encoding.tag()), &body)
                    .map_err(|_| "agent_logs_otlp_payload")?;
            let (batches, counts) =
                validated_batches(&request).map_err(|_| "agent_logs_otlp_payload")?;
            let document = crate::event_center::api_projection::to_value(&request).and_then(|value| {
                crate::routes::applications_group::application_runtime::log_exchange::event_projection::document(&value)
            });
            Ok(PreparedInput {
                batches,
                counts,
                document,
            })
        })();
        Self {
            format,
            encoding,
            byte_count: body.len(),
            prepared,
        }
    }
}
struct Output {
    response: ExportTraceServiceResponse,
    accepted_spans: usize,
    receipt: AgentLogsReceipt,
}
struct TargetError(ApiError);

struct Handler<R>(R);
impl<R> InterfaceHandler<Input, Output, TargetError, ApplicationPrincipal> for Handler<R>
where
    R: BillingRepository + OrchestrationRuntimeRepository + Clone + Send + Sync + 'static,
{
    fn invoke(
        &self,
        context: InterfaceHandlerContext<ApplicationPrincipal>,
        input: Input,
    ) -> InterfaceHandlerFuture<Output, TargetError> {
        let store = self.0.clone();
        Box::pin(async move {
            // Validate the entire request before the first batch can write anything.
            let PreparedInput {
                batches, counts, ..
            } = input
                .prepared
                .map_err(|code| failure(ControlPlaneError::InvalidInput(code).into()))?;
            let principal = context.principal();
            let service = control_plane::agent_logs::AgentLogsService::new(store);
            let mut accepted_spans = 0;
            let mut remaining_spans = counts.values().sum::<usize>();
            let mut receipt = AgentLogsReceipt {
                accepted_events: 0,
                duplicate_events: 0,
                record_ids: vec![],
            };
            let mut record_ids = BTreeSet::new();
            for batch in batches {
                let count = counts[&(batch.source_id.clone(), batch.source_client.clone())];
                let result = service
                    .ingest(
                        principal.application_id(),
                        principal.workspace_id(),
                        principal.api_key_id(),
                        batch,
                    )
                    .await;
                match result {
                    Ok(batch_receipt) => {
                        receipt.accepted_events += batch_receipt.accepted_events;
                        receipt.duplicate_events += batch_receipt.duplicate_events;
                        record_ids.extend(batch_receipt.record_ids);
                    }
                    Err(error) => {
                        if accepted_spans == 0 {
                            return Err(failure(error.into()));
                        }
                        // Batch transactions cannot roll back earlier sources. Count input
                        // spans, not derived Agent Log events, and stop without attempting
                        // the later sources. OTLP partial ACKs must not be retried.
                        let rejected_spans = remaining_spans;
                        let rejected_spans = i64::try_from(rejected_spans).map_err(|_| {
                            failure(
                                ControlPlaneError::InvalidInput("agent_logs_otlp_span_count")
                                    .into(),
                            )
                        })?;
                        receipt.record_ids = record_ids.into_iter().collect();
                        return Ok(Output { accepted_spans, receipt, response: ExportTraceServiceResponse { partial_success: Some(ExportTracePartialSuccess { rejected_spans, error_message: "Some source batches were not accepted; earlier source batches were persisted. Do not retry a partial acknowledgement.".into() }) } });
                    }
                }
                accepted_spans += count;
                remaining_spans -= count;
            }
            receipt.record_ids = record_ids.into_iter().collect();
            Ok(Output {
                accepted_spans,
                receipt,
                response: ExportTraceServiceResponse {
                    partial_success: None,
                },
            })
        })
    }
}
fn failure(error: ApiError) -> InterfaceTargetFailure<TargetError> {
    InterfaceTargetFailure::new("agent_logs_otlp", TargetError(error))
}

type SourceSpanCounts = BTreeMap<(String, String), usize>;

fn validated_batches(
    request: &agent_logs_protocol::ExportTraceServiceRequest,
) -> anyhow::Result<(Vec<AgentLogsBatch>, SourceSpanCounts)> {
    let batches = agent_logs_protocol::traces_to_batches(request)?;
    let mut counts = BTreeMap::new();
    for resource in &request.resource_spans {
        for scope in &resource.scope_spans {
            for span in &scope.spans {
                // Copy only this span and its shared metadata. Never clone the
                // resource's scope/span lists or the complete request per span.
                let single = agent_logs_protocol::ExportTraceServiceRequest {
                    resource_spans: vec![ResourceSpans {
                        resource: resource.resource.clone(),
                        schema_url: resource.schema_url.clone(),
                        scope_spans: vec![ScopeSpans {
                            scope: scope.scope.clone(),
                            schema_url: scope.schema_url.clone(),
                            spans: vec![span.clone()],
                        }],
                    }],
                };
                let mapped = agent_logs_protocol::traces_to_batches(&single)?;
                anyhow::ensure!(mapped.len() == 1, "agent_logs_otlp_span_source");
                let batch = &mapped[0];
                *counts
                    .entry((batch.source_id.clone(), batch.source_client.clone()))
                    .or_insert(0) += 1;
            }
        }
    }
    Ok((batches, counts))
}

#[cfg_attr(test, api_test_selection::group(public_api))]
#[cfg(test)]
mod _tests;
