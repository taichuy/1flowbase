use super::*;

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
    next_content_index: u32,
    active_content: Option<AnthropicContentBlockKind>,
    emitted_reasoning_delta: bool,
    emitted_text_delta: bool,
    terminal_stop_reason: AnthropicMessageStopReason,
}

impl AnthropicStreamMapper {
    pub(in crate::routes::application_public_api::compat_sse) fn new(model: String) -> Self {
        Self {
            model,
            next_content_index: 0,
            active_content: None,
            emitted_reasoning_delta: false,
            emitted_text_delta: false,
            terminal_stop_reason: AnthropicMessageStopReason::EndTurn,
        }
    }

    pub(in crate::routes::application_public_api::compat_sse) fn runtime_event_to_sse(
        &mut self,
        initial_run: &NativeRunResult,
        event: impl Into<CompatibleRuntimeEventView>,
    ) -> Vec<Result<Event, Infallible>> {
        let event = event.into();
        let envelope = event.envelope();
        let visible_text = envelope
            .text
            .as_deref()
            .is_some_and(|text| !text.is_empty());
        match event.answer_delta() {
            Some(CompatibleAnswerDeltaKind::Reasoning) if visible_text => {
                self.emitted_reasoning_delta = true;
                return self.anthropic_delta_events(
                    "reasoning_delta",
                    envelope.text.clone().unwrap_or_default(),
                );
            }
            Some(CompatibleAnswerDeltaKind::Text) if visible_text => {
                self.emitted_text_delta = true;
                let mut events =
                    self.ensure_anthropic_content_block(AnthropicContentBlockKind::Text);
                let (event_name, payload) = anthropic_delta_payload(
                    self.active_content_index(),
                    "text_delta",
                    envelope.text.clone().unwrap_or_default(),
                )
                .expect("text_delta should map to Anthropic text_delta");
                events.push(event_json_sse(event_name, payload));
                return events;
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
                return self
                    .anthropic_tool_use_events(&envelope.payload, initial_run.usage.as_ref())
                    .unwrap_or_else(required_action_not_supported_anthropic_sse);
            }
            Some(CompatibleTerminalKind::WaitingHuman) => {
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
            "flow_started" => vec![event_json_sse(
                "message_start",
                json!({
                    "type": "message_start",
                    "message": {
                        "id": format!("msg_{}", initial_run.id),
                        "type": "message",
                        "role": "assistant",
                        "model": self.model,
                        "content": [],
                        "stop_reason": null,
                        "usage": anthropic_message_start_usage_payload(initial_run.usage.as_ref())
                    }
                }),
            )],
            "text_delta" | "reasoning_delta" => Vec::new(),
            "finish" => Vec::new(),
            _ => Vec::new(),
        }
    }

    fn anthropic_terminal_events(
        &mut self,
        initial_run: &NativeRunResult,
        payload: &Value,
    ) -> Vec<Result<Event, Infallible>> {
        let mut events = self.anthropic_terminal_content_events(initial_run, payload);
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
                    "type": "api_error",
                    "message": canonical_runtime_error_message(initial_run)
                }
            }),
        ));
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
        events
    }

    fn anthropic_terminal_content_events(
        &mut self,
        initial_run: &NativeRunResult,
        payload: &Value,
    ) -> Vec<Result<Event, Infallible>> {
        let mut events = Vec::new();
        let had_reasoning_delta = self.emitted_reasoning_delta;
        let had_text_delta = self.emitted_text_delta;
        for delta in terminal_answer_deltas_from_run_or_payload(initial_run, payload) {
            match delta.kind {
                TerminalAnswerDeltaKind::Reasoning if !had_reasoning_delta => {
                    events.extend(self.anthropic_delta_events("reasoning_delta", delta.text));
                    self.emitted_reasoning_delta = true;
                }
                TerminalAnswerDeltaKind::Text if !had_text_delta => {
                    events.extend(self.anthropic_delta_events("text_delta", delta.text));
                    self.emitted_text_delta = true;
                }
                _ => {}
            }
        }
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

    pub(super) fn anthropic_stop_events(
        &mut self,
        usage: Option<&NativeUsage>,
    ) -> Vec<Result<Event, Infallible>> {
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
        events
    }

    pub(in crate::routes::application_public_api::compat_sse) fn anthropic_tool_use_events(
        &mut self,
        payload: &Value,
        usage: Option<&NativeUsage>,
    ) -> Option<Vec<Result<Event, Infallible>>> {
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
                json!({"type": "thinking", "thinking": "", "signature": ""})
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
