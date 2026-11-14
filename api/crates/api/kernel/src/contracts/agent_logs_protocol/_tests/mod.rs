use super::*;
use control_plane_contracts::ports::{
    AgentLogEvent, AgentLogEventKind, AgentLogUsage, AgentLogUsageBasis, AgentLogsBatch,
    AGENT_LOGS_SCHEMA_VERSION,
};
use opentelemetry_proto::tonic::{
    common::v1::{any_value, AnyValue},
    trace::v1::Span,
};
use serde_json::{json, Value};

mod codec;
mod generic;
mod roundtrip;

fn traces_fixture() -> ExportTraceServiceRequest {
    decode(
        "application/json",
        None,
        include_bytes!("fixtures/openlit.json"),
    )
    .unwrap()
}

fn first_span(request: &mut ExportTraceServiceRequest) -> &mut Span {
    &mut request.resource_spans[0].scope_spans[0].spans[1]
}

fn source_batches() -> Vec<AgentLogsBatch> {
    let mut events = Vec::new();
    for (index, (session, task, kind)) in [
        ("session-a", "parent", AgentLogEventKind::Context),
        ("session-a", "child", AgentLogEventKind::ToolCall),
        ("session-a", "child", AgentLogEventKind::ToolResult),
        ("session-b", "another-task", AgentLogEventKind::Assistant),
    ]
    .into_iter()
    .enumerate()
    {
        events.push(AgentLogEvent { event_id: format!("fact-{index}"), source_session_id: session.into(), source_task_id: task.into(), parent_source_task_id: (task == "child").then(|| "parent".into()), sequence: index as i64,
            occurred_at: format!("2026-10-07T08:00:0{index}.123456789Z"), kind, content: Some(format!("exact source text {index}\u{0}\n")), phase: (kind == AgentLogEventKind::Assistant).then(|| "final_answer".into()), name: Some("tool-or-event-name".into()), call_id: Some("real-call-id".into()), model_id: Some("model-original".into()), reasoning_effort: Some("high".into()), provider_code: Some("provider-original".into()),
            usage: Some(AgentLogUsage { basis: if index == 0 { AgentLogUsageBasis::Delta } else { AgentLogUsageBasis::Cumulative }, response_id: Some(format!("response-{index}")), input_tokens: Some(8), output_tokens: Some(2), total_tokens: Some(10), input_cache_hit_tokens: Some(3), cache_write_tokens: Some(4) }), inherited: index == 0, raw: json!({"original":index,"unknown":{"values":[true,null,123],"secret":"kept original"},"embedded":"nul\u{0}value"}) });
    }
    vec![AgentLogsBatch {
        schema_version: AGENT_LOGS_SCHEMA_VERSION.into(),
        source_id: "collector-a".into(),
        source_client: "codex".into(),
        events,
    }]
}

fn string_attr(span: &mut Span, key: &str, value: &str) {
    if let Some(attr) = span.attributes.iter_mut().find(|attr| attr.key == key) {
        attr.value = Some(AnyValue {
            value: Some(any_value::Value::StringValue(value.into())),
        });
    } else {
        span.attributes
            .push(super::attributes::string_attribute(key, value));
    }
}
