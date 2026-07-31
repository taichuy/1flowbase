use super::*;
use uuid::Uuid;

#[derive(Clone, Copy, PartialEq, Eq)]
enum AnthropicContentBlockKind {
    Text,
    Thinking,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum AnthropicMessageStopReason {
    EndTurn,
    MaxTokens,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum AnthropicStreamState {
    Open,
    Terminal,
}

impl AnthropicMessageStopReason {
    fn as_str(self) -> &'static str {
        match self {
            Self::EndTurn => "end_turn",
            Self::MaxTokens => "max_tokens",
        }
    }
}

pub(in crate::routes::application_public_api::compat_sse) struct AnthropicStreamMapper {
    model: String,
    message_id: String,
    next_content_index: u32,
    active_content: Option<AnthropicContentBlockKind>,
    terminal_stop_reason: AnthropicMessageStopReason,
    stream_state: AnthropicStreamState,
    message_start_emitted: bool,
}

impl AnthropicStreamMapper {
    pub(in crate::routes::application_public_api::compat_sse) fn new(model: String) -> Self {
        Self {
            model,
            message_id: format!("msg_{}", Uuid::now_v7()),
            next_content_index: 0,
            active_content: None,
            terminal_stop_reason: AnthropicMessageStopReason::EndTurn,
            stream_state: AnthropicStreamState::Open,
            message_start_emitted: false,
        }
    }

    pub(in crate::routes::application_public_api::compat_sse) fn runtime_event_to_sse(
        &mut self,
        initial_run: &NativeRunResult,
        event: impl Into<CompatibleRuntimeEventView>,
    ) -> Vec<Result<Event, Infallible>> {
        let event = event.into();
        let envelope = event.envelope();
        if self.stream_state == AnthropicStreamState::Terminal {
            return Vec::new();
        }
        match event.answer_delta() {
            Some(CompatibleAnswerDeltaKind::Reasoning) => {
                if let Some(text) = envelope.text.clone() {
                    return self.anthropic_delta_events("reasoning_delta", text);
                }
            }
            Some(CompatibleAnswerDeltaKind::Text) => {
                if let Some(text) = envelope.text.clone() {
                    return self.anthropic_delta_events("text_delta", text);
                }
            }
            _ => {}
        }
        match event.terminal() {
            Some(CompatibleTerminalKind::Finished) => {
                self.terminal_stop_reason = AnthropicMessageStopReason::EndTurn;
                return self.anthropic_terminal_events(initial_run, &envelope.payload);
            }
            Some(CompatibleTerminalKind::Incomplete) => {
                self.terminal_stop_reason = AnthropicMessageStopReason::MaxTokens;
                return self.anthropic_terminal_events(initial_run, &envelope.payload);
            }
            Some(CompatibleTerminalKind::WaitingCallback) => {
                return match self
                    .anthropic_tool_use_events(&envelope.payload, initial_run.usage.as_ref())
                {
                    Some(events) => events,
                    None => {
                        self.stream_state = AnthropicStreamState::Terminal;
                        required_action_not_supported_anthropic_sse()
                    }
                };
            }
            Some(CompatibleTerminalKind::WaitingHuman) => {
                self.stream_state = AnthropicStreamState::Terminal;
                return required_action_not_supported_anthropic_sse();
            }
            Some(CompatibleTerminalKind::Failed) => {
                return self.anthropic_failed_events(initial_run, &envelope.payload);
            }
            Some(CompatibleTerminalKind::Cancelled) => {
                return self.anthropic_cancelled_events();
            }
            None => {}
        }
        match envelope.event_type.as_str() {
            "flow_started" if !self.message_start_emitted => {
                self.message_start_emitted = true;
                vec![event_json_sse(
                    "message_start",
                    json!({
                        "type": "message_start",
                        "message": {
                            "id": self.message_id,
                            "type": "message",
                            "role": "assistant",
                            "model": self.model,
                            "content": [],
                            "stop_reason": null,
                            "usage": anthropic_message_start_usage_payload(initial_run.usage.as_ref())
                        }
                    }),
                )]
            }
            "reasoning_signature_delta" => envelope
                .payload
                .get("signature")
                .and_then(Value::as_str)
                .map(|signature| self.anthropic_signature_delta_events(signature.to_string()))
                .unwrap_or_default(),
            "text_delta" | "reasoning_delta" => Vec::new(),
            "finish" => Vec::new(),
            _ => Vec::new(),
        }
    }

    fn anthropic_terminal_events(
        &mut self,
        initial_run: &NativeRunResult,
        _payload: &Value,
    ) -> Vec<Result<Event, Infallible>> {
        let mut events = self.close_active_anthropic_content_block();
        events.extend(self.anthropic_stop_events(initial_run.usage.as_ref()));
        events
    }

    fn anthropic_failed_events(
        &mut self,
        initial_run: &NativeRunResult,
        _payload: &Value,
    ) -> Vec<Result<Event, Infallible>> {
        let mut events = self.close_active_anthropic_content_block();
        events.push(event_json_sse(
            "error",
            json!({
                "type": "error",
                "error": {
                    "type": anthropic_runtime_error_type(initial_run),
                    "message": canonical_runtime_error_message(initial_run)
                }
            }),
        ));
        self.stream_state = AnthropicStreamState::Terminal;
        events
    }

    fn anthropic_cancelled_events(&mut self) -> Vec<Result<Event, Infallible>> {
        let mut events = self.close_active_anthropic_content_block();
        events.push(event_json_sse(
            "error",
            json!({
                "type": "error",
                "error": {
                    "type": "invalid_request_error",
                    "message": "published run cancelled"
                }
            }),
        ));
        self.stream_state = AnthropicStreamState::Terminal;
        events
    }

    fn anthropic_delta_events(
        &mut self,
        event_type: &str,
        text: String,
    ) -> Vec<Result<Event, Infallible>> {
        let block_kind = match event_type {
            "reasoning_delta" => AnthropicContentBlockKind::Thinking,
            "text_delta" => AnthropicContentBlockKind::Text,
            _ => return Vec::new(),
        };
        let mut events = self.ensure_anthropic_content_block(block_kind);
        let (event_name, payload) =
            anthropic_delta_payload(self.active_content_index(), event_type, text)
                .expect("known Anthropic delta event type should map");
        events.push(event_json_sse(event_name, payload));
        events
    }

    fn anthropic_signature_delta_events(
        &mut self,
        signature: String,
    ) -> Vec<Result<Event, Infallible>> {
        let mut events = self.ensure_anthropic_content_block(AnthropicContentBlockKind::Thinking);
        events.push(event_json_sse(
            "content_block_delta",
            json!({
                "type": "content_block_delta",
                "index": self.active_content_index(),
                "delta": {
                    "type": "signature_delta",
                    "signature": signature
                }
            }),
        ));
        events
    }

    pub(super) fn anthropic_stop_events(
        &mut self,
        usage: Option<&NativeUsage>,
    ) -> Vec<Result<Event, Infallible>> {
        if self.stream_state == AnthropicStreamState::Terminal {
            return Vec::new();
        }
        let mut events = Vec::new();
        let stop_reason = self.terminal_stop_reason.as_str();
        if self.active_content.is_none() && self.next_content_index == 0 {
            events.extend(self.ensure_anthropic_content_block(AnthropicContentBlockKind::Text));
        }
        events.extend(self.close_active_anthropic_content_block());
        events.push(event_json_sse(
            "message_delta",
            json!({
                "type": "message_delta",
                "delta": {"stop_reason": stop_reason},
                "usage": anthropic_message_delta_usage_payload(usage)
            }),
        ));
        events.push(event_json_sse(
            "message_stop",
            json!({"type": "message_stop"}),
        ));
        self.stream_state = AnthropicStreamState::Terminal;
        events
    }

    pub(in crate::routes::application_public_api::compat_sse) fn anthropic_tool_use_events(
        &mut self,
        payload: &Value,
        usage: Option<&NativeUsage>,
    ) -> Option<Vec<Result<Event, Infallible>>> {
        if self.stream_state == AnthropicStreamState::Terminal {
            return Some(Vec::new());
        }
        let blocks = anthropic_tool_use_blocks_from_waiting_payload(payload)?;
        let mut events = self.close_active_anthropic_content_block();
        for block in blocks {
            let index = self.next_content_index;
            self.next_content_index += 1;
            let input = block.get("input").cloned().unwrap_or_else(|| json!({}));
            let mut start_block = block;
            if let Some(object) = start_block.as_object_mut() {
                object.insert("input".to_string(), json!({}));
            }
            events.push(event_json_sse(
                "content_block_start",
                json!({
                    "type": "content_block_start",
                    "index": index,
                    "content_block": start_block
                }),
            ));
            if input != json!({}) {
                events.push(event_json_sse(
                    "content_block_delta",
                    json!({
                        "type": "content_block_delta",
                        "index": index,
                        "delta": {
                            "type": "input_json_delta",
                            "partial_json": serde_json::to_string(&input)
                                .expect("tool input JSON should serialize")
                        }
                    }),
                ));
            }
            events.push(event_json_sse(
                "content_block_stop",
                json!({"type": "content_block_stop", "index": index}),
            ));
        }
        events.push(event_json_sse(
            "message_delta",
            json!({
                "type": "message_delta",
                "delta": {"stop_reason": "tool_use"},
                "usage": anthropic_message_delta_usage_payload(usage)
            }),
        ));
        events.push(event_json_sse(
            "message_stop",
            json!({"type": "message_stop"}),
        ));
        self.stream_state = AnthropicStreamState::Terminal;
        Some(events)
    }

    fn ensure_anthropic_content_block(
        &mut self,
        kind: AnthropicContentBlockKind,
    ) -> Vec<Result<Event, Infallible>> {
        if self.active_content == Some(kind) {
            return Vec::new();
        }

        let mut events = self.close_active_anthropic_content_block();
        let index = self.next_content_index;
        self.next_content_index += 1;
        self.active_content = Some(kind);
        let content_block = match kind {
            AnthropicContentBlockKind::Text => json!({"type": "text", "text": ""}),
            AnthropicContentBlockKind::Thinking => {
                json!({"type": "thinking", "thinking": ""})
            }
        };
        events.push(event_json_sse(
            "content_block_start",
            json!({
                "type": "content_block_start",
                "index": index,
                "content_block": content_block
            }),
        ));
        events
    }

    fn close_active_anthropic_content_block(&mut self) -> Vec<Result<Event, Infallible>> {
        if self.active_content.is_none() {
            return Vec::new();
        }
        let index = self.active_content_index();
        self.active_content = None;
        vec![event_json_sse(
            "content_block_stop",
            json!({"type": "content_block_stop", "index": index}),
        )]
    }

    fn active_content_index(&self) -> u32 {
        self.next_content_index.saturating_sub(1)
    }
}

fn anthropic_runtime_error_type(run: &NativeRunResult) -> &'static str {
    let error_code = run.error.as_ref().map(|error| error.code.as_str());
    let status = run
        .error
        .as_ref()
        .and_then(|error| error.details.get("status_code"))
        .and_then(Value::as_u64);
    if error_code == Some("rate_limited") || status == Some(429) {
        "rate_limit_error"
    } else if status == Some(529) {
        "overloaded_error"
    } else {
        "api_error"
    }
}

pub(in crate::routes::application_public_api::compat_sse) fn anthropic_delta_payload(
    index: u32,
    event_type: &str,
    text: String,
) -> Option<(&'static str, Value)> {
    let delta = match event_type {
        "reasoning_delta" => json!({
            "type": "thinking_delta",
            "thinking": text
        }),
        "text_delta" => json!({
            "type": "text_delta",
            "text": text
        }),
        _ => return None,
    };

    Some((
        "content_block_delta",
        json!({
            "type": "content_block_delta",
            "index": index,
            "delta": delta
        }),
    ))
}
