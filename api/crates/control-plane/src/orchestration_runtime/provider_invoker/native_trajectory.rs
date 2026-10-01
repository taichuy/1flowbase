//! Observation at the Native execution boundary, not protocol translation or billing.
//! Input is the actual value handed to the plugin; output is what the plugin returned.
//! The repository owns storage/query. Child steps reference immutable invocation-local
//! snapshots; observation failure never changes execution or client protocol facts.
use super::*;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};

#[cfg(test)]
const CAPACITY: usize = 1024 * 1024;
const RECORDS: usize = 128;
static WRITERS: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(32);

#[derive(Clone)]
struct Identity {
    run: Uuid,
    node: String,
    node_run: Uuid,
    invocation: Uuid,
    attempt: u64,
    observation_context: Option<control_plane_contracts::ports::WorkflowObservationContext>,
    purpose: &'static str,
    duration_ms: Arc<AtomicU64>,
}
impl Identity {
    fn event(&self, kind: &str, mut value: Value) -> crate::ports::RuntimeEventPayload {
        value["type"] = json!(kind);
        value["source"] = json!("ai_native");
        value["flow_run_id"] = json!(self.run);
        value["node_id"] = json!(self.node);
        value["node_run_id"] = json!(self.node_run);
        value["invocation_id"] = json!(self.invocation);
        value["provider_attempt_index"] = json!(self.attempt);
        value["trigger_request_id"] = json!(self
            .observation_context
            .as_ref()
            .map(|context| context.client_request_id));
        value["context_flow_run_id"] = json!(self
            .observation_context
            .as_ref()
            .and_then(|context| context.context_flow_run_id));
        value["context_response_id"] = json!(self
            .observation_context
            .as_ref()
            .and_then(|context| context.context_response_id.as_deref()));
        value["purpose"] = json!(self.purpose);
        let duration = self.duration_ms.load(Relaxed);
        value["duration_ms"] = json!((duration != u64::MAX).then_some(duration));
        crate::ports::RuntimeEventPayload {
            event_type: kind.into(),
            source: crate::ports::RuntimeEventSource::Provider,
            durability: RuntimeEventDurability::DurableRequired,
            persist_required: true,
            trace_visible: false,
            payload: value,
        }
    }
}

// Immutable actual values are archived once; size cannot erase evidence.
fn snapshot_value<T: serde::Serialize + ?Sized>(value: &T) -> Option<Value> {
    serde_json::to_value(value).ok()
}
// This applies only to Native snapshots, never to execution values. Unknown ordinary
// metadata survives; authentication containers and credential-bearing fields do not.
fn redact(value: &mut Value) {
    match value {
        Value::Object(map) => {
            map.retain(|key, _| {
                let key = key.to_ascii_lowercase().replace(['-', '_'], "");
                ![
                    "auth",
                    "authentication",
                    "authorization",
                    "proxyauthorization",
                    "credential",
                    "credentials",
                    "secret",
                    "clientsecret",
                    "token",
                    "password",
                    "apikey",
                    "accesstoken",
                    "refreshtoken",
                    "headers",
                    "cookie",
                    "providerconfig",
                    "runcontext",
                    "clientprotocolenvelope",
                    "nativetransport",
                ]
                .iter()
                .any(|part| key == *part)
            });
            for value in map.values_mut() {
                redact(value);
            }
        }
        Value::Array(values) => values.iter_mut().for_each(redact),
        _ => {}
    }
}
#[derive(serde::Serialize)]
struct SafeInput<'a> {
    operation: &'a plugin_framework::provider_contract::ProviderWireOperation,
    model: &'a str,
    provider_code: &'a str,
    protocol: &'a str,
    messages: &'a [plugin_framework::provider_contract::ProviderMessage],
    system: &'a [plugin_framework::provider_contract::NativePromptBlock],
    tools: &'a [Value],
    response_format: &'a Option<Value>,
    model_parameters: &'a BTreeMap<String, Value>,
    // Already sealed Native request body; no network headers or authentication envelope.
    native_request: &'a Option<plugin_framework::provider_contract::ProviderNativeTransport>,
    previous_response_id: &'a Option<String>,
}
fn safe_input(input: &ProviderInvocationInput) -> Option<Value> {
    snapshot_value(&SafeInput {
        operation: &input.operation,
        model: &input.model,
        provider_code: &input.provider_code,
        protocol: &input.protocol,
        messages: &input.messages,
        system: &input.system,
        tools: &input.tools,
        response_format: &input.response_format,
        model_parameters: &input.model_parameters,
        native_request: &input.native_transport,
        previous_response_id: &input.previous_response_id,
    })
}

// Only metadata/diagnostics are scrubbed. User content, tool arguments and schema
// properties retain their Native meaning even when they are named password/authorship.
fn redact_metadata(value: &mut Value) {
    match value {
        Value::Object(map) => {
            for (key, value) in map {
                if key == "provider_metadata" || key == "provider_details" {
                    redact(value);
                } else {
                    redact_metadata(value);
                }
            }
        }
        Value::Array(values) => values.iter_mut().for_each(redact_metadata),
        _ => {}
    }
}
async fn record_input(sink: &Sink, input: &ProviderInvocationInput) -> bool {
    let Some(detail) = safe_input(input) else {
        sink.step(
            "model_call",
            &Uuid::now_v7().to_string(),
            "prepared",
            json!({"truncated":true}),
            None,
            true,
        )
        .await;
        return true;
    };
    let snapshot_key = Uuid::now_v7().to_string();
    let mut entries = Vec::new();
    let mut submitted = BTreeSet::new();
    let mut occurrence = |key: String, detail: &Value, tool: Option<&str>, pointer: String| {
        entries.push(json!({"event_id":Uuid::now_v7(), "metadata": {
            "step_key":key, "kind":"tool_result", "status":"recorded",
            "direction":"prepared", "preview":native_preview("tool_result", detail),
            "tool_call_id":tool, "body_ref":{"step_key":snapshot_key,"pointer":pointer}
        }}));
    };
    for (index, message) in input
        .messages
        .iter()
        .enumerate()
        .filter(|(_, message)| message.role == ProviderMessageRole::Tool)
    {
        occurrence(
            format!("submitted:{index}"),
            &detail["messages"][index],
            message.tool_call_id.as_deref(),
            format!("/messages/{index}"),
        );
        if let Some(id) = &message.tool_call_id {
            submitted.insert(id.clone());
        }
    }
    if let Some(items) = detail["native_request"]["wire_body"]["input"].as_array() {
        for (index, item) in items.iter().enumerate() {
            let id = item["call_id"].as_str();
            if item["type"] == "function_call_output"
                && !id.is_some_and(|id| submitted.contains(id))
            {
                occurrence(
                    format!("native-submitted:{index}"),
                    item,
                    id,
                    format!("/native_request/wire_body/input/{index}"),
                );
            }
        }
    }
    let count = entries.len() as u64;
    let mut payload = sink.id.event(
        "provider_semantic_step",
        json!({
            "step_key":snapshot_key,"kind":"model_call","status":"recorded",
            "direction":"prepared","preview":native_preview("model_call", &detail),
            "tool_call_id":null,"body":detail.to_string()
        }),
    );
    if !entries.is_empty() {
        payload.payload["_context_occurrences"] = json!({"version":1,"entries":entries});
    }
    // One durable record owns the exact input and all ordered occurrence identities.
    // The repository reserves their cursor positions and projects the old list on reads.
    let saved = sink.enqueue(payload).await;
    if saved {
        sink.observed.fetch_add(count, Relaxed);
    }
    !saved
}

#[derive(Default)]
struct Aggregate {
    text: String,
    reasoning: String,
    signature: String,
    open_items: BTreeSet<usize>,
    open_tools: BTreeSet<String>,
    items: BTreeMap<usize, Value>,
    tools: BTreeMap<String, Value>,
    errors: Vec<Value>,
    partial_items: BTreeMap<usize, Value>,
    partial_tools: BTreeMap<String, Value>,
    partial_content: BTreeMap<(usize, String, u64), String>,
    extensions: Vec<Value>,
    gap: bool,
}
impl Aggregate {
    fn observe(&mut self, event: &ProviderStreamEvent) {
        // Chunks are accumulated, never emitted as semantic steps.
        match event {
            ProviderStreamEvent::TextDelta { delta }
            | ProviderStreamEvent::ReasoningDelta { delta } => {
                if matches!(event, ProviderStreamEvent::TextDelta { .. }) {
                    self.text.push_str(delta);
                } else {
                    self.reasoning.push_str(delta);
                }
            }
            ProviderStreamEvent::ReasoningSignatureDelta { signature } => {
                self.signature.push_str(signature);
            }
            ProviderStreamEvent::OutputItem {
                phase,
                output_index,
                item,
            } => {
                if *phase == ProviderOutputItemPhase::Added {
                    self.open_items.insert(*output_index);
                    self.partial_items.insert(*output_index, item.clone());
                    return;
                }
                self.open_items.remove(output_index);
                self.partial_items.remove(output_index);
                self.partial_content
                    .retain(|(index, _, _), _| index != output_index);
                if let Some(value) = self.admit(item) {
                    self.items.insert(*output_index, value);
                }
            }
            ProviderStreamEvent::ResponsesOutputDelta { event } => {
                if let Some(index) = event["output_index"].as_u64() {
                    let index = index as usize;
                    if !self.open_items.contains(&index) {
                        self.gap = true;
                    }
                    let kind = event["type"].as_str().unwrap_or_default();
                    let content_index = event["content_index"]
                        .as_u64()
                        .or_else(|| event["summary_index"].as_u64())
                        .unwrap_or(0);
                    if let Some(delta) = event["delta"].as_str() {
                        self.partial_content
                            .entry((index, kind.trim_end_matches(".delta").into(), content_index))
                            .or_default()
                            .push_str(delta);
                    } else {
                        // Part/done snapshots and unknown fields are actual facts,
                        // not disposable text fragments.
                        self.extensions.push(event.clone());
                    }
                    let mut extra = event.clone();
                    if let Some(fields) = extra.as_object_mut() {
                        for key in [
                            "type",
                            "delta",
                            "output_index",
                            "item_id",
                            "content_index",
                            "summary_index",
                            "sequence_number",
                        ] {
                            fields.remove(key);
                        }
                        if !fields.is_empty() && event.get("delta").is_some() {
                            self.extensions.push(json!({"output_index":index,"item_id":event["item_id"],"fields":extra}));
                        }
                    }
                } else {
                    self.gap = true;
                    self.extensions.push(event.clone());
                }
            }
            ProviderStreamEvent::ToolCallDelta { call_id, delta }
            | ProviderStreamEvent::McpCallDelta { call_id, delta } => {
                self.open_tools.insert(call_id.clone());
                let partial = self
                    .partial_tools
                    .entry(call_id.clone())
                    .or_insert_with(|| json!({}));
                merge_partial_value(partial, delta);
                // A commit replaces argument fragments, but cannot erase unique
                // vendor evidence carried beside them.
                if let Some(fields) = delta.as_object() {
                    let extra = fields
                        .iter()
                        .filter(|(key, _)| {
                            !matches!(
                                key.as_str(),
                                "arguments" | "name" | "id" | "call_id" | "type"
                            )
                        })
                        .map(|(key, value)| (key.clone(), value.clone()))
                        .collect::<serde_json::Map<_, _>>();
                    if !extra.is_empty() {
                        self.extensions
                            .push(json!({"call_id":call_id,"fields":extra}));
                    }
                }
            }
            ProviderStreamEvent::ToolCallCommit { call } => {
                self.open_tools.remove(&call.id);
                self.partial_tools.remove(&call.id);
                if let Some(value) = self.admit(call) {
                    self.tools.insert(call.id.clone(), value);
                }
            }
            ProviderStreamEvent::McpCallCommit { call } => {
                self.open_tools.remove(&call.id);
                self.partial_tools.remove(&call.id);
                if let Some(value) = self.admit(call) {
                    self.tools.insert(call.id.clone(), value);
                }
            }
            ProviderStreamEvent::Error { error } => {
                if let Some(value) = snapshot_value(error) {
                    self.errors.push(value);
                } else {
                    self.gap = true;
                }
            }
            ProviderStreamEvent::OutputProtocolFailure { failure } => {
                self.gap = true;
                if let Some(value) = self.admit(failure) {
                    self.errors.push(value);
                }
            }
            _ => {}
        }
    }
    fn admit<T: serde::Serialize + ?Sized>(&mut self, value: &T) -> Option<Value> {
        let Some(mut value) = snapshot_value(value) else {
            self.gap = true;
            return None;
        };
        redact_metadata(&mut value);
        Some(value)
    }
}
// Merge strings only inside a single identified call. Preserve structured fields.
fn merge_partial_value(target: &mut Value, delta: &Value) {
    match (target, delta) {
        (Value::String(target), Value::String(delta)) => target.push_str(delta),
        (Value::Object(target), Value::Object(delta)) => {
            for (key, value) in delta {
                if let Some(existing) = target.get_mut(key) {
                    merge_partial_value(existing, value);
                } else {
                    target.insert(key.clone(), value.clone());
                }
            }
        }
        (target, delta) => *target = delta.clone(),
    }
}
struct Record {
    payload: crate::ports::RuntimeEventPayload,
}
struct Sink {
    id: Identity,
    sender: mpsc::Sender<Record>,
    observed: Arc<AtomicU64>,
    dropped: Arc<AtomicU64>,
}
// Read only declared Native content fields; arbitrary metadata is never promoted
// into user-visible text. Bound the output while traversing, not after allocation.
fn native_preview(kind: &str, detail: &Value) -> String {
    fn content(value: &Value, output: &mut String, remaining: &mut usize, depth: usize) {
        if *remaining == 0 || depth > 8 {
            return;
        }
        match value {
            Value::String(text) if !text.is_empty() => {
                if !output.is_empty() {
                    output.push(' ');
                    *remaining -= 1;
                }
                for ch in text.chars().take(*remaining) {
                    output.push(ch);
                    *remaining -= 1;
                }
            }
            Value::Array(values) => {
                for value in values {
                    content(value, output, remaining, depth + 1);
                    if *remaining == 0 {
                        break;
                    }
                }
            }
            Value::Object(_) => {
                for field in ["text", "content", "output", "final_content"] {
                    if let Some(value) = value.get(field) {
                        content(value, output, remaining, depth + 1);
                    }
                }
            }
            _ => {}
        }
    }
    let mut output = String::new();
    let mut remaining = 240;
    let mut add = |value: &Value| content(value, &mut output, &mut remaining, 0);
    match kind {
        "model_call" => {
            if let Some(messages) = detail["messages"].as_array() {
                if let Some(message) = messages.last() {
                    add(message);
                }
            }
            if output.is_empty() {
                content(
                    &detail["native_request"]["wire_body"]["input"],
                    &mut output,
                    &mut remaining,
                    0,
                );
            }
            if output.is_empty() {
                content(&detail["model"], &mut output, &mut remaining, 0);
            }
        }
        "model_reply" => {
            add(&detail["final_content"]);
            if output.is_empty() {
                content(&detail["output_items"], &mut output, &mut remaining, 0);
            }
            if output.is_empty() {
                content(&detail["reasoning"], &mut output, &mut remaining, 0);
            }
        }
        "tool_call" => {
            add(&detail["name"]);
            if output.is_empty() {
                content(&detail["function"]["name"], &mut output, &mut remaining, 0);
            }
        }
        "tool_result" => add(detail),
        "error" => add(&detail["code"]),
        _ => {}
    }
    output
}

impl Sink {
    async fn step(
        &self,
        kind: &str,
        key: &str,
        direction: &str,
        detail: Value,
        tool: Option<&str>,
        incomplete: bool,
    ) -> bool {
        let mut detail = detail;
        let preview = native_preview(kind, &detail);
        // Exact duplicate only. The reader restores the public detail shape.
        let compact_reply = kind == "model_reply"
            && detail["result"].is_object()
            && detail.get("final_content") == detail["result"].get("final_content");
        if compact_reply {
            if let Some(result) = detail["result"].as_object_mut() {
                result.remove("final_content");
            }
        }
        let body = detail.to_string();
        let mut payload = self.id.event("provider_semantic_step", json!({"step_key":key,"kind":kind,
            "status":if incomplete {"incomplete"} else {"recorded"},"direction":direction,"preview":preview,
            "tool_call_id":tool,"body":body}));
        if compact_reply {
            payload.payload["body_format"] = json!("native_reply_v2");
        }
        self.enqueue(payload).await
    }

    async fn referenced_step(
        &self,
        kind: &str,
        key: &str,
        direction: &str,
        detail: Value,
        tool: Option<&str>,
        reference: Option<(&str, String)>,
    ) {
        let Some((step_key, pointer)) = reference else {
            self.step(kind, key, direction, detail, tool, false).await;
            return;
        };
        let preview = native_preview(kind, &detail);
        self.enqueue(self.id.event(
            "provider_semantic_step",
            json!({
                "step_key":key, "kind":kind, "status":"recorded", "direction":direction,
                "preview":preview, "tool_call_id":tool,
                "body_ref":{"step_key":step_key,"pointer":pointer}
            }),
        ))
        .await;
    }

    async fn enqueue(&self, payload: crate::ports::RuntimeEventPayload) -> bool {
        if self.sender.send(Record { payload }).await.is_ok() {
            self.observed.fetch_add(1, Relaxed);
            return true;
        }
        self.dropped.fetch_add(1, Relaxed);
        false
    }
}

/// Only this owner signals completion; dropping it always marks collection incomplete.
#[derive(Default)]
pub(super) struct Capture {
    sink: Option<Sink>,
    aggregate: Arc<std::sync::Mutex<Aggregate>>,
    completion: Option<tokio::sync::oneshot::Sender<bool>>,
    started_at: Option<std::time::Instant>,
    writer: Option<tokio::task::JoinHandle<()>>,
}
impl Drop for Capture {
    fn drop(&mut self) {
        self.record_duration();
        if let Some(done) = self.completion.take() {
            let _ = done.send(false);
        }
    }
}
impl Capture {
    fn record_duration(&self) {
        if let (Some(sink), Some(started)) = (&self.sink, self.started_at) {
            let elapsed = started.elapsed().as_millis().min((u64::MAX - 1) as u128) as u64;
            let _ = sink
                .id
                .duration_ms
                .compare_exchange(u64::MAX, elapsed, Relaxed, Relaxed);
        }
    }

    /// Remote compaction has its own typed result; preserve it without inventing generated text.
    pub(super) async fn finish_compact(
        mut self,
        result: Option<&plugin_framework::provider_contract::ProviderCompactResult>,
        failed: bool,
    ) {
        self.record_duration();
        let Some(sink) = self.sink.as_ref() else {
            return;
        };
        let mut complete = self.aggregate.lock().is_ok_and(|state| !state.gap);
        if let Some(result) = result {
            let detail = snapshot_value(result);
            complete &= detail.is_some();
            sink.step(
                "model_reply",
                &Uuid::now_v7().to_string(),
                "received",
                detail.unwrap_or_else(|| json!({"truncated":true})),
                None,
                !complete,
            )
            .await;
        }
        if failed {
            sink.step(
                "error",
                "error:invocation",
                "received",
                json!({"message":"Native compaction failed"}),
                None,
                false,
            )
            .await;
        }
        if !complete {
            sink.dropped.fetch_add(1, Relaxed);
        }
        if let Some(done) = self.completion.take() {
            let _ = done.send(complete);
        }
        if let Some(writer) = self.writer.take() {
            let _ = writer.await;
        }
    }

    pub(super) fn observer(&self) -> Observer {
        Observer(self.sink.as_ref().map(|_| self.aggregate.clone()))
    }
    pub(super) async fn finish(
        mut self,
        result: Option<&plugin_framework::provider_contract::ProviderInvocationResult>,
        error: Option<&str>,
        forwarding_ok: bool,
    ) {
        self.record_duration();
        let Some(sink) = self.sink.as_ref() else {
            return;
        };
        let mut state = match self.aggregate.lock() {
            Ok(mut state) => std::mem::take(&mut *state),
            Err(_) => return,
        };
        let result = result.and_then(|result| {
            let mut value = snapshot_value(result);
            if let Some(value) = value.as_mut() {
                redact_metadata(value);
            }
            if value.is_none() {
                state.gap = true;
            }
            value
        });
        if let Some(result) = &result {
            for field in ["tool_calls", "mcp_calls"] {
                for call in result[field].as_array().into_iter().flatten() {
                    if let Some(id) = call["id"].as_str() {
                        if !state.tools.contains_key(id) {
                            if let Some(value) = state.admit(call) {
                                state.tools.insert(id.into(), value);
                            }
                        }
                    }
                }
            }
        }
        // Typed output items may contain committed tools; prefer that richer fact once.
        let items = std::mem::take(&mut state.items);
        let mut reply_items = Vec::new();
        for (_, item) in items {
            if item["type"]
                .as_str()
                .is_some_and(|kind| kind.ends_with("_call"))
            {
                if let Some(id) = item
                    .get("call_id")
                    .or_else(|| item.get("id"))
                    .and_then(Value::as_str)
                {
                    // Done carries both Native identities: argument deltas may use
                    // the output-item id while the displayed tool uses call_id.
                    // Close only aliases explicitly attached to this committed item.
                    if let Some(item_id) = item.get("id").and_then(Value::as_str) {
                        state.open_tools.remove(item_id);
                    }
                    state.open_tools.remove(id);
                    state.tools.insert(id.into(), item);
                } else {
                    state.gap = true;
                }
            } else {
                reply_items.push(item);
            }
        }
        // Native passthrough may commit a tool solely through OutputItem::Done.
        // Resolve those facts before deciding whether argument streams stayed open.
        for id in state.tools.keys().cloned().collect::<Vec<_>>() {
            state.open_tools.remove(&id);
        }
        let unclosed = !state.open_items.is_empty() || !state.open_tools.is_empty();
        state.gap |= unclosed;
        let text = result
            .as_ref()
            .and_then(|value| value["final_content"].as_str())
            .unwrap_or(&state.text);
        let reply_key = Uuid::now_v7().to_string();
        let mut saved_reply = false;
        if result.is_some()
            || !text.is_empty()
            || !state.reasoning.is_empty()
            || !state.signature.is_empty()
            || !state.partial_content.is_empty()
            || !reply_items.is_empty()
            || !state.partial_items.is_empty()
            || !state.partial_tools.is_empty()
            || !state.extensions.is_empty()
        {
            saved_reply = sink.step(
                "model_reply",
                &reply_key,
                "received",
                json!({"final_content":text,"reasoning":state.reasoning,
            "reasoning_signature":state.signature,"output_items":reply_items,"result":result,
            "partial_output_items":state.partial_items.values().collect::<Vec<_>>(),
            "partial_content":state.partial_content.iter().map(|((index,kind,content_index),text)|json!({"output_index":index,"kind":kind,"content_index":content_index,"text":text})).collect::<Vec<_>>(),
            "partial_tool_calls":state.partial_tools.iter().filter(|(id,_)|state.open_tools.contains(*id)).map(|(id,value)|json!({"call_id":id,"delta":value})).collect::<Vec<_>>(),
            "extensions":state.extensions}),
                None,
                state.gap,
            ).await;
        }
        for (id, call) in &state.tools {
            // A stream output item may carry different metadata from the final
            // result. Share only exact values; never infer equality from call_id.
            let pointer = saved_reply
                .then(|| {
                    result.as_ref().and_then(|result| {
                        ["tool_calls", "mcp_calls"].into_iter().find_map(|field| {
                            result[field]
                                .as_array()?
                                .iter()
                                .position(|value| value == call)
                                .map(|index| format!("/result/{field}/{index}"))
                        })
                    })
                })
                .flatten();
            sink.referenced_step(
                "tool_call",
                &format!("tool:{id}"),
                "received",
                call.clone(),
                Some(id),
                pointer.map(|pointer| (reply_key.as_str(), pointer)),
            )
            .await;
        }
        for (index, error) in state.errors.iter().enumerate() {
            sink.step(
                "error",
                &format!("error:{index}"),
                "received",
                error.clone(),
                None,
                false,
            )
            .await;
        }
        if let Some(error) = error {
            if state.errors.is_empty() {
                // Runtime errors may contain transport diagnostics: retain the fact, not arbitrary error text.
                let _ = error;
                sink.step(
                    "error",
                    "error:invocation",
                    "received",
                    json!({"message":"Native invocation failed"}),
                    None,
                    false,
                )
                .await;
            }
        }
        if state.gap {
            sink.dropped.fetch_add(1, Relaxed);
            sink.step(
                "observation_gap",
                "gap",
                "received",
                json!({"reason":"native_capture_capacity_or_output_gap"}),
                None,
                true,
            )
            .await;
        }
        if let Some(done) = self.completion.take() {
            let _ = done.send(forwarding_ok && !state.gap);
        }
        if let Some(writer) = self.writer.take() {
            let _ = writer.await;
        }
    }
}
#[derive(Clone)]
pub(super) struct Observer(Option<Arc<std::sync::Mutex<Aggregate>>>);
impl Observer {
    pub(super) fn observe(&self, event: &ProviderStreamEvent) {
        if let Some(aggregate) = &self.0 {
            if let Ok(mut state) = aggregate.lock() {
                state.observe(event);
            }
        }
    }
}

// Classify the actual operation and this segment's admitted trigger. Client headers,
// empty outputs, and submitted historical tool messages do not establish purpose.
fn invocation_purpose(
    input: &ProviderInvocationInput,
    context: Option<&control_plane_contracts::ports::WorkflowObservationContext>,
) -> &'static str {
    use plugin_framework::provider_contract::ProviderWireOperation;
    match input.operation {
        ProviderWireOperation::Compact => "compact",
        ProviderWireOperation::CountTokens => "unknown",
        ProviderWireOperation::Generate => {
            if super::is_responses_prewarm(input) {
                "prewarm"
            } else if context.is_some_and(|context| context.is_resume) {
                "tool_resume"
            } else {
                "generate"
            }
        }
    }
}

pub(super) async fn start<
    R: crate::ports::OrchestrationRuntimeRepository + Clone + Send + Sync + 'static,
>(
    repository: R,
    run: Option<Uuid>,
    node: Option<(String, Uuid)>,
    input: &ProviderInvocationInput,
    observation_context: Option<&control_plane_contracts::ports::WorkflowObservationContext>,
) -> Capture {
    let (Some(run), Some((node, node_run))) = (run, node) else {
        return Capture::default();
    };
    let Ok(permit) = WRITERS.acquire().await else {
        return Capture::default();
    };
    let id = Identity {
        observation_context: observation_context.cloned(),
        purpose: invocation_purpose(input, observation_context),
        duration_ms: Arc::new(AtomicU64::new(u64::MAX)),
        run,
        node,
        node_run,
        invocation: input
            .trace_context
            .get("provider_invocation_id")
            .and_then(|id| Uuid::parse_str(id).ok())
            .unwrap_or_else(Uuid::now_v7),
        attempt: input
            .trace_context
            .get("provider_attempt_index")
            .and_then(|value| value.parse().ok())
            .unwrap_or(0),
    };
    let (sender, receiver) = mpsc::channel(RECORDS);
    let (done, completion) = tokio::sync::oneshot::channel();
    let observed = Arc::new(AtomicU64::new(0));
    let dropped = Arc::new(AtomicU64::new(0));
    let sink = Sink {
        id: id.clone(),
        sender,
        observed: observed.clone(),
        dropped: dropped.clone(),
    };
    let writer = tokio::spawn(async move {
        let _permit = permit;
        write(
            id,
            receiver,
            observed,
            dropped,
            completion,
            move |payload| {
                let repository = repository.clone();
                async move {
                    runtime_event_persister::persist_runtime_event_payload(
                        &repository,
                        run,
                        &payload,
                    )
                    .await
                    .is_ok()
                }
            },
        )
        .await;
    });
    let gap = record_input(&sink, input).await;
    Capture {
        sink: Some(sink),
        aggregate: Arc::new(std::sync::Mutex::new(Aggregate {
            gap,
            ..Default::default()
        })),
        completion: Some(done),
        started_at: Some(std::time::Instant::now()),
        writer: Some(writer),
    }
}
async fn write<F, Fut>(
    id: Identity,
    mut receiver: mpsc::Receiver<Record>,
    observed: Arc<AtomicU64>,
    dropped: Arc<AtomicU64>,
    mut completion: tokio::sync::oneshot::Receiver<bool>,
    mut writer: F,
) where
    F: FnMut(crate::ports::RuntimeEventPayload) -> Fut,
    Fut: std::future::Future<Output = bool>,
{
    let integrity = |status: &str, failed: u64| {
        id.event("native_trajectory_integrity", json!({"status":status,"observed_count":observed.load(Relaxed),"persist_failed_count":failed,"dropped_count":dropped.load(Relaxed)}))
    };
    let mut failed = 0;
    // Repository operations own their resource/error policy. A local deadline
    // must not cancel an already-admitted immutable snapshot during a slow write.
    if !writer(integrity("pending", 0)).await {
        failed += 1;
    }
    let mut finished = None;
    loop {
        let record = tokio::select! {
            done = &mut completion, if finished.is_none() => { finished = Some(done.unwrap_or(false)); receiver.close(); continue; }
            record = receiver.recv() => record,
        };
        let Some(record) = record else {
            if finished.is_none() {
                finished = Some(completion.await.unwrap_or(false));
            }
            break;
        };
        if !writer(record.payload).await {
            failed += 1;
        }
    }
    let status = if finished == Some(true) && failed == 0 && dropped.load(Relaxed) == 0 {
        "complete"
    } else {
        "incomplete"
    };
    if !writer(integrity(status, failed)).await {
        tracing::warn!(run=%id.run, "Native trajectory integrity persistence failed");
    }
}

#[cfg(test)]
#[path = "_tests/native_trajectory.rs"]
mod tests;
