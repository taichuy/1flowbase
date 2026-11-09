//! Explicit business fields for all Agent Logs reception operations. No raw facts or credentials.
use control_plane::ports::{AgentLogsBatch, AgentLogsReceipt, AGENT_LOGS_SCHEMA_VERSION};
use interface_runtime::{InterfaceEventContext, InterfaceEventPrincipal, InvocationId};
use serde_json::{json, Value};
use uuid::Uuid;

pub(crate) const CONSOLE_IMPORT_OPERATION: &str = "applications.logs.otlp.import";
pub(crate) const OPERATIONS: &[&str] = &[super::ID, super::otlp::ID, CONSOLE_IMPORT_OPERATION];

/// An operation subject, never a native run or an external task terminal.
pub(crate) struct ApplicationLogOperationContext {
    pub application_id: Uuid,
    pub workspace_id: Uuid,
    pub invocation_id: InvocationId,
    pub operation: &'static str,
}

/// Only the sealed Application principal can supply public-ingress ownership.
/// Console import retains the route application_id in its admitted typed input.
pub(crate) fn application_operation_context(
    context: &InterfaceEventContext,
    input: Option<&Value>,
) -> Option<ApplicationLogOperationContext> {
    let operation = *OPERATIONS
        .iter()
        .find(|operation| **operation == context.definition().interface_id().as_str())?;
    let (application_id, workspace_id) = match (operation, context.principal()?) {
        (CONSOLE_IMPORT_OPERATION, InterfaceEventPrincipal::User(principal)) => (
            input?.get("application_id")?.as_str()?.parse().ok()?,
            principal.actor().current_workspace_id,
        ),
        (super::ID | super::otlp::ID, InterfaceEventPrincipal::Application(principal)) => {
            (principal.application_id(), principal.workspace_id())
        }
        _ => return None,
    };
    Some(ApplicationLogOperationContext {
        application_id,
        workspace_id,
        invocation_id: context.invocation_id(),
        operation,
    })
}

pub(crate) fn object_schema(fields: impl IntoIterator<Item = (&'static str, Value)>) -> Value {
    let properties = fields
        .into_iter()
        .map(|(name, schema)| (name.to_owned(), schema))
        .collect::<serde_json::Map<String, Value>>();
    let required = properties.keys().cloned().collect::<Vec<_>>();
    json!({"type":"object","properties":properties,"required":required,"additionalProperties":false})
}
pub(crate) fn array_schema(items: Value) -> Value {
    json!({"type":"array","items":items})
}
fn nullable(schema: Value) -> Value {
    json!({"anyOf":[schema,{"type":"null"}]})
}
pub(crate) fn batch_schema() -> Value {
    let text = || json!({"type":"string"});
    let usage = object_schema([
        (
            "basis",
            json!({"type":"string","enum":["delta","cumulative"]}),
        ),
        ("response_id", nullable(text())),
        (
            "input_tokens",
            nullable(json!({"type":"integer","minimum":0})),
        ),
        (
            "output_tokens",
            nullable(json!({"type":"integer","minimum":0})),
        ),
        (
            "input_cache_hit_tokens",
            nullable(json!({"type":"integer","minimum":0})),
        ),
        (
            "cache_write_tokens",
            nullable(json!({"type":"integer","minimum":0})),
        ),
        (
            "total_tokens",
            nullable(json!({"type":"integer","minimum":0})),
        ),
    ]);
    let event = object_schema([
        ("event_id", text()),
        ("source_session_id", text()),
        ("source_task_id", text()),
        ("parent_source_task_id", nullable(text())),
        ("sequence", json!({"type":"integer","minimum":0})),
        ("occurred_at", text()),
        (
            "kind",
            json!({"type":"string","enum":["system","user","assistant","tool_call","tool_result","usage","context","task_end"]}),
        ),
        ("content", nullable(text())),
        ("phase", nullable(text())),
        ("name", nullable(text())),
        ("call_id", nullable(text())),
        ("model_id", nullable(text())),
        ("reasoning_effort", nullable(text())),
        ("provider_code", nullable(text())),
        ("usage", nullable(usage)),
        ("inherited", json!({"type":"boolean"})),
    ]);
    object_schema([
        (
            "schema_version",
            json!({"type":"string","const":AGENT_LOGS_SCHEMA_VERSION}),
        ),
        ("source_id", text()),
        ("source_client", text()),
        ("events", array_schema(event)),
    ])
}
pub(crate) fn batch_value(batch: &AgentLogsBatch) -> Value {
    let events = batch
        .events
        .iter()
        .map(|event| {
            json!({
                "event_id":event.event_id,
                "source_session_id":event.source_session_id,
                "source_task_id":event.source_task_id,
                "parent_source_task_id":event.parent_source_task_id,
                "sequence":event.sequence,
                "occurred_at":event.occurred_at,
                "kind":event.kind,
                "content":event.content,
                "phase":event.phase,
                "name":event.name,
                "call_id":event.call_id,
                "model_id":event.model_id,
                "reasoning_effort":event.reasoning_effort,
                "provider_code":event.provider_code,
                "usage":event.usage,
                "inherited":event.inherited,
            })
        })
        .collect::<Vec<_>>();
    json!({"schema_version":batch.schema_version,"source_id":batch.source_id,"source_client":batch.source_client,"events":events})
}
pub(crate) fn receipt_schema() -> Value {
    object_schema([
        ("accepted_events", json!({"type":"integer","minimum":0})),
        ("duplicate_events", json!({"type":"integer","minimum":0})),
        (
            "record_ids",
            array_schema(json!({"type":"string","format":"uuid"})),
        ),
    ])
}
pub(crate) fn receipt_value(receipt: &AgentLogsReceipt) -> Value {
    json!({"accepted_events":receipt.accepted_events,"duplicate_events":receipt.duplicate_events,"record_ids":receipt.record_ids})
}
