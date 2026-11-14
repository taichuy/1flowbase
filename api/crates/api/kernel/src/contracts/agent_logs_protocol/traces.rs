use super::{
    attributes::{attributes, text, Attributes},
    facts::{hex, source_fact, Batches, SELF_NAMESPACE, SOURCE_FACT_ATTRIBUTE},
    observations::Observation,
    ExportTraceServiceRequest,
};
use anyhow::{ensure, Result};
use control_plane_contracts::ports::AgentLogsBatch;
use serde_json::json;

pub fn traces_to_batches(request: &ExportTraceServiceRequest) -> Result<Vec<AgentLogsBatch>> {
    let mut batches = Batches::default();
    for resource in &request.resource_spans {
        let resource_attrs = resource
            .resource
            .as_ref()
            .map(|value| attributes(&value.attributes))
            .transpose()?
            .unwrap_or_default();
        for scope in &resource.scope_spans {
            let scope_attrs = scope
                .scope
                .as_ref()
                .map(|value| attributes(&value.attributes))
                .transpose()?
                .unwrap_or_default();
            for span in &scope.spans {
                ensure!(
                    span.trace_id.len() == 16
                        && span.trace_id.iter().any(|value| *value != 0)
                        && span.span_id.len() == 8
                        && span.span_id.iter().any(|value| *value != 0),
                    "agent_logs.otlp_span_identity"
                );
                ensure!(
                    span.parent_span_id.is_empty()
                        || (span.parent_span_id.len() == 8
                            && span.parent_span_id.iter().any(|value| *value != 0)),
                    "agent_logs.otlp_parent_identity"
                );
                let span_attrs = attributes(&span.attributes)?;
                ensure!(
                    span_attrs
                        .get(SOURCE_FACT_ATTRIBUTE)
                        .is_none_or(|value| value.is_string()),
                    "agent_logs.source_fact_encoding"
                );
                let neutral = span_attrs
                    .get(SOURCE_FACT_ATTRIBUTE)
                    .map(|value| json!({SELF_NAMESPACE: value}));
                let legacy = span_attrs.get("langfuse.observation.metadata");
                ensure!(
                    neutral.is_none() || legacy.is_none(),
                    "agent_logs.source_fact_ambiguous"
                );
                if let Some(fact) = source_fact(neutral.as_ref().or(legacy))? {
                    batches.push(fact.source_id, fact.source_client, fact.event)?;
                    continue;
                }
                let mut attrs: Attributes = resource_attrs.clone();
                attrs.extend(scope_attrs.clone());
                attrs.extend(span_attrs);
                let trace = hex(&span.trace_id);
                let task = format!("{trace}/{}", hex(&span.span_id));
                let session = text(
                    &attrs,
                    &[
                        "gen_ai.conversation.id",
                        "coding_agent.session.id",
                        "session.id",
                    ],
                )
                .unwrap_or_else(|| trace.clone());
                let source_id = match (
                    text(&resource_attrs, &["service.namespace"]),
                    text(&resource_attrs, &["service.name"]),
                ) {
                    (Some(namespace), Some(name)) => format!("otlp:{namespace}:{name}"),
                    (_, Some(name)) => format!("otlp:{name}"),
                    _ => "otlp".into(),
                };
                let source_client = text(&attrs, &["coding_agent.client", "gen_ai.agent.name"])
                    .or_else(|| {
                        scope
                            .scope
                            .as_ref()
                            .map(|scope| scope.name.clone())
                            .filter(|name| !name.is_empty())
                    })
                    .or_else(|| text(&resource_attrs, &["service.name"]))
                    .unwrap_or_else(|| "otlp".into());
                let observation = Observation {
                    session,
                    task,
                    parent: (!span.parent_span_id.is_empty())
                        .then(|| format!("{trace}/{}", hex(&span.parent_span_id))),
                    name: span.name.clone(),
                    start: span.start_time_unix_nano,
                    end: span.end_time_unix_nano,
                    session_summary: span.name == "coding_agent.session",
                    raw: json!({"otlp":{"resource":resource.resource,"resource_schema_url":resource.schema_url,"scope":scope.scope,"scope_schema_url":scope.schema_url,"span":span}}),
                    attrs,
                };
                for event in observation.events()? {
                    batches.push(source_id.clone(), source_client.clone(), event)?;
                }
            }
        }
    }
    batches.finish()
}
