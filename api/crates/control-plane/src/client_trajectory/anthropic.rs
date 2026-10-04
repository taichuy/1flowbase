//! Anthropic wire semantics. Transport capture retains every original frame.
use super::{bounded_id, preview, Classifier, FactSink};
use crate::ports::ClientTrajectoryFact as Fact;
use serde_json::{json, Value};
use uuid::Uuid;

pub(super) struct ContentBlock {
    value: Value,
    json: String,
    fragmented: bool,
}
impl Classifier {
    pub(super) async fn anthropic_message(
        &mut self,
        message: Value,
        origin: &str,
        at: &str,
        facts: &mut impl FactSink,
    ) {
        let role = message["role"].as_str().unwrap_or("assistant");
        if let Some(blocks) = message["content"].as_array() {
            for block in blocks {
                self.anthropic_block(block.clone(), role, origin, at, facts)
                    .await;
            }
        } else {
            self.item(message, origin, at, facts).await;
        }
    }
    async fn anthropic_block(
        &mut self,
        block: Value,
        role: &str,
        origin: &str,
        at: &str,
        facts: &mut impl FactSink,
    ) {
        let kind = block["type"].as_str().unwrap_or("");
        if !matches!(
            kind,
            "tool_use" | "tool_result" | "thinking" | "redacted_thinking"
        ) {
            self.item(json!({"role":role,"content":block}), origin, at, facts)
                .await;
            return;
        }
        let category = match kind {
            "tool_use" => "tool_call",
            "tool_result" => "tool_result",
            _ => "reasoning",
        };
        let call_id = block[if kind == "tool_result" {
            "tool_use_id"
        } else {
            "id"
        }]
        .as_str()
        .and_then(bounded_id);
        let related = if kind == "tool_result" {
            call_id.as_ref().and_then(|id| self.calls.get(id))
        } else {
            None
        };
        let name = block["name"]
            .as_str()
            .map(str::to_owned)
            .or_else(|| related.map(|(_, name, _)| name.clone()))
            .unwrap_or_else(|| category.into());
        let related_step_id = related.map(|(id, _, _)| *id);
        let mut step = self.step(Uuid::now_v7(), category, &name, origin, at, &block);
        step.call_id = call_id.clone();
        step.item_id = block["id"].as_str().and_then(bounded_id);
        step.related_step_id = related_step_id;
        let mut sections = vec![("overview", block.clone())];
        if kind == "tool_use" {
            if let Some(input) = block.get("input") {
                step.parameters_preview = Some(preview(input));
                sections.push(("parameters", input.clone()));
            }
            if let Some(schema) = self.schemas.get(None, &name) {
                sections.push(("schema", schema.clone()));
            }
            if let Some(id) = call_id {
                self.calls.insert(id, (step.id, name, None));
            }
        } else {
            let result = if kind == "tool_result" {
                block.get("content")
            } else {
                block.get("thinking").or_else(|| block.get("data"))
            };
            if let Some(result) = result {
                step.preview = preview(result);
                step.result_preview = Some(preview(result));
                sections.push(("result", result.clone()));
            }
            if block["is_error"] == true {
                step.status = "failed".into();
            }
        }
        self.emit(step, sections, at, facts).await;
    }
    async fn anthropic_identity(&mut self, value: &Value, facts: &mut impl FactSink) {
        if let Some(id) = value["id"].as_str() {
            if self.response_id.as_deref() != Some(id) {
                self.response_id = bounded_id(id);
                facts
                    .push(Fact::ResponseLink {
                        response_id: id.into(),
                    })
                    .await;
            }
        }
    }
    fn anthropic_usage(&mut self, usage: &Value) {
        if let Some(fields) = usage.as_object() {
            if !self.anthropic_usage.is_object() {
                self.anthropic_usage = json!({});
            }
            for (key, value) in fields {
                self.anthropic_usage[key] = value.clone();
            }
        }
    }
    async fn emit_anthropic_usage(&mut self, at: &str, facts: &mut impl FactSink) {
        if !self.anthropic_usage.is_null() && !self.usage_seen {
            self.usage_seen = true;
            let value = self.anthropic_usage.clone();
            let step = self.step(Uuid::now_v7(), "usage", "Usage", "emitted", at, &value);
            self.emit(step, vec![("usage", value)], at, facts).await;
        }
    }
    async fn stop_anthropic_block(&mut self, index: u64, at: &str, facts: &mut impl FactSink) {
        let Some(mut block) = self.anthropic_blocks.remove(&index) else {
            self.incomplete = true;
            return;
        };
        if block.fragmented {
            match serde_json::from_str::<Value>(&block.json) {
                Ok(input) if input.is_object() => block.value["input"] = input,
                _ => {
                    self.incomplete = true;
                    block.value["input"] = Value::String(block.json);
                }
            }
        }
        self.anthropic_block(block.value, "assistant", "emitted", at, facts)
            .await;
    }
    /// EOF preserves unfinished content as evidence without inventing a protocol terminal.
    pub(crate) async fn finish_anthropic_into(&mut self, at: &str, facts: &mut impl FactSink) {
        if !self.anthropic_blocks.is_empty() {
            self.incomplete = true;
        }
        let indices: Vec<_> = self.anthropic_blocks.keys().copied().collect();
        for index in indices {
            self.stop_anthropic_block(index, at, facts).await;
        }
        self.emit_anthropic_usage(at, facts).await;
    }
    pub(super) async fn anthropic_response(
        &mut self,
        value: Value,
        at: &str,
        facts: &mut impl FactSink,
    ) {
        match value["type"].as_str().unwrap_or("") {
            "message" => {
                self.anthropic_identity(&value, facts).await;
                self.anthropic_message(value.clone(), "emitted", at, facts)
                    .await;
                self.anthropic_usage(&value["usage"]);
                self.emit_anthropic_usage(at, facts).await;
                self.completed = true;
            }
            "message_start" => {
                self.anthropic_identity(&value["message"], facts).await;
                self.anthropic_usage(&value["message"]["usage"]);
                self.anthropic_message(value["message"].clone(), "emitted", at, facts)
                    .await;
            }
            "content_block_start" => {
                if let (Some(index), Some(block)) = (
                    value["index"].as_u64(),
                    value.get("content_block").filter(|v| v.is_object()),
                ) {
                    if self
                        .anthropic_blocks
                        .insert(
                            index,
                            ContentBlock {
                                value: block.clone(),
                                json: String::new(),
                                fragmented: false,
                            },
                        )
                        .is_some()
                    {
                        self.incomplete = true;
                    }
                } else {
                    self.incomplete = true;
                }
            }
            "content_block_delta" => {
                let Some(block) = value["index"]
                    .as_u64()
                    .and_then(|index| self.anthropic_blocks.get_mut(&index))
                else {
                    self.incomplete = true;
                    return;
                };
                let delta = &value["delta"];
                let field = match delta["type"].as_str().unwrap_or("") {
                    "text_delta" => "text",
                    "thinking_delta" => "thinking",
                    "signature_delta" => "signature",
                    "input_json_delta" => {
                        if let Some(text) = delta["partial_json"].as_str() {
                            block.json.push_str(text);
                            block.fragmented = true;
                        } else {
                            self.incomplete = true;
                        }
                        return;
                    }
                    _ => {
                        self.incomplete = true;
                        return;
                    }
                };
                if let Some(text) = delta[field].as_str() {
                    let previous = block.value[field].as_str().unwrap_or_default();
                    block.value[field] = Value::String(format!("{previous}{text}"));
                } else {
                    self.incomplete = true;
                }
            }
            "content_block_stop" => {
                if let Some(index) = value["index"].as_u64() {
                    self.stop_anthropic_block(index, at, facts).await;
                } else {
                    self.incomplete = true;
                }
            }
            "message_delta" => self.anthropic_usage(&value["usage"]),
            "message_stop" => {
                self.finish_anthropic_into(at, facts).await;
                self.completed = true;
            }
            "error" => {
                self.finish_anthropic_into(at, facts).await;
                let error = value.get("error").cloned().unwrap_or(value.clone());
                let mut step = self.step(Uuid::now_v7(), "error", "Error", "emitted", at, &error);
                step.status = "failed".into();
                self.emit(
                    step,
                    vec![("overview", value), ("result", error)],
                    at,
                    facts,
                )
                .await;
                self.completed = true;
            }
            "ping" => {}
            _ if value.get("input_tokens").is_some() => {
                self.anthropic_usage(&value);
                self.emit_anthropic_usage(at, facts).await;
                self.completed = true;
            }
            _ if value.get("error").is_some() => {
                let mut step = self.step(
                    Uuid::now_v7(),
                    "error",
                    "Error",
                    "emitted",
                    at,
                    &value["error"],
                );
                step.status = "failed".into();
                self.emit(step, vec![("overview", value)], at, facts).await;
                self.completed = true;
            }
            _ => self.incomplete = true,
        }
    }
}
