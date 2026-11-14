use crate::openapi_interface::contract as ep;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
#[derive(Deserialize, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct OtlpDocument {
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

pub fn document(value: &Value) -> Result<OtlpDocument, Arc<str>> {
    // Decode through the actual protocol DTO first. This preserves canonical OTLP
    // hex IDs and string-encoded nanoseconds before selecting the safe fields.
    let request: opentelemetry_proto::tonic::collector::trace::v1::ExportTraceServiceRequest =
        serde_json::from_value(value.clone())
            .map_err(|_| Arc::from("invalid_typed_otlp_document"))?;
    serde_json::from_value(ep::to_value(&request)?)
        .map_err(|_| Arc::from("invalid_safe_otlp_document"))
}
