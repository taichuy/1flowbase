//! Observation at the Native execution boundary, not protocol translation or billing.
//! Input is the actual value handed to the plugin; output is what the plugin returned.
//! The repository owns storage/query. Child steps reference immutable invocation-local
//! snapshots; observation failure never changes execution or client protocol facts.
use super::*;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};

const CAPACITY: usize = 1024 * 1024;
const RECORDS: usize = 128;
const TIMEOUT: std::time::Duration = std::time::Duration::from_secs(2);
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

// Serialize into a capped buffer before cloning any provider-owned structured value.
struct Limited(Vec<u8>);
impl std::io::Write for Limited {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if self.0.len().saturating_add(bytes.len()) > CAPACITY / 4 {
            return Err(std::io::Error::other("native detail capacity"));
        }
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
fn bounded<T: serde::Serialize + ?Sized>(value: &T) -> Option<Value> {
    let mut buffer = Limited(Vec::new());
    serde_json::to_writer(&mut buffer, value).ok()?;
    serde_json::from_slice(&buffer.0).ok()
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
    bounded(&SafeInput {
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
fn record_input(sink: &Sink, input: &ProviderInvocationInput) -> bool {
    let detail = safe_input(input);
    let gap = detail.is_none();
    let snapshot_key = Uuid::now_v7().to_string();
    let saved = sink.step(
        "model_call",
        &snapshot_key,
        "prepared",
        detail.unwrap_or_else(|| json!({"truncated":true})),
        None,
        gap,
    );
    let mut submitted = BTreeSet::new();
    for (index, message) in input
        .messages
        .iter()
        .enumerate()
        .filter(|(_, message)| message.role == ProviderMessageRole::Tool)
    {
        if let Some(detail) = bounded(message) {
            sink.referenced_step(
                "tool_result",
                &format!("submitted:{index}"),
                "prepared",
                detail,
                message.tool_call_id.as_deref(),
                (saved && !gap).then(|| (snapshot_key.as_str(), format!("/messages/{index}"))),
            );
            if let Some(id) = &message.tool_call_id {
                if submitted.len() < RECORDS {
                    submitted.insert(id.clone());
                }
            }
        } else {
            sink.dropped.fetch_add(1, Relaxed);
        }
    }
    // Responses is an explicit Native passthrough contract here, not supplier raw
    // evidence. Only its declared function_call_output input is a submitted result.
    if let Some(native) = input.native_transport.as_ref() {
        for (index, item) in native
            .wire_body
            .get("input")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .enumerate()
        {
            if item["type"] != "function_call_output" {
                continue;
            }
            let id = item["call_id"].as_str();
            if id.is_some_and(|id| submitted.contains(id)) {
                continue;
            }
            if let Some(detail) = bounded(item) {
                sink.referenced_step(
                    "tool_result",
                    &format!("native-submitted:{index}"),
                    "prepared",
                    detail,
                    id,
                    (saved && !gap).then(|| {
                        (
                            snapshot_key.as_str(),
                            format!("/native_request/wire_body/input/{index}"),
                        )
                    }),
                );
            } else {
                sink.dropped.fetch_add(1, Relaxed);
            }
        }
    }
    gap
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
    retained: usize,
    gap: bool,
}
impl Aggregate {
    fn observe(&mut self, event: &ProviderStreamEvent) {
        // Chunks are accumulated, never emitted as semantic steps.
        match event {
            ProviderStreamEvent::TextDelta { delta }
            | ProviderStreamEvent::ReasoningDelta { delta } => {
                if self.retained.saturating_add(delta.len()) > CAPACITY / 2 {
                    self.gap = true;
                    return;
                }
                self.retained += delta.len();
                if matches!(event, ProviderStreamEvent::TextDelta { .. }) {
                    self.text.push_str(delta);
                } else {
                    self.reasoning.push_str(delta);
                }
            }
            ProviderStreamEvent::ReasoningSignatureDelta { signature } => {
                if self.retained.saturating_add(signature.len()) > CAPACITY / 2 {
                    self.gap = true;
                    return;
                }
                self.retained += signature.len();
                self.signature.push_str(signature);
            }
            ProviderStreamEvent::OutputItem {
                phase,
                output_index,
                item,
            } => {
                if *phase == ProviderOutputItemPhase::Added {
                    if self.open_items.len() >= RECORDS {
                        self.gap = true;
                    } else {
                        self.open_items.insert(*output_index);
                    }
                    return;
                }
                self.open_items.remove(output_index);
                if let Some(value) = self.admit(item) {
                    self.items.insert(*output_index, value);
                }
            }
            ProviderStreamEvent::ResponsesOutputDelta { event } => {
                if let Some(index) = event["output_index"].as_u64() {
                    // A Responses delta belongs to an Added, not-yet-Done item.
                    // It cannot open a new item or reopen one already committed.
                    if !self.open_items.contains(&(index as usize)) {
                        self.gap = true;
                    }
                } else {
                    self.gap = true;
                }
            }
            ProviderStreamEvent::ToolCallDelta { call_id, .. }
            | ProviderStreamEvent::McpCallDelta { call_id, .. } => {
                if call_id.len() > 1024 || self.open_tools.len() >= RECORDS {
                    self.gap = true;
                } else {
                    self.open_tools.insert(call_id.clone());
                }
            }
            ProviderStreamEvent::ToolCallCommit { call } => {
                self.open_tools.remove(&call.id);
                if let Some(value) = self.admit(call) {
                    self.tools.insert(call.id.clone(), value);
                }
            }
            ProviderStreamEvent::McpCallCommit { call } => {
                self.open_tools.remove(&call.id);
                if let Some(value) = self.admit(call) {
                    self.tools.insert(call.id.clone(), value);
                }
            }
            ProviderStreamEvent::Error { error } => {
                // Freeform runtime messages can quote transport credentials. Keep the
                // typed failure identity and scrub structured diagnostics separately.
                if let Some(value) = self.admit(&json!({"kind":error.kind,"message":"Native provider error","provider_details":error.provider_details})) {
                    self.errors.push(value);
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
        let Some(mut value) = bounded(value) else {
            self.gap = true;
            return None;
        };
        redact_metadata(&mut value);
        let size = value.to_string().len();
        if self.retained.saturating_add(size) > CAPACITY / 2
            || self.items.len() + self.tools.len() + self.errors.len() >= RECORDS / 2
        {
            self.gap = true;
            return None;
        }
        self.retained += size;
        Some(value)
    }
}
struct Record {
    payload: crate::ports::RuntimeEventPayload,
    _bytes: tokio::sync::OwnedSemaphorePermit,
}
struct Sink {
    id: Identity,
    sender: mpsc::Sender<Record>,
    bytes: Arc<tokio::sync::Semaphore>,
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
    fn step(
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
        self.enqueue(payload)
    }

    fn referenced_step(
        &self,
        kind: &str,
        key: &str,
        direction: &str,
        detail: Value,
        tool: Option<&str>,
        reference: Option<(&str, String)>,
    ) {
        let Some((step_key, pointer)) = reference else {
            self.step(kind, key, direction, detail, tool, false);
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
        ));
    }

    fn enqueue(&self, payload: crate::ports::RuntimeEventPayload) -> bool {
        let size = payload.payload.to_string().len().saturating_add(512);
        let permit = u32::try_from(size)
            .ok()
            .and_then(|size| self.bytes.clone().try_acquire_many_owned(size).ok());
        if let Some(permit) = permit {
            if self
                .sender
                .try_send(Record {
                    payload,
                    _bytes: permit,
                })
                .is_ok()
            {
                self.observed.fetch_add(1, Relaxed);
                return true;
            }
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
    pub(super) fn finish_compact(
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
            let detail = bounded(result);
            complete &= detail.is_some();
            sink.step(
                "model_reply",
                &Uuid::now_v7().to_string(),
                "received",
                detail.unwrap_or_else(|| json!({"truncated":true})),
                None,
                !complete,
            );
        }
        if failed {
            sink.step(
                "error",
                "error:invocation",
                "received",
                json!({"message":"Native compaction failed"}),
                None,
                false,
            );
        }
        if !complete {
            sink.dropped.fetch_add(1, Relaxed);
        }
        if let Some(done) = self.completion.take() {
            let _ = done.send(complete);
        }
    }

    pub(super) fn observer(&self) -> Observer {
        Observer(self.sink.as_ref().map(|_| self.aggregate.clone()))
    }
    pub(super) fn finish(
        mut self,
        result: Option<&plugin_framework::provider_contract::ProviderInvocationResult>,
        error: Option<&str>,
        forwarding_ok: bool,
    ) {
        self.record_duration();
        let Some(sink) = self.sink.as_ref() else {
            return;
        };
        let Ok(mut state) = self.aggregate.lock() else {
            return;
        };
        let result = result.and_then(|result| {
            let mut value = bounded(result);
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
            || !reply_items.is_empty()
        {
            saved_reply = sink.step(
                "model_reply",
                &reply_key,
                "received",
                json!({"final_content":text,"reasoning":state.reasoning,
            "reasoning_signature":state.signature,"output_items":reply_items,"result":result}),
                None,
                state.gap,
            );
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
            );
        }
        for (index, error) in state.errors.iter().enumerate() {
            sink.step(
                "error",
                &format!("error:{index}"),
                "received",
                error.clone(),
                None,
                false,
            );
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
                );
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
            );
        }
        if let Some(done) = self.completion.take() {
            let _ = done.send(forwarding_ok && !state.gap);
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

pub(super) fn start<
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
    let Ok(permit) = WRITERS.try_acquire() else {
        tracing::warn!(%run, "Native trajectory writer capacity exceeded; trajectory unavailable");
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
        bytes: Arc::new(tokio::sync::Semaphore::new(CAPACITY)),
        observed: observed.clone(),
        dropped: dropped.clone(),
    };
    let gap = record_input(&sink, input);
    tokio::spawn(async move {
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
    Capture {
        sink: Some(sink),
        aggregate: Arc::new(std::sync::Mutex::new(Aggregate {
            gap,
            ..Default::default()
        })),
        completion: Some(done),
        started_at: Some(std::time::Instant::now()),
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
    if !matches!(
        tokio::time::timeout(TIMEOUT, writer(integrity("pending", 0))).await,
        Ok(true)
    ) {
        failed += 1;
    }
    let mut finished = None;
    let mut deadline = None;
    loop {
        let record = tokio::select! {
            done = &mut completion, if finished.is_none() => { finished = Some(done.unwrap_or(false)); receiver.close(); deadline = Some(tokio::time::Instant::now() + std::time::Duration::from_secs(5)); continue; }
            _ = async { if let Some(deadline) = deadline { tokio::time::sleep_until(deadline).await } else { std::future::pending::<()>().await } } => { dropped.fetch_add(receiver.len() as u64, Relaxed); finished = Some(false); break; }
            record = receiver.recv() => record,
        };
        let Some(record) = record else {
            if finished.is_none() {
                finished = Some(completion.await.unwrap_or(false));
            }
            break;
        };
        if !matches!(
            tokio::time::timeout(TIMEOUT, writer(record.payload)).await,
            Ok(true)
        ) {
            failed += 1;
        }
    }
    let status = if finished == Some(true) && failed == 0 && dropped.load(Relaxed) == 0 {
        "complete"
    } else {
        "incomplete"
    };
    if !matches!(
        tokio::time::timeout(TIMEOUT, writer(integrity(status, failed))).await,
        Ok(true)
    ) {
        tracing::warn!(run=%id.run, "Native trajectory integrity persistence failed");
    }
}

#[cfg(test)]
#[path = "_tests/native_trajectory.rs"]
mod tests;
