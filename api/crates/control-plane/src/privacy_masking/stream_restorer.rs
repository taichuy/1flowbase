//! LIFO multi-stage placeholder restoration for streaming text, reasoning, tool calls, and JSON items.
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use extension_contracts::provider_contract::{
    ProviderInvocationResult, ProviderMcpCall, ProviderStreamEvent, ProviderToolCall,
};
use serde_json::Value;

use super::engine::placeholder_regex;

/// Maximum possible byte length of `{{<24-char KIND>_<8-char tag>}}` = 2 + 24 + 1 + 8 + 2 = 37.
const MAX_PLACEHOLDER_BYTES: usize = 40;

#[derive(Debug, Clone)]
pub struct RestorationStage {
    pub stage_index: usize,
    pub map: HashMap<String, String>,
}

/// Thread-safe LIFO restoration stack with per-stage restoration counters.
#[derive(Debug, Clone, Default)]
pub struct RestorationStack {
    /// Ordered from first-applied stage (index 0) to last-applied stage; restored in reverse (LIFO).
    stages: Vec<RestorationStage>,
    /// Optional fallback map from in-memory session vault for `previous_response_id` continuations.
    session_fallback: HashMap<String, String>,
    /// Per-stage restored counts: stage_index -> placeholder -> count.
    restored_counts: Arc<Mutex<HashMap<usize, HashMap<String, u32>>>>,
}

impl RestorationStack {
    pub fn new(
        stages: Vec<RestorationStage>,
        session_fallback: HashMap<String, String>,
    ) -> Self {
        Self {
            stages,
            session_fallback,
            restored_counts: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.stages.iter().all(|s| s.map.is_empty()) && self.session_fallback.is_empty()
    }

    /// Creates a clone with an isolated counter table so secondary restore passes do not double-count.
    pub fn clone_without_shared_counters(&self) -> Self {
        Self {
            stages: self.stages.clone(),
            session_fallback: self.session_fallback.clone(),
            restored_counts: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Merges all stage maps (plus session fallback) into one flat placeholder->plaintext map for memory-only continuation vault.
    pub fn merged_restore_map(&self) -> HashMap<String, String> {
        let mut merged = self.session_fallback.clone();
        for stage in &self.stages {
            for (k, v) in &stage.map {
                merged.insert(k.clone(), v.clone());
            }
        }
        merged
    }

    /// Restores all placeholders in a complete string (LIFO across stages, then session fallback).
    pub fn restore_str(&self, input: &str) -> String {
        if self.is_empty() || !input.contains("{{") {
            return input.to_string();
        }
        let mut current = input.to_string();
        // LIFO across stages: handles hypothetical double-masking losslessly.
        for stage in self.stages.iter().rev() {
            if stage.map.is_empty() || !current.contains("{{") {
                continue;
            }
            let (next, hits) = replace_placeholders_once(&current, &stage.map);
            if !hits.is_empty() {
                self.record_hits(stage.stage_index, hits);
            }
            current = next;
        }
        if !self.session_fallback.is_empty() && current.contains("{{") {
            let (next, _) = replace_placeholders_once(&current, &self.session_fallback);
            current = next;
        }
        current
    }

    /// Recursively restores all string leaf values in a JSON Value.
    pub fn restore_json(&self, value: &mut Value) {
        if self.is_empty() {
            return;
        }
        match value {
            Value::String(text) => {
                if text.contains("{{") {
                    *text = self.restore_str(text);
                }
            }
            Value::Array(items) => {
                for item in items {
                    self.restore_json(item);
                }
            }
            Value::Object(map) => {
                for item in map.values_mut() {
                    self.restore_json(item);
                }
            }
            Value::Null | Value::Bool(_) | Value::Number(_) => {}
        }
    }

    pub fn restore_tool_call(&self, call: &mut ProviderToolCall) {
        self.restore_json(&mut call.arguments);
    }

    pub fn restore_mcp_call(&self, call: &mut ProviderMcpCall) {
        self.restore_json(&mut call.arguments);
    }

    pub fn restore_invocation_result(&self, result: &mut ProviderInvocationResult) {
        if self.is_empty() {
            return;
        }
        if let Some(content) = result.final_content.as_mut() {
            if content.contains("{{") {
                *content = self.restore_str(content);
            }
        }
        for call in &mut result.tool_calls {
            self.restore_tool_call(call);
        }
        for call in &mut result.mcp_calls {
            self.restore_mcp_call(call);
        }
    }

    pub fn restored_counts_for_stage(&self, stage_index: usize) -> HashMap<String, u32> {
        self.restored_counts
            .lock()
            .ok()
            .and_then(|guard| guard.get(&stage_index).cloned())
            .unwrap_or_default()
    }

    fn record_hits(&self, stage_index: usize, hits: HashMap<String, u32>) {
        if let Ok(mut guard) = self.restored_counts.lock() {
            let entry = guard.entry(stage_index).or_default();
            for (placeholder, count) in hits {
                *entry.entry(placeholder).or_insert(0) += count;
            }
        }
    }
}

fn replace_placeholders_once(
    text: &str,
    map: &HashMap<String, String>,
) -> (String, HashMap<String, u32>) {
    let re = placeholder_regex();
    let mut out = String::with_capacity(text.len());
    let mut hits = HashMap::new();
    let mut last = 0usize;
    let mut changed = false;

    for m in re.find_iter(text) {
        if let Some(original) = map.get(m.as_str()) {
            out.push_str(&text[last..m.start()]);
            out.push_str(original);
            *hits.entry(m.as_str().to_string()).or_insert(0) += 1;
            last = m.end();
            changed = true;
        }
    }
    if !changed {
        return (text.to_string(), hits);
    }
    out.push_str(&text[last..]);
    (out, hits)
}

/// Chunk-boundary-safe streaming text restorer for a single text/reasoning/argument delta channel.
pub struct StreamingTextRestorer {
    stack: RestorationStack,
    pending: String,
}

impl StreamingTextRestorer {
    pub fn new(stack: RestorationStack) -> Self {
        Self {
            stack,
            pending: String::new(),
        }
    }

    /// Push a streaming delta chunk and return the safe-to-emit restored prefix.
    pub fn push(&mut self, delta: &str) -> String {
        if self.stack.is_empty() {
            return delta.to_string();
        }
        self.pending.push_str(delta);
        if self.pending.is_empty() {
            return String::new();
        }
        let hold_start = trailing_placeholder_prefix_start(&self.pending);
        let ready = self.pending[..hold_start].to_string();
        let remaining = self.pending[hold_start..].to_string();
        self.pending = remaining;
        if ready.is_empty() {
            String::new()
        } else {
            self.stack.restore_str(&ready)
        }
    }

    /// Flush any held trailing fragment on stream terminal or boundary.
    pub fn flush(&mut self) -> String {
        if self.pending.is_empty() {
            return String::new();
        }
        let tail = std::mem::take(&mut self.pending);
        self.stack.restore_str(&tail)
    }
}

/// Finds the earliest byte offset near the tail of `s` that could be the start of an incomplete
/// `{{KIND_xxxxxxxx}}` placeholder split across streaming chunks.
fn trailing_placeholder_prefix_start(s: &str) -> usize {
    let min_start = s.len().saturating_sub(MAX_PLACEHOLDER_BYTES - 1);
    let bytes = s.as_bytes();
    for idx in min_start..bytes.len() {
        if bytes[idx] != b'{' {
            continue;
        }
        let tail = &s[idx..];
        if is_valid_incomplete_placeholder_prefix(tail) {
            return idx;
        }
    }
    s.len()
}

fn is_valid_incomplete_placeholder_prefix(tail: &str) -> bool {
    if tail.is_empty() || tail.len() >= MAX_PLACEHOLDER_BYTES {
        return false;
    }
    if tail == "{" {
        return true;
    }
    if !tail.starts_with("{{") {
        return false;
    }
    // If already closed with `}}`, it is complete and should be emitted immediately.
    if tail.contains("}}") {
        return false;
    }
    let body = &tail[2..];
    // Allow trailing single `}` at the end (e.g. `{{API_KEY_k3v9x2mq}` waiting for second `}`).
    let body = body.strip_suffix('}').unwrap_or(body);
    if body.is_empty() {
        return true;
    }
    let first = body.as_bytes()[0];
    if !first.is_ascii_uppercase() {
        return false;
    }
    body.bytes()
        .all(|b| b.is_ascii_uppercase() || b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}

/// Stateful per-stream restorer covering all `ProviderStreamEvent` channels.
pub struct ProviderStreamEventRestorer {
    stack: RestorationStack,
    text: StreamingTextRestorer,
    reasoning: StreamingTextRestorer,
    tool_deltas: HashMap<String, StreamingTextRestorer>,
    mcp_deltas: HashMap<String, StreamingTextRestorer>,
}

impl ProviderStreamEventRestorer {
    pub fn new(stack: RestorationStack) -> Self {
        Self {
            text: StreamingTextRestorer::new(stack.clone()),
            reasoning: StreamingTextRestorer::new(stack.clone()),
            tool_deltas: HashMap::new(),
            mcp_deltas: HashMap::new(),
            stack,
        }
    }

    pub fn is_active(&self) -> bool {
        !self.stack.is_empty()
    }

    /// Processes one upstream `ProviderStreamEvent` into 0..N restored downstream events.
    pub fn process_event(&mut self, event: ProviderStreamEvent) -> Vec<ProviderStreamEvent> {
        if self.stack.is_empty() {
            return vec![event];
        }
        match event {
            ProviderStreamEvent::TextDelta { delta } => {
                let restored = self.text.push(&delta);
                if restored.is_empty() {
                    Vec::new()
                } else {
                    vec![ProviderStreamEvent::TextDelta { delta: restored }]
                }
            }
            ProviderStreamEvent::ReasoningDelta { delta } => {
                let restored = self.reasoning.push(&delta);
                if restored.is_empty() {
                    Vec::new()
                } else {
                    vec![ProviderStreamEvent::ReasoningDelta { delta: restored }]
                }
            }
            ProviderStreamEvent::ToolCallDelta { call_id, mut delta } => {
                if let Value::String(fragment) = &delta {
                    let restorer = self
                        .tool_deltas
                        .entry(call_id.clone())
                        .or_insert_with(|| StreamingTextRestorer::new(self.stack.clone()));
                    let restored = restorer.push(fragment);
                    if restored.is_empty() {
                        return Vec::new();
                    }
                    delta = Value::String(restored);
                } else {
                    self.stack.restore_json(&mut delta);
                }
                vec![ProviderStreamEvent::ToolCallDelta { call_id, delta }]
            }
            ProviderStreamEvent::ToolCallCommit { mut call } => {
                let mut out = Vec::new();
                if let Some(mut restorer) = self.tool_deltas.remove(&call.id) {
                    let flushed = restorer.flush();
                    if !flushed.is_empty() {
                        out.push(ProviderStreamEvent::ToolCallDelta {
                            call_id: call.id.clone(),
                            delta: Value::String(flushed),
                        });
                    }
                }
                self.stack.restore_tool_call(&mut call);
                out.push(ProviderStreamEvent::ToolCallCommit { call });
                out
            }
            ProviderStreamEvent::McpCallDelta { call_id, mut delta } => {
                if let Value::String(fragment) = &delta {
                    let restorer = self
                        .mcp_deltas
                        .entry(call_id.clone())
                        .or_insert_with(|| StreamingTextRestorer::new(self.stack.clone()));
                    let restored = restorer.push(fragment);
                    if restored.is_empty() {
                        return Vec::new();
                    }
                    delta = Value::String(restored);
                } else {
                    self.stack.restore_json(&mut delta);
                }
                vec![ProviderStreamEvent::McpCallDelta { call_id, delta }]
            }
            ProviderStreamEvent::McpCallCommit { mut call } => {
                let mut out = Vec::new();
                if let Some(mut restorer) = self.mcp_deltas.remove(&call.id) {
                    let flushed = restorer.flush();
                    if !flushed.is_empty() {
                        out.push(ProviderStreamEvent::McpCallDelta {
                            call_id: call.id.clone(),
                            delta: Value::String(flushed),
                        });
                    }
                }
                self.stack.restore_mcp_call(&mut call);
                out.push(ProviderStreamEvent::McpCallCommit { call });
                out
            }
            ProviderStreamEvent::OutputItem {
                phase,
                output_index,
                mut item,
            } => {
                self.stack.restore_json(&mut item);
                vec![ProviderStreamEvent::OutputItem {
                    phase,
                    output_index,
                    item,
                }]
            }
            ProviderStreamEvent::ResponsesOutputDelta { mut event } => {
                self.stack.restore_json(&mut event);
                vec![ProviderStreamEvent::ResponsesOutputDelta { event }]
            }
            ProviderStreamEvent::NativeEvent {
                protocol,
                mut event,
            } => {
                self.stack.restore_json(&mut event);
                vec![ProviderStreamEvent::NativeEvent { protocol, event }]
            }
            terminal @ (ProviderStreamEvent::Finish { .. }
            | ProviderStreamEvent::Error { .. }
            | ProviderStreamEvent::OutputProtocolFailure { .. }) => {
                let mut out = self.flush_all();
                out.push(terminal);
                out
            }
            other => vec![other],
        }
    }

    /// Flush all buffered channels before stream exit.
    pub fn flush_all(&mut self) -> Vec<ProviderStreamEvent> {
        let mut out = Vec::new();
        let reasoning_tail = self.reasoning.flush();
        if !reasoning_tail.is_empty() {
            out.push(ProviderStreamEvent::ReasoningDelta {
                delta: reasoning_tail,
            });
        }
        let text_tail = self.text.flush();
        if !text_tail.is_empty() {
            out.push(ProviderStreamEvent::TextDelta { delta: text_tail });
        }
        for (call_id, mut restorer) in self.tool_deltas.drain() {
            let tail = restorer.flush();
            if !tail.is_empty() {
                out.push(ProviderStreamEvent::ToolCallDelta {
                    call_id,
                    delta: Value::String(tail),
                });
            }
        }
        for (call_id, mut restorer) in self.mcp_deltas.drain() {
            let tail = restorer.flush();
            if !tail.is_empty() {
                out.push(ProviderStreamEvent::McpCallDelta {
                    call_id,
                    delta: Value::String(tail),
                });
            }
        }
        out
    }

    pub fn stack(&self) -> &RestorationStack {
        &self.stack
    }
}
