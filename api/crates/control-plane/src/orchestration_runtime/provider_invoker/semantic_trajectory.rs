//! Write-side, bounded protocol projection. Network chunks are evidence, never steps.
use crate::ports::RuntimeEventPayload;
use base64::Engine;
use serde_json::{json, Value};
use std::collections::BTreeMap;

const MAX_BUFFER: usize = 2 * 1024 * 1024;
const MAX_STEPS: usize = 128;
const MAX_PREVIEW: usize = 4096;
const MAX_ATTEMPTS: usize = 8;

#[derive(Default)]
pub(super) struct SemanticProjector {
    attempts: BTreeMap<(String, u64), Attempt>,
}
#[derive(Default)]
struct Attempt {
    buffer: Vec<u8>,
    steps: BTreeMap<String, Value>,
    first: u64,
    last: u64,
    exchange: u64,
    broken: bool,
    reason: Option<&'static str>,
    protocol: String,
    transport: String,
}

impl SemanticProjector {
    pub(super) fn observe(&mut self, event: &RuntimeEventPayload) -> Vec<RuntimeEventPayload> {
        let p = &event.payload;
        let Some(invocation) = p.get("invocation_id").and_then(Value::as_str) else {
            return vec![];
        };
        let key = (
            invocation.to_owned(),
            p["provider_attempt_index"].as_u64().unwrap_or(0),
        );
        if event.event_type == "provider_protocol_integrity" {
            let Some(mut attempt) = self.attempts.remove(&key) else {
                return vec![];
            };
            if p["status"] != "complete" {
                attempt.fail("capture_incomplete");
            }
            return attempt.finish(event);
        }
        if event.event_type != "provider_protocol_observation" {
            return vec![];
        }
        if !self.attempts.contains_key(&key) && self.attempts.len() >= MAX_ATTEMPTS {
            return vec![output(
                event,
                json!({"step_key":"projection_limit", "kind":"observation_gap", "status":"incomplete", "reason":"attempt_limit", "raw_sequence_start":p["sequence"], "raw_sequence_end":p["sequence"]}),
            )];
        }
        let attempt = self.attempts.entry(key).or_default();
        attempt.protocol = p["protocol"].as_str().unwrap_or("").to_owned();
        attempt.transport = p["transport"].as_str().unwrap_or("").to_owned();
        let seq = p["sequence"].as_u64().unwrap_or(0);
        if attempt.last != 0 && seq != attempt.last + 1 {
            attempt.fail("missing_observation");
        }
        attempt.last = seq;
        if attempt.first == 0 {
            attempt.first = seq;
        }
        let kind = p["kind"].as_str().unwrap_or("");
        if kind == "capture_integrity" {
            if let Ok(value) = serde_json::from_str::<Value>(p["body"].as_str().unwrap_or("")) {
                if value["status"] != "complete" || value["dropped_count"].as_u64().unwrap_or(0) > 0
                {
                    attempt.fail("capture_incomplete");
                }
            } else {
                attempt.fail("invalid_integrity");
            }
            return vec![];
        }
        if kind == "stream_end" {
            return attempt.finish(event);
        }
        if kind == "response_head" {
            if p["status"].as_u64().is_some_and(|s| s >= 400) {
                attempt.step("error", "error", "", "", seq);
            }
            return vec![];
        }
        if !matches!(
            kind,
            "request_prepared" | "request" | "response_body" | "message"
        ) {
            return vec![];
        }
        if attempt.broken {
            return vec![];
        }
        let bytes = match p["encoding"].as_str() {
            Some("base64") => {
                base64::engine::general_purpose::STANDARD.decode(p["body"].as_str().unwrap_or(""))
            }
            _ => Ok(p["body"].as_str().unwrap_or("").as_bytes().to_vec()),
        };
        let Ok(bytes) = bytes else {
            attempt.fail("invalid_encoding");
            return vec![];
        };
        if matches!(kind, "request_prepared" | "request") {
            let mut finished = attempt.finish(event);
            attempt.steps.clear();
            attempt.exchange += 1;
            attempt.first = seq;
            attempt.step("call", "model_call", "", "", seq);
            if bytes.len() <= MAX_BUFFER {
                if let Ok(request) = serde_json::from_slice::<Value>(&bytes) {
                    attempt.request_results(&request, seq);
                }
            } else {
                attempt.fail("byte_limit");
            }
            finished.extend(attempt.snapshot(event));
            return finished;
        }
        if p["transport"] == "sse" {
            // Enforce a per-event budget, independent of how the network chunks events.
            for byte in bytes {
                attempt.buffer.push(byte);
                if attempt.buffer.len() > MAX_BUFFER {
                    attempt.fail("byte_limit");
                    break;
                }
                if !attempt.buffer.ends_with(b"\n\n") && !attempt.buffer.ends_with(b"\r\n\r\n") {
                    continue;
                }
                let frame = std::mem::take(&mut attempt.buffer);
                let Ok(frame) = std::str::from_utf8(&frame) else {
                    attempt.fail("invalid_utf8");
                    break;
                };
                let data = frame
                    .lines()
                    .filter_map(|line| {
                        line.strip_prefix("data:")
                            .map(|s| s.strip_prefix(' ').unwrap_or(s))
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                if data.is_empty() || data == "[DONE]" {
                    continue;
                }
                match serde_json::from_str::<Value>(&data) {
                    Ok(value) => attempt.response(&value, seq),
                    Err(_) => {
                        attempt.fail("invalid_protocol_event");
                        break;
                    }
                }
            }
        } else {
            if attempt.buffer.len().saturating_add(bytes.len()) > MAX_BUFFER {
                attempt.fail("byte_limit");
                return vec![];
            }
            attempt.buffer.extend(bytes);
            match serde_json::from_slice::<Value>(&attempt.buffer) {
                Ok(value) => {
                    attempt.buffer.clear();
                    attempt.response(&value, seq);
                }
                Err(error) if error.is_eof() => {}
                Err(_) => attempt.fail("invalid_protocol_body"),
            }
        }
        vec![]
    }
}

fn output(input: &RuntimeEventPayload, mut metadata: Value) -> RuntimeEventPayload {
    for field in [
        "flow_run_id",
        "node_id",
        "node_run_id",
        "invocation_id",
        "provider_attempt_index",
        "protocol",
        "transport",
    ] {
        if matches!(field, "protocol" | "transport") && metadata.get(field).is_some() {
            continue;
        }
        if let Some(value) = input.payload.get(field) {
            metadata[field] = value.clone();
        }
    }
    metadata["type"] = json!("provider_semantic_step");
    RuntimeEventPayload {
        event_type: "provider_semantic_step".into(),
        source: input.source,
        durability: input.durability,
        persist_required: true,
        trace_visible: false,
        payload: metadata,
    }
}
impl Attempt {
    fn fail(&mut self, reason: &'static str) {
        self.buffer.clear();
        self.broken = true;
        self.reason = Some(reason);
    }
    fn step(&mut self, id: &str, kind: &str, text: &str, tool_id: &str, seq: u64) {
        let key = format!("{}:{kind}:{id}", self.exchange);
        if !self.steps.contains_key(&key) && self.steps.len() >= MAX_STEPS {
            self.fail("step_limit");
            return;
        }
        let step = self.steps.entry(key.clone()).or_insert_with(|| json!({
            "step_key":key, "kind":kind, "preview":"", "tool_call_id":tool_id.chars().take(512).collect::<String>(), "protocol":self.protocol, "transport":self.transport,
            "raw_sequence_start":self.first, "raw_sequence_end":seq,
            "status":"recorded", "direction":if kind == "model_call" || kind == "tool_result" { "prepared" } else { "received" },
            "provenance":if kind == "tool_result" { "submitted_tool_result" } else { "supplier_protocol" }
        }));
        if !tool_id.is_empty() {
            step["tool_call_id"] = json!(tool_id.chars().take(512).collect::<String>());
        }
        let mut preview = step["preview"].as_str().unwrap_or("").to_owned();
        for ch in text.chars() {
            if preview.len() + ch.len_utf8() > MAX_PREVIEW {
                break;
            }
            preview.push(ch);
        }
        step["preview"] = json!(preview);
        step["raw_sequence_end"] = json!(seq);
    }
    fn request_results(&mut self, value: &Value, seq: u64) {
        for field in ["messages", "input", "contents"] {
            if let Some(items) = value[field].as_array() {
                for (index, item) in items.iter().enumerate() {
                    if item["role"] == "tool" || item["type"] == "function_call_output" {
                        let id = item["tool_call_id"]
                            .as_str()
                            .or_else(|| item["call_id"].as_str())
                            .unwrap_or("");
                        self.step(&index.to_string(), "tool_result", "", id, seq);
                    }
                    for content_field in ["content", "parts"] {
                        if let Some(parts) = item[content_field].as_array() {
                            for (part_index, part) in parts.iter().enumerate() {
                                if part["type"] == "tool_result"
                                    || part.get("functionResponse").is_some()
                                {
                                    let id = part["tool_use_id"]
                                        .as_str()
                                        .or_else(|| part["functionResponse"]["id"].as_str())
                                        .unwrap_or("");
                                    self.step(
                                        &format!("{index}:{part_index}"),
                                        "tool_result",
                                        "",
                                        id,
                                        seq,
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    fn response(&mut self, value: &Value, seq: u64) {
        if let Some(values) = value.as_array() {
            for value in values {
                self.response(value, seq);
            }
            return;
        }
        if value.get("error").is_some()
            || value["type"] == "error"
            || value["type"] == "response.failed"
        {
            self.step("error", "error", "", "", seq);
            return;
        }
        // OpenAI Chat Completions and compatible providers (including DeepSeek/Bailian).
        if let Some(choices) = value["choices"].as_array() {
            for (index, choice) in choices.iter().enumerate() {
                let id = choice["index"].as_u64().unwrap_or(index as u64).to_string();
                let message = choice
                    .get("delta")
                    .or_else(|| choice.get("message"))
                    .unwrap_or(choice);
                if let Some(text) = message["content"].as_str() {
                    self.step(&id, "model_reply", text, "", seq);
                }
                if let Some(calls) = message["tool_calls"].as_array() {
                    for (index, call) in calls.iter().enumerate() {
                        let call_index = call["index"].as_u64().unwrap_or(index as u64);
                        self.step(
                            &format!("{id}:{call_index}"),
                            "tool_call",
                            call["function"]["arguments"].as_str().unwrap_or(""),
                            call["id"].as_str().unwrap_or(""),
                            seq,
                        );
                    }
                }
            }
        }
        let ty = value["type"].as_str().unwrap_or("");
        // Responses: delta indices remain stable when tool ids arrive in a later event.
        if ty.starts_with("response.") {
            let index = value["output_index"].as_u64().unwrap_or(0).to_string();
            match ty {
                "response.output_text.delta" => self.step(
                    &index,
                    "model_reply",
                    value["delta"].as_str().unwrap_or(""),
                    "",
                    seq,
                ),
                "response.function_call_arguments.delta" => self.step(
                    &index,
                    "tool_call",
                    value["delta"].as_str().unwrap_or(""),
                    "",
                    seq,
                ),
                "response.output_item.added" => {
                    let item = &value["item"];
                    if item["type"] == "function_call" {
                        self.step(
                            &index,
                            "tool_call",
                            "",
                            item["call_id"].as_str().unwrap_or(""),
                            seq,
                        );
                    }
                }
                "response.completed" => {
                    if self
                        .steps
                        .values()
                        .all(|s| s["kind"] != "model_reply" && s["kind"] != "tool_call")
                    {
                        self.response(&value["response"], seq);
                    }
                }
                _ => {}
            }
        } else if let Some(items) = value["output"].as_array() {
            for (index, item) in items.iter().enumerate() {
                self.output_item(item, &index.to_string(), seq);
            }
        }
        // Anthropic Messages (native JSON and SSE).
        if ty == "content_block_start" {
            self.output_item(&value["content_block"], &value["index"].to_string(), seq);
        }
        if ty == "content_block_delta" {
            let delta = &value["delta"];
            let id = value["index"].to_string();
            if let Some(text) = delta["text"].as_str() {
                self.step(&id, "model_reply", text, "", seq);
            }
            if let Some(text) = delta["partial_json"].as_str() {
                self.step(&id, "tool_call", text, "", seq);
            }
        }
        if let Some(content) = value["content"].as_array() {
            for (index, item) in content.iter().enumerate() {
                self.output_item(item, &index.to_string(), seq);
            }
        }
        // Gemini GenerateContent; tool requests are distinct from submitted functionResponse.
        if let Some(candidates) = value["candidates"].as_array() {
            for (candidate_index, candidate) in candidates.iter().enumerate() {
                if let Some(parts) = candidate["content"]["parts"].as_array() {
                    for (index, part) in parts.iter().enumerate() {
                        let id = format!("{candidate_index}:{index}");
                        if let Some(text) = part["text"].as_str() {
                            self.step(&candidate_index.to_string(), "model_reply", text, "", seq);
                        }
                        if let Some(call) = part.get("functionCall") {
                            self.step(
                                &id,
                                "tool_call",
                                &call["args"].to_string(),
                                call["id"].as_str().unwrap_or(""),
                                seq,
                            );
                        }
                    }
                }
            }
        }
    }
    fn output_item(&mut self, item: &Value, id: &str, seq: u64) {
        match item["type"].as_str().unwrap_or("") {
            "function_call" | "tool_use" => self.step(
                id,
                "tool_call",
                item["arguments"].as_str().unwrap_or(""),
                item["call_id"]
                    .as_str()
                    .or_else(|| item["id"].as_str())
                    .unwrap_or(""),
                seq,
            ),
            "text" | "output_text" => self.step(
                id,
                "model_reply",
                item["text"].as_str().unwrap_or(""),
                "",
                seq,
            ),
            "message" => {
                if let Some(parts) = item["content"].as_array() {
                    for part in parts {
                        self.output_item(part, id, seq);
                    }
                }
            }
            _ => {}
        }
    }
    fn snapshot(&self, event: &RuntimeEventPayload) -> Vec<RuntimeEventPayload> {
        self.steps
            .values()
            .map(|step| {
                let mut step = step.clone();
                if step.get("protocol").is_none() {
                    step["protocol"] = json!(self.protocol);
                }
                if step.get("transport").is_none() {
                    step["transport"] = json!(self.transport);
                }
                output(event, step)
            })
            .collect()
    }
    fn finish(&mut self, event: &RuntimeEventPayload) -> Vec<RuntimeEventPayload> {
        if !self.buffer.iter().all(u8::is_ascii_whitespace) {
            self.fail("unfinished_protocol_event");
        }
        self.buffer.clear();
        if self.broken {
            let reason = self.reason.unwrap_or("capture_incomplete");
            for step in self.steps.values_mut() {
                step["status"] = json!("incomplete");
            }
            self.steps.insert(format!("{}:gap", self.exchange), json!({"step_key":format!("{}:gap",self.exchange), "kind":"observation_gap", "status":"incomplete", "reason":reason, "raw_sequence_start":self.first, "raw_sequence_end":self.last}));
        } else if self
            .steps
            .values()
            .all(|s| s["kind"] == "model_call" || s["kind"] == "tool_result")
            && self.last > self.first
        {
            self.steps.insert(format!("{}:unknown",self.exchange), json!({"step_key":format!("{}:unknown",self.exchange),"kind":"observation_gap","status":"unavailable","reason":"no_supported_semantic_response","raw_sequence_start":self.first,"raw_sequence_end":self.last}));
        }
        self.snapshot(event)
    }
}

#[cfg(test)]
#[path = "_tests/semantic_trajectory.rs"]
mod tests;
