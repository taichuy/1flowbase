use super::{
    attributes::{int_attribute, string_attribute},
    facts::{encode_source_fact, hash, nanos, SourceFact, SOURCE_FACT_ATTRIBUTE},
    ExportTraceServiceRequest,
};
use anyhow::Result;
use control_plane_contracts::ports::{
    AgentLogEvent, AgentLogEventKind, AgentLogUsageBasis, AgentLogsBatch,
};
use opentelemetry_proto::tonic::{
    common::v1::InstrumentationScope,
    resource::v1::Resource,
    trace::v1::{ResourceSpans, ScopeSpans, Span},
};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};

type TaskIdentity = (String, String, String, String);

fn span_identity(
    batch: &AgentLogsBatch,
    event: &control_plane_contracts::ports::AgentLogEvent,
) -> Result<Vec<u8>> {
    Ok(hash(&json!([
        "1flowbase.span.v1",
        batch.source_id,
        batch.source_client,
        event.source_session_id,
        event.source_task_id,
        event.event_id
    ]))?[..8]
        .to_vec())
}

/// Each selected source event is exactly one observation. The metadata envelope is
/// the replay authority; display attributes never replace or meter the source fact.
pub fn batches_to_traces(batches: &[AgentLogsBatch]) -> Result<ExportTraceServiceRequest> {
    let mut anchors = BTreeMap::<TaskIdentity, (i64, String, Vec<u8>)>::new();
    for batch in batches {
        batch.validate()?;
        for event in &batch.events {
            let key = (
                batch.source_id.clone(),
                batch.source_client.clone(),
                event.source_session_id.clone(),
                event.source_task_id.clone(),
            );
            let candidate = (
                event.sequence,
                event.event_id.clone(),
                span_identity(batch, event)?,
            );
            if anchors
                .get(&key)
                .is_none_or(|existing| (&candidate.0, &candidate.1) < (&existing.0, &existing.1))
            {
                anchors.insert(key, candidate);
            }
        }
    }
    let metered = metered_events(batches);
    let mut resources = Vec::new();
    for batch in batches {
        let mut spans = Vec::new();
        for event in &batch.events {
            let span_id = span_identity(batch, event)?;
            let task_key = (
                batch.source_id.clone(),
                batch.source_client.clone(),
                event.source_session_id.clone(),
                event.source_task_id.clone(),
            );
            let anchor = &anchors[&task_key].2;
            let parent_span_id = if &span_id != anchor {
                anchor.clone()
            } else {
                event
                    .parent_source_task_id
                    .as_ref()
                    .and_then(|parent| {
                        anchors.get(&(
                            batch.source_id.clone(),
                            batch.source_client.clone(),
                            event.source_session_id.clone(),
                            parent.clone(),
                        ))
                    })
                    .map(|(_, _, id)| id.clone())
                    .unwrap_or_default()
            };
            let timestamp = nanos(&event.occurred_at)?;
            let fact = SourceFact {
                schema_version: batch.schema_version.clone(),
                source_id: batch.source_id.clone(),
                source_client: batch.source_client.clone(),
                event: event.clone(),
            };
            let mut attrs = vec![
                string_attribute(SOURCE_FACT_ATTRIBUTE, encode_source_fact(&fact)?),
                string_attribute("session.id", &event.source_session_id),
                string_attribute("gen_ai.conversation.id", &event.source_session_id),
                string_attribute("coding_agent.client", &batch.source_client),
                string_attribute("1flowbase.source_task_id", &event.source_task_id),
            ];
            if let Some(content) = &event.content {
                let key = match event.kind {
                    AgentLogEventKind::Assistant
                    | AgentLogEventKind::ToolResult
                    | AgentLogEventKind::TaskEnd => "gen_ai.output.messages",
                    _ => "gen_ai.input.messages",
                };
                let role = match event.kind {
                    AgentLogEventKind::System => "system",
                    AgentLogEventKind::User => "user",
                    AgentLogEventKind::Assistant | AgentLogEventKind::ToolCall => "assistant",
                    AgentLogEventKind::ToolResult => "tool",
                    _ => "context",
                };
                attrs.push(string_attribute(
                    key,
                    serde_json::to_string(&json!([
                        {"role":role,"parts":[{"type":"text","content":content}]}
                    ]))?,
                ));
            }
            if let Some(model) = &event.model_id {
                attrs.push(string_attribute("gen_ai.response.model", model));
            }
            if let Some(provider) = &event.provider_code {
                attrs.push(string_attribute("gen_ai.provider.name", provider));
            }
            if metered.contains(&(
                batch.source_id.clone(),
                batch.source_client.clone(),
                event.event_id.clone(),
            )) {
                let usage = event.usage.as_ref().expect("selected usage event");
                for (_dimension, key, value) in [
                    ("input", "gen_ai.usage.input_tokens", usage.input_tokens),
                    ("output", "gen_ai.usage.output_tokens", usage.output_tokens),
                    ("total", "gen_ai.usage.total_tokens", usage.total_tokens),
                    (
                        "cache_read_input_tokens",
                        "gen_ai.usage.cache.read_input_tokens",
                        usage.input_cache_hit_tokens,
                    ),
                    (
                        "cache_creation_input_tokens",
                        "gen_ai.usage.cache.creation_input_tokens",
                        usage.cache_write_tokens,
                    ),
                ] {
                    if let Some(value) = value {
                        attrs.push(int_attribute(key, value));
                    }
                }
            }
            if let Some(name) = &event.name {
                attrs.push(string_attribute("gen_ai.tool.name", name));
            }
            if let Some(call_id) = &event.call_id {
                attrs.push(string_attribute("gen_ai.tool.call.id", call_id));
            }
            spans.push(Span {
                trace_id: hash(&json!([
                    "1flowbase.trace.v1",
                    batch.source_id,
                    batch.source_client,
                    event.source_session_id
                ]))?[..16]
                    .to_vec(),
                span_id,
                parent_span_id,
                name: event.name.clone().unwrap_or_else(|| {
                    format!(
                        "agent-log.{}",
                        serde_json::to_value(event.kind)
                            .expect("enum serialization")
                            .as_str()
                            .expect("enum string")
                    )
                }),
                kind: 1,
                start_time_unix_nano: timestamp,
                end_time_unix_nano: timestamp,
                attributes: attrs,
                ..Default::default()
            });
        }
        resources.push(ResourceSpans {
            resource: Some(Resource {
                attributes: vec![
                    string_attribute("service.name", "1flowbase.agent-logs"),
                    string_attribute("service.namespace", &batch.source_id),
                ],
                ..Default::default()
            }),
            scope_spans: vec![ScopeSpans {
                scope: Some(InstrumentationScope {
                    name: "1flowbase.agent-logs".into(),
                    version: "1".into(),
                    ..Default::default()
                }),
                spans,
                ..Default::default()
            }],
            ..Default::default()
        });
    }
    Ok(ExportTraceServiceRequest {
        resource_spans: resources,
    })
}

/// Match canonical per-record accounting: inherited is excluded, deltas take
/// precedence, and cumulative usage without a response identity is unowned.
fn metered_events(batches: &[AgentLogsBatch]) -> BTreeSet<(String, String, String)> {
    let mut groups =
        BTreeMap::<(TaskIdentity, String), Vec<(&AgentLogsBatch, &AgentLogEvent)>>::new();
    for batch in batches {
        for event in &batch.events {
            let Some(usage) = &event.usage else {
                continue;
            };
            if event.inherited
                || (usage.basis == AgentLogUsageBasis::Cumulative
                    && usage.response_id.as_deref().is_none_or(str::is_empty))
            {
                continue;
            }
            let identity = (
                batch.source_id.clone(),
                batch.source_client.clone(),
                event.source_session_id.clone(),
                event.source_task_id.clone(),
            );
            groups
                .entry((identity, usage.response_id.clone().unwrap_or_default()))
                .or_default()
                .push((batch, event));
        }
    }
    let mut selected = BTreeSet::new();
    for events in groups.values_mut() {
        events.sort_by(|(_, left), (_, right)| {
            (left.sequence, &left.event_id).cmp(&(right.sequence, &right.event_id))
        });
        let deltas: Vec<_> = events
            .iter()
            .filter(|(_, event)| {
                event
                    .usage
                    .as_ref()
                    .is_some_and(|usage| usage.basis == AgentLogUsageBasis::Delta)
            })
            .collect();
        let contributions = if deltas.is_empty() {
            events.last().into_iter().collect::<Vec<_>>()
        } else {
            deltas
        };
        for (batch, event) in contributions {
            selected.insert((
                batch.source_id.clone(),
                batch.source_client.clone(),
                event.event_id.clone(),
            ));
        }
    }
    selected
}
