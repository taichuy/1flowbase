use super::*;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum OpenAiResponseOutputItemKind {
    Reasoning,
    Message,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum OpenAiResponseStreamState {
    Initial,
    Streaming,
    Terminal,
}

pub(crate) struct OpenAiResponseStreamMapper {
    model: String,
    previous_response_id: Option<String>,
    active_output_item: Option<OpenAiResponseOutputItemKind>,
    mode: ResponsesProjectionMode,
    active_output_item_text: String,
    completed_output_items: Vec<Value>,
    output_item_index: usize,
    next_sequence_number: u64,
    state: OpenAiResponseStreamState,
}

impl OpenAiResponseStreamMapper {
    #[cfg(test)]
    pub(in crate::routes::application_public_api::compat_sse) fn new(
        model: String,
        previous_response_id: Option<String>,
    ) -> Self {
        Self::with_mode(
            model,
            previous_response_id,
            ResponsesProjectionMode::SemanticNativeResponses,
        )
    }

    pub(in crate::routes::application_public_api) fn with_mode(
        model: String,
        previous_response_id: Option<String>,
        mode: ResponsesProjectionMode,
    ) -> Self {
        Self {
            model,
            previous_response_id,
            active_output_item: None,
            mode,
            active_output_item_text: String::new(),
            completed_output_items: Vec::new(),
            output_item_index: 0,
            next_sequence_number: 0,
            state: OpenAiResponseStreamState::Initial,
        }
    }

    fn event(&mut self, event_name: &str, mut payload: Value) -> Result<Event, Infallible> {
        payload["sequence_number"] = json!(self.next_sequence_number);
        self.next_sequence_number = self.next_sequence_number.saturating_add(1);
        event_json_sse(event_name, payload)
    }

    fn open_output_item(
        &mut self,
        initial_run: &NativeRunResult,
        kind: OpenAiResponseOutputItemKind,
        events: &mut Vec<Result<Event, Infallible>>,
    ) {
        if self.active_output_item == Some(kind) {
            return;
        }
        self.close_output_item(initial_run, events);
        events.push(self.event(
            "response.output_item.added",
            json!({
                "type": "response.output_item.added",
                "response_id": native_response_id(initial_run),
                "output_index": self.output_item_index,
                "item": openai_response_output_item_payload(initial_run, kind, None)
            }),
        ));
        self.active_output_item = Some(kind);
        self.active_output_item_text = String::new();
    }

    fn close_output_item(
        &mut self,
        initial_run: &NativeRunResult,
        events: &mut Vec<Result<Event, Infallible>>,
    ) {
        let Some(kind) = self.active_output_item.take() else {
            return;
        };
        let text = std::mem::take(&mut self.active_output_item_text);
        let item = openai_response_output_item_payload(initial_run, kind, Some(text));
        events.push(self.event(
            "response.output_item.done",
            json!({
                "type": "response.output_item.done",
                "response_id": native_response_id(initial_run),
                "output_index": self.output_item_index,
                "item": item.clone()
            }),
        ));
        self.completed_output_items.push(item);
        self.output_item_index += 1;
    }

    fn project_provider_output_item(
        &mut self,
        initial_run: &NativeRunResult,
        envelope: &RuntimeEventEnvelope,
        events: &mut Vec<Result<Event, Infallible>>,
    ) -> bool {
        let event_name = match envelope.event_type.as_str() {
            "provider_output_item_added" => "response.output_item.added",
            "provider_output_item_done" => "response.output_item.done",
            _ => return false,
        };
        let Some(output_index) = envelope
            .payload
            .get("output_index")
            .and_then(Value::as_u64)
            .and_then(|value| usize::try_from(value).ok())
        else {
            return true;
        };
        let Some(item) = envelope.payload.get("item").cloned() else {
            return true;
        };
        if envelope.event_type == "provider_output_item_done"
            && is_client_executable_item(&item)
            && envelope
                .payload
                .get("committed_delivery")
                .and_then(Value::as_bool)
                != Some(true)
        {
            return true;
        }

        self.close_output_item(initial_run, events);
        events.push(self.event(
            event_name,
            json!({
                "type": event_name,
                "response_id": native_response_id(initial_run),
                "output_index": output_index,
                "item": item.clone()
            }),
        ));
        if envelope.event_type == "provider_output_item_done" {
            self.completed_output_items.push(item);
        }
        self.output_item_index = self.output_item_index.max(output_index.saturating_add(1));
        true
    }

    pub(in crate::routes::application_public_api::compat_sse) fn runtime_event_to_sse(
        &mut self,
        initial_run: &NativeRunResult,
        event: impl Into<CompatibleRuntimeEventView>,
    ) -> Vec<Result<Event, Infallible>> {
        let event = event.into();
        let envelope = event.envelope();
        if self.state == OpenAiResponseStreamState::Terminal {
            return Vec::new();
        }
        if envelope.event_type == "provider_native_event" {
            return Vec::new();
        }
        if envelope.event_type == "flow_started" {
            if self.state != OpenAiResponseStreamState::Initial {
                return Vec::new();
            }
            self.state = OpenAiResponseStreamState::Streaming;
        } else if self.state == OpenAiResponseStreamState::Initial {
            self.state = OpenAiResponseStreamState::Streaming;
        }
        let is_terminal = event.terminal().is_some();
        let mut events = Vec::new();
        if envelope.event_type == "provider_responses_output_delta" {
            if self.mode != ResponsesProjectionMode::TransparentProviderResponses {
                return events;
            }
            if let Some(mut payload) = envelope.payload.get("event").cloned() {
                payload["response_id"] = json!(native_response_id(initial_run));
                let kind = payload["type"].as_str().unwrap_or_default().to_string();
                events.push(self.event(&kind, payload));
            }
            return events;
        }
        if self.mode == ResponsesProjectionMode::TransparentProviderResponses
            && event.answer_delta().is_some()
        {
            return events;
        }
        if self.mode == ResponsesProjectionMode::TransparentProviderResponses
            && self.project_provider_output_item(initial_run, envelope, &mut events)
        {
            return events;
        }
        match event.answer_delta() {
            Some(CompatibleAnswerDeltaKind::Reasoning) => {
                self.open_output_item(
                    initial_run,
                    OpenAiResponseOutputItemKind::Reasoning,
                    &mut events,
                );
                if let Some(text) = envelope.text.as_deref().filter(|text| !text.is_empty()) {
                    self.active_output_item_text.push_str(text);
                }
            }
            Some(CompatibleAnswerDeltaKind::Text) => {
                self.open_output_item(
                    initial_run,
                    OpenAiResponseOutputItemKind::Message,
                    &mut events,
                );
                if let Some(text) = envelope.text.as_deref().filter(|text| !text.is_empty()) {
                    self.active_output_item_text.push_str(text);
                }
            }
            _ => {}
        }

        if is_terminal {
            self.close_output_item(initial_run, &mut events);
        }
        let runtime_events = openai_response_runtime_event_to_sse(
            initial_run,
            &self.model,
            self.previous_response_id.as_deref(),
            &self.completed_output_items,
            event.into_envelope(),
        );
        for (event_name, payload) in runtime_events {
            events.push(self.event(event_name, payload));
        }
        if is_terminal {
            self.state = OpenAiResponseStreamState::Terminal;
        }
        events
    }
}

fn is_client_executable_item(item: &Value) -> bool {
    let item_type = item.get("type").and_then(Value::as_str).unwrap_or_default();
    item_type.ends_with("_call") || item_type == "mcp_approval_request"
}
