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

pub(in crate::routes::application_public_api::compat_sse) struct OpenAiResponseStreamMapper {
    model: String,
    previous_response_id: Option<String>,
    active_output_item: Option<OpenAiResponseOutputItemKind>,
    active_output_item_text: String,
    output_item_index: usize,
    state: OpenAiResponseStreamState,
}

impl OpenAiResponseStreamMapper {
    pub(in crate::routes::application_public_api::compat_sse) fn new(
        model: String,
        previous_response_id: Option<String>,
    ) -> Self {
        Self {
            model,
            previous_response_id,
            active_output_item: None,
            active_output_item_text: String::new(),
            output_item_index: 0,
            state: OpenAiResponseStreamState::Initial,
        }
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
        events.push(event_json_sse(
            "response.output_item.added",
            json!({
                "type": "response.output_item.added",
                "response_id": response_id_from_run_id(initial_run.id),
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
        events.push(event_json_sse(
            "response.output_item.done",
            json!({
                "type": "response.output_item.done",
                "response_id": response_id_from_run_id(initial_run.id),
                "output_index": self.output_item_index,
                "item": openai_response_output_item_payload(initial_run, kind, Some(text))
            }),
        ));
        self.output_item_index += 1;
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
        events.extend(openai_response_runtime_event_to_sse(
            initial_run,
            &self.model,
            self.previous_response_id.as_deref(),
            event.into_envelope(),
        ));
        if is_terminal {
            self.state = OpenAiResponseStreamState::Terminal;
        }
        events
    }
}
