//! Conservative generic projections: historical prompts are Context and span
//! completion does not declare an Agent Log task terminal or a final answer.
use super::{
    attributes::{content, integer, structured, text, Attributes},
    facts::{hash, hex, occurred_at},
};
use anyhow::{ensure, Result};
use control_plane_contracts::ports::{
    AgentLogEvent, AgentLogEventKind, AgentLogUsage, AgentLogUsageBasis,
};
use serde_json::Value;

pub(super) struct Observation {
    pub session: String,
    pub task: String,
    pub parent: Option<String>,
    pub name: String,
    pub start: u64,
    pub end: u64,
    pub attrs: Attributes,
    pub raw: Value,
    pub session_summary: bool,
}
impl Observation {
    pub fn events(&self) -> Result<Vec<AgentLogEvent>> {
        let timestamp = if self.start > 0 { self.start } else { self.end };
        occurred_at(timestamp)?;
        ensure!(
            self.end == 0 || self.end >= timestamp,
            "agent_logs.otlp_time_order"
        );
        let snapshot = hex(&hash(&self.raw)?);
        let mut output = Vec::new();
        if self.session_summary {
            output.push(self.event(
                (&snapshot, "session-summary"),
                AgentLogEventKind::Context,
                None,
                None,
                None,
                timestamp,
            )?);
            return Ok(output);
        }
        if let Some(value) = self.attrs.get("gen_ai.input.messages") {
            self.messages(&snapshot, value, false, timestamp, &mut output)?;
        } else if let Some(value) = self.attrs.get("gen_ai.prompt") {
            let value = structured(value);
            if value.is_array() || value.get("messages").is_some() {
                self.messages(&snapshot, &value, false, timestamp, &mut output)?;
            } else {
                output.push(self.event(
                    (&snapshot, "input"),
                    AgentLogEventKind::Context,
                    content(&value),
                    None,
                    None,
                    timestamp,
                )?);
            }
        }
        let end = if self.end > 0 { self.end } else { timestamp };
        if let Some(value) = self.attrs.get("gen_ai.output.messages") {
            self.messages(&snapshot, value, true, end, &mut output)?;
        } else if let Some(value) = self.attrs.get("gen_ai.completion") {
            let value = structured(value);
            if value.is_array() || value.get("messages").is_some() {
                self.messages(&snapshot, &value, true, end, &mut output)?;
            } else {
                output.push(self.event(
                    (&snapshot, "output"),
                    AgentLogEventKind::Assistant,
                    content(&value),
                    None,
                    None,
                    end,
                )?);
            }
        }
        let tool = text(&self.attrs, &["gen_ai.tool.name", "tool.name"]);
        let call_id = text(&self.attrs, &["gen_ai.tool.call.id", "tool.call.id"]);
        if let Some(arguments) = self
            .attrs
            .get("gen_ai.tool.call.arguments")
            .or_else(|| self.attrs.get("gen_ai.tool.input"))
            .or_else(|| self.attrs.get("gen_ai.tool.args"))
            .or_else(|| self.attrs.get("tool.arguments"))
        {
            output.push(self.event(
                (&snapshot, "tool-call"),
                AgentLogEventKind::ToolCall,
                content(arguments),
                tool.clone(),
                call_id.clone(),
                timestamp,
            )?);
        }
        if let Some(result) = self
            .attrs
            .get("gen_ai.tool.call.result")
            .or_else(|| self.attrs.get("gen_ai.tool.output"))
            .or_else(|| self.attrs.get("tool.result"))
        {
            output.push(self.event(
                (&snapshot, "tool-result"),
                AgentLogEventKind::ToolResult,
                content(result),
                tool.clone(),
                call_id.clone(),
                end,
            )?);
        }
        if let Some(usage) = self.usage()? {
            let mut event = self.event(
                (&snapshot, "usage"),
                AgentLogEventKind::Usage,
                None,
                None,
                None,
                end,
            )?;
            event.usage = Some(usage);
            output.push(event);
        }
        if output.is_empty() {
            output.push(self.event(
                (&snapshot, "observation"),
                AgentLogEventKind::Context,
                None,
                Some(self.name.clone()),
                call_id,
                timestamp,
            )?);
        }
        // One raw source span is retained once. Other normalized events point to
        // that source unit rather than duplicating a complete transcript.
        for event in &mut output {
            event.raw = serde_json::json!({"otlp_source_snapshot": snapshot});
        }
        if let Some(first) = output.first_mut() {
            first.raw = self.raw.clone();
        }
        Ok(output)
    }

    fn event(
        &self,
        (snapshot, slot): (&str, &str),
        kind: AgentLogEventKind,
        content: Option<String>,
        name: Option<String>,
        call_id: Option<String>,
        timestamp: u64,
    ) -> Result<AgentLogEvent> {
        Ok(AgentLogEvent {
            event_id: format!("{}:{snapshot}:{slot}", self.task),
            source_session_id: self.session.clone(),
            source_task_id: self.task.clone(),
            parent_source_task_id: self.parent.clone(),
            sequence: i64::try_from(timestamp)
                .map_err(|_| anyhow::anyhow!("agent_logs.otlp_timestamp_range"))?,
            occurred_at: occurred_at(timestamp)?,
            kind,
            content,
            phase: None,
            name,
            call_id,
            model_id: text(
                &self.attrs,
                &["gen_ai.response.model", "gen_ai.request.model"],
            ),
            reasoning_effort: text(&self.attrs, &["gen_ai.request.reasoning_effort"]),
            provider_code: text(&self.attrs, &["gen_ai.provider.name", "gen_ai.system"]),
            usage: None,
            inherited: false,
            raw: self.raw.clone(),
        })
    }

    fn messages(
        &self,
        snapshot: &str,
        value: &Value,
        output: bool,
        timestamp: u64,
        events: &mut Vec<AgentLogEvent>,
    ) -> Result<()> {
        let value = structured(value);
        let messages = value.get("messages").unwrap_or(&value);
        let owned;
        let messages = match messages.as_array() {
            Some(messages) => messages.as_slice(),
            None if messages.is_object() => {
                owned = vec![messages.clone()];
                owned.as_slice()
            }
            None => {
                events.push(self.event(
                    (snapshot, if output { "output" } else { "input" }),
                    if output {
                        AgentLogEventKind::Assistant
                    } else {
                        AgentLogEventKind::Context
                    },
                    content(messages),
                    None,
                    None,
                    timestamp,
                )?);
                return Ok(());
            }
        };
        for (index, message) in messages.iter().enumerate() {
            let role = message.get("role").and_then(Value::as_str).unwrap_or("");
            // A role of `user` in a model's prompt is history, not evidence of
            // a fresh human turn. Require explicit message-level provenance.
            let human = role == "human"
                || message.get("source").and_then(Value::as_str) == Some("human")
                || message.pointer("/source/type").and_then(Value::as_str) == Some("human")
                || message.get("human").and_then(Value::as_bool) == Some(true);
            let kind = if !output {
                if human {
                    AgentLogEventKind::User
                } else {
                    AgentLogEventKind::Context
                }
            } else if role == "tool" {
                AgentLogEventKind::ToolResult
            } else {
                AgentLogEventKind::Assistant
            };
            let parts = message.get("parts").or_else(|| message.get("content"));
            if let Some(parts) = parts.and_then(Value::as_array) {
                for (part_index, part) in parts.iter().enumerate() {
                    let part_type = part.get("type").and_then(Value::as_str).unwrap_or("text");
                    let part_kind = if output {
                        match part_type {
                            "tool_call" | "function_call" => AgentLogEventKind::ToolCall,
                            "tool_call_response" | "tool_result" => AgentLogEventKind::ToolResult,
                            _ => kind,
                        }
                    } else {
                        kind
                    };
                    let body = part
                        .get("content")
                        .or_else(|| part.get("text"))
                        .or_else(|| part.get("arguments"))
                        .or_else(|| part.get("result"))
                        .or_else(|| part.get("response"))
                        .unwrap_or(part);
                    events.push(
                        self.event(
                            (
                                snapshot,
                                &format!(
                                    "{}-{index:08}-part-{part_index:08}",
                                    if output { "output" } else { "input" }
                                ),
                            ),
                            part_kind,
                            content(body),
                            part.get("name").and_then(Value::as_str).map(str::to_owned),
                            part.get("id")
                                .or_else(|| part.get("tool_call_id"))
                                .and_then(Value::as_str)
                                .map(str::to_owned),
                            timestamp,
                        )?,
                    );
                }
            } else {
                let body = parts.unwrap_or(message);
                events.push(
                    self.event(
                        (
                            snapshot,
                            &format!("{}-{index:08}", if output { "output" } else { "input" }),
                        ),
                        kind,
                        content(body),
                        message
                            .get("name")
                            .and_then(Value::as_str)
                            .map(str::to_owned),
                        message
                            .get("tool_call_id")
                            .and_then(Value::as_str)
                            .map(str::to_owned),
                        timestamp,
                    )?,
                );
            }
            // OpenAI-style tool_calls sit alongside content rather than in parts.
            if output {
                if let Some(calls) = message.get("tool_calls").and_then(Value::as_array) {
                    for (call_index, call) in calls.iter().enumerate() {
                        events.push(
                            self.event(
                                (snapshot, &format!("output-{index:08}-call-{call_index:08}")),
                                AgentLogEventKind::ToolCall,
                                call.pointer("/function/arguments")
                                    .or_else(|| call.get("arguments"))
                                    .and_then(content),
                                call.pointer("/function/name")
                                    .or_else(|| call.get("name"))
                                    .and_then(Value::as_str)
                                    .map(str::to_owned),
                                call.get("id").and_then(Value::as_str).map(str::to_owned),
                                timestamp,
                            )?,
                        );
                    }
                }
            }
        }
        Ok(())
    }

    fn usage(&self) -> Result<Option<AgentLogUsage>> {
        let input_tokens = integer(
            &self.attrs,
            &["gen_ai.usage.input_tokens", "gen_ai.usage.prompt_tokens"],
        )?;
        let output_tokens = integer(
            &self.attrs,
            &[
                "gen_ai.usage.output_tokens",
                "gen_ai.usage.completion_tokens",
            ],
        )?;
        let declared_total = integer(&self.attrs, &["gen_ai.usage.total_tokens"])?;
        let total_tokens = match (declared_total, input_tokens, output_tokens) {
            (Some(total), _, _) => Some(total),
            (None, Some(input), Some(output)) => Some(
                input
                    .checked_add(output)
                    .ok_or_else(|| anyhow::anyhow!("agent_logs.usage_overflow"))?,
            ),
            _ => None,
        };
        let input_cache_hit_tokens = integer(
            &self.attrs,
            &[
                "gen_ai.usage.cache.read_input_tokens",
                "gen_ai.usage.cache_read.input_tokens",
                "gen_ai.usage.input_cached_tokens",
                "gen_ai.usage.prompt_tokens_details.cache_read",
            ],
        )?;
        let cache_write_tokens = integer(
            &self.attrs,
            &[
                "gen_ai.usage.cache.creation_input_tokens",
                "gen_ai.usage.cache_creation.input_tokens",
                "gen_ai.usage.prompt_tokens_details.cache_write",
            ],
        )?;
        if [
            input_tokens,
            output_tokens,
            total_tokens,
            input_cache_hit_tokens,
            cache_write_tokens,
        ]
        .iter()
        .all(Option::is_none)
        {
            return Ok(None);
        }
        Ok(Some(AgentLogUsage {
            basis: AgentLogUsageBasis::Cumulative,
            response_id: Some(self.task.clone()),
            input_tokens,
            output_tokens,
            total_tokens,
            input_cache_hit_tokens,
            cache_write_tokens,
        }))
    }
}
