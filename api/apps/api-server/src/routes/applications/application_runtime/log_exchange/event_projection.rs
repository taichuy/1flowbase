//! Safe document-shaped OTLP views. Arbitrary OTLP attributes are opaque carriers
//! for headers, credentials and original source facts. Their contents are never
//! included by this API codec. The existing typed source-fact producer remains
//! the owner of semantic Agent Logs events.
use super::*;
use crate::event_center::api_projection::{
    self as ep, ApiEventInput, ApiEventOperationContext, ApiEventOutput,
};
use crate::routes::applications_group::event_projection as application;

#[derive(Deserialize, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OtlpDocument {
    resource_spans: Vec<ResourceSpans>,
}

#[derive(Deserialize, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
struct ResourceSpans {
    resource: Option<Resource>,
    scope_spans: Vec<ScopeSpans>,
    schema_url: String,
}

#[derive(Deserialize, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
struct Resource {
    dropped_attributes_count: u32,
}

#[derive(Deserialize, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
struct ScopeSpans {
    scope: Option<Scope>,
    spans: Vec<Span>,
    schema_url: String,
}

#[derive(Deserialize, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
struct Scope {
    name: String,
    version: String,
    dropped_attributes_count: u32,
}

#[derive(Deserialize, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
struct Span {
    trace_id: String,
    span_id: String,
    parent_span_id: String,
    flags: u32,
    name: String,
    kind: i32,
    start_time_unix_nano: String,
    end_time_unix_nano: String,
    dropped_attributes_count: u32,
    events: Vec<Event>,
    dropped_events_count: u32,
    links: Vec<Link>,
    dropped_links_count: u32,
    status: Option<Status>,
}

#[derive(Deserialize, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
struct Event {
    time_unix_nano: String,
    name: String,
    dropped_attributes_count: u32,
}

#[derive(Deserialize, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
struct Link {
    trace_id: String,
    span_id: String,
    flags: u32,
    dropped_attributes_count: u32,
}

#[derive(Deserialize, Serialize, utoipa::ToSchema)]
struct Status {
    message: String,
    code: i32,
}

#[derive(Serialize, utoipa::ToSchema)]
struct ImportBody {
    otlp: OtlpDocument,
}

pub(crate) fn document(value: &Value) -> Result<OtlpDocument, Arc<str>> {
    // Decode through the actual protocol DTO first. This preserves canonical OTLP
    // hex IDs and string-encoded nanoseconds before selecting the safe fields.
    let request: crate::agent_logs_protocol::ExportTraceServiceRequest =
        serde_json::from_value(value.clone())
            .map_err(|_| Arc::from("invalid_typed_otlp_document"))?;
    serde_json::from_value(ep::to_value(&request)?)
        .map_err(|_| Arc::from("invalid_safe_otlp_document"))
}

impl ApiEventInput for ExchangeInput {
    fn document_input_schema(operation: &ApiEventOperationContext) -> Option<Value> {
        (operation.interface_id == "applications.logs.otlp.import")
            .then(|| application::application_body_schema(operation, ep::schema::<ImportBody>()))
    }

    fn project_document_input(
        &self,
        operation: &ApiEventOperationContext,
    ) -> Result<Value, Arc<str>> {
        let (application_id, body) = match self {
            Self::OtlpExport {
                application_id,
                body,
            } => (application_id, ep::to_value(body)?),
            Self::OtlpImport {
                application_id,
                body,
            } => (
                application_id,
                ep::to_value(&ImportBody {
                    otlp: document(&body.otlp)?,
                })?,
            ),
        };
        Ok(application::request(
            operation,
            json!({"application_id":application_id}),
            json!({}),
            body,
        ))
    }
}

impl ApiEventOutput for ExchangeOutput {
    fn document_output_schema(operation: &ApiEventOperationContext) -> Option<Value> {
        (operation.interface_id == "applications.logs.otlp.export").then(ep::schema::<OtlpDocument>)
    }

    fn project_document_output(
        &self,
        _operation: &ApiEventOperationContext,
    ) -> Result<Value, Arc<str>> {
        match self {
            Self::Otlp(value) => ep::to_value(&document(value)?),
            Self::Imported(receipt) => ep::to_value(receipt),
        }
    }
}

#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
#[path = "_tests/document_event_projection.rs"]
mod tests;
