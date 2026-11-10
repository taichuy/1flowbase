use std::collections::BTreeMap;

use control_plane::ports::{LlmInputRepresentation, LlmInputSelection, RuntimeEventEnvelope};
use uuid::Uuid;

use super::ResponsesProjectionMode;

/// Resolve Preferred input per node execution, never from plugin output or sidecar presence.
pub(crate) struct ResponsesProjectionSelector {
    mode: ResponsesProjectionMode,
    run_id: Option<Uuid>,
    unresolved_output: bool,
    selections: BTreeMap<Uuid, LlmInputSelection>,
}

impl ResponsesProjectionSelector {
    pub(crate) fn new(mode: ResponsesProjectionMode) -> Self {
        Self {
            mode,
            run_id: None,
            unresolved_output: false,
            selections: BTreeMap::new(),
        }
    }

    pub(crate) fn observe(&mut self, run_id: Uuid, event: &RuntimeEventEnvelope) {
        self.run_id.get_or_insert(run_id);
        if self.run_id == Some(run_id) && event.run_id == run_id {
            if let Some(selection) = LlmInputSelection::from_host_event(event) {
                self.selections.insert(selection.node_run_id, selection);
            } else if self.mode == ResponsesProjectionMode::AwaitingHostSelection {
                let source = if matches!(
                    event.event_type.as_str(),
                    "provider_responses_output_delta"
                        | "provider_output_item_added"
                        | "provider_output_item_done"
                ) {
                    event
                        .node_run_id
                        .map(|id| (id, event.payload["node_id"].as_str()))
                } else if super::is_answer_presentation_delta(event) {
                    event.payload["presentation"]["source_node_run_id"]
                        .as_str()
                        .and_then(|id| Uuid::parse_str(id).ok())
                        .map(|id| (id, event.payload["presentation"]["source_node_id"].as_str()))
                } else {
                    None
                };
                if let Some((id, node_id)) = source {
                    self.unresolved_output |= !self
                        .selections
                        .get(&id)
                        .is_some_and(|selected| node_id == Some(selected.node_id.as_str()));
                }
            }
        }
    }

    pub(crate) fn selection_missing(&self) -> bool {
        self.mode == ResponsesProjectionMode::AwaitingHostSelection
            && (self.selections.is_empty() || self.unresolved_output)
    }

    pub(crate) fn has_native_selection(&self) -> bool {
        self.mode == ResponsesProjectionMode::TransparentProviderResponses
            || (self.mode == ResponsesProjectionMode::AwaitingHostSelection
                && self.selections.values().any(|selection| {
                    selection.representation == LlmInputRepresentation::NativeOpaque
                }))
    }

    pub(crate) fn native_event(&self, event: &RuntimeEventEnvelope) -> bool {
        if self.run_id.is_some_and(|id| id != event.run_id) {
            return false;
        }
        match self.mode {
            ResponsesProjectionMode::TransparentProviderResponses => true,
            ResponsesProjectionMode::SemanticNativeResponses => false,
            ResponsesProjectionMode::AwaitingHostSelection => event
                .node_run_id
                .and_then(|id| self.selections.get(&id))
                .is_some_and(|selection| {
                    event
                        .payload
                        .get("node_id")
                        .and_then(serde_json::Value::as_str)
                        == Some(selection.node_id.as_str())
                        && selection.representation == LlmInputRepresentation::NativeOpaque
                }),
        }
    }

    pub(crate) fn semantic_event(&self, event: &RuntimeEventEnvelope) -> bool {
        if self.run_id.is_some_and(|id| id != event.run_id) {
            return false;
        }
        match self.mode {
            ResponsesProjectionMode::SemanticNativeResponses => true,
            ResponsesProjectionMode::TransparentProviderResponses => false,
            ResponsesProjectionMode::AwaitingHostSelection => {
                let presentation = &event.payload["presentation"];
                match presentation.get("source_node_run_id") {
                    Some(id) => id
                        .as_str()
                        .and_then(|id| Uuid::parse_str(id).ok())
                        .and_then(|id| self.selections.get(&id))
                        .is_some_and(|selection| {
                            presentation["source_node_id"].as_str()
                                == Some(selection.node_id.as_str())
                                && selection.representation
                                    == LlmInputRepresentation::SemanticMapped
                        }),
                    // Static presentation segments have no provider source. Native
                    // completed items already contain their canonical text.
                    None => !self.selections.is_empty() && !self.has_native_selection(),
                }
            }
        }
    }
}
