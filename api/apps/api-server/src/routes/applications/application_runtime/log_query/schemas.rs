//! OpenAPI projections for ports-owned DTOs; no parallel response model.
use serde_json::{json, Value};
fn object(properties: Value) -> Value {
    let required: Vec<_> = properties
        .as_object()
        .expect("schema properties")
        .keys()
        .cloned()
        .collect();
    json!({"type":"object","required":required,"properties":properties})
}
fn strings() -> Value {
    json!({"type":"array","items":{"type":"string"}})
}
fn text() -> Value {
    json!({"type":"string"})
}
fn nullable(value: Value) -> Value {
    json!({"anyOf":[value,{"type":"null"}]})
}
fn uuid() -> Value {
    json!({"type":"string","format":"uuid"})
}
fn integer() -> Value {
    json!({"type":"integer","format":"int64"})
}
fn timestamp() -> Value {
    json!({"type":"string","format":"date-time"})
}

pub(super) fn step_schema() -> Value {
    object(json!({
        "id":uuid(),"request_id":uuid(),"sequence":integer(),"created_at":timestamp(),
        "category":text(),"name":text(),"namespace":nullable(text()),"status":text(),
        "origin":text(),"protocol":text(),"transport":{"type":"string","enum":["http","websocket","file"]},
        "flow_run_id":nullable(uuid()),"node_run_id":nullable(uuid()),"parent_id":nullable(uuid()),
        "related_step_id":nullable(uuid()),"call_id":nullable(text()),"item_id":nullable(text()),
        "response_id":nullable(text()),"turn_id":nullable(text()),"preview":text(),
        "parameters_preview":nullable(text()),"result_preview":nullable(text()),"available_sections":strings()
    }))
}
fn cost_schema() -> Value {
    object(json!({"total_cost":nullable(text())}))
}
fn summary_schema() -> Value {
    object(json!({
        "record_id":uuid(),"application_id":uuid(),"source_kind":{"type":"string","enum":["native","imported"]},
        "source_id":nullable(text()),"source_client":nullable(text()),"source_session_id":nullable(text()),
        "source_task_id":nullable(text()),"native_run_id":nullable(uuid()),"log_conversation_id":nullable(uuid()),
        "requested_model_id":nullable(text()),"reasoning_effort":nullable(text()),"status":text(),"outcome":text(),
        "title":text(),"total_tokens":nullable(integer()),"input_tokens":nullable(integer()),"output_tokens":nullable(integer()),
        "input_cache_hit_tokens":nullable(integer()),"total_cost":nullable(text()),"cost_breakdown":cost_schema(),
        "started_at":timestamp(),"finished_at":nullable(timestamp()),"created_at":timestamp(),"updated_at":timestamp(),
        "available_views":strings()
    }))
}
fn log_records_page_schema() -> Value {
    object(
        json!({"items":{"type":"array","items":summary_schema()},"next_cursor":nullable(text())}),
    )
}
fn trajectory_query_page_schema() -> Value {
    object(json!({
        "items":{"type":"array","items":step_schema()},
        "matches":{"type":"array","items":object(json!({"step_id":uuid(),"section":text(),"sequence":integer(),"snippet":text()}))},
        "next_cursor":nullable(text()),"search_sections":strings(),"integrity":text()
    }))
}
fn query_fields_schema() -> Value {
    let fields = json!({"type":"array","items":object(json!({"field":text(),"value_type":{"type":"string","enum":["string","uuid","number","boolean","datetime"]},"operators":strings(),"sortable":{"type":"boolean"}}))});
    object(json!({
        "record_fields":fields,"trajectory_fields":fields,"logical_operators":strings(),
        "record_keyword_fields":strings(),"trajectory_search_sections":strings(),
        "record_contains_semantics":text(),"trajectory_keyword_semantics":text(),"pagination":text()
    }))
}
fn record_overview_schema() -> Value {
    object(json!({
        "record_id":uuid(),"source_kind":text(),"source_id":nullable(text()),"source_client":nullable(text()),
        "source_session_id":nullable(text()),"source_task_id":nullable(text()),"native_run_id":nullable(uuid()),
        "status":text(),"title":text(),"outcome":text(),"projection_output":nullable(text()),
        "output_state":nullable(object(json!({"flow_run_id":uuid(),"status":text(),"call_kind":text(),"request_kind":nullable(text()),"output_source":text(),"output_item_count":integer()}))),
        "messages":{"type":"array","items":object(json!({"role":text(),"content":text(),"sequence":integer()}))},
        "total_tokens":nullable(integer()),"input_tokens":nullable(integer()),"output_tokens":nullable(integer()),
        "input_cache_hit_tokens":nullable(integer()),"cost_breakdown":cost_schema(),"available_views":strings()
    }))
}
fn record_trajectory_page_schema() -> Value {
    object(
        json!({"items":{"type":"array","items":step_schema()},"next_cursor":{"anyOf":[text(),integer(),{"type":"null"}]},"integrity":text()}),
    )
}
fn trajectory_section_schema() -> Value {
    object(
        json!({"step_id":uuid(),"request_id":uuid(),"evidence_scope":text(),"section":text(),"items":{"type":"array","items":object(json!({"sequence":integer(),"value":{}}))},"next_cursor":nullable(integer())}),
    )
}
macro_rules! projection {
    ($name:ident, $schema:ident) => {
        pub struct $name;
        impl utoipa::ToSchema for $name {}
        impl utoipa::PartialSchema for $name {
            fn schema() -> utoipa::openapi::RefOr<utoipa::openapi::schema::Schema> {
                serde_json::from_value($schema()).expect("static log query schema")
            }
        }
    };
}
projection!(LogRecordsPageSchema, log_records_page_schema);
projection!(TrajectoryQueryPageSchema, trajectory_query_page_schema);
projection!(QueryFieldsSchema, query_fields_schema);

projection!(RecordOverviewSchema, record_overview_schema);
projection!(RecordTrajectoryPageSchema, record_trajectory_page_schema);
projection!(TrajectorySectionSchema, trajectory_section_schema);
