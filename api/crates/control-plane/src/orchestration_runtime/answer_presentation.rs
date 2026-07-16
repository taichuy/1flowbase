use std::collections::BTreeMap;

use orchestration_runtime::answer_presentation::{
    AnswerPresentationPlan, AnswerPresentationSegment,
};
use plugin_framework::provider_contract::ProviderStreamEvent;
use serde_json::{Map, Value};
use uuid::Uuid;

use crate::ports::RuntimeEventPayload;

use super::{debug_stream_events, DebugDeltaKind, ThinkTagStreamSplitter};

#[derive(Debug)]
pub(super) struct AnswerPresentationCursor {
    candidates: Vec<AnswerPresentationCandidateCursor>,
    selected_candidate_index: Option<usize>,
}

#[derive(Debug)]
struct AnswerPresentationCandidateCursor {
    plan: AnswerPresentationPlan,
    next_segment_index: usize,
    emitted_text: BTreeMap<usize, String>,
    completed_outputs: BTreeMap<(String, String), CompletedOutput>,
}

#[derive(Debug, Clone)]
struct CompletedOutput {
    value: String,
    node_run_id: Option<Uuid>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ReadyAnswerOutput {
    pub(super) answer_node_id: String,
    pub(super) answer_output_key: String,
    pub(super) text: String,
    pub(super) complete: bool,
}

pub(super) fn ready_waiting_answer_output_from_variable_pool(
    plan: &orchestration_runtime::compiled_plan::CompiledPlan,
    variable_pool: &Map<String, Value>,
    active_node_ids: &[String],
    waiting_node_id: &str,
) -> Option<ReadyAnswerOutput> {
    AnswerPresentationPlan::candidates_from_plan(plan)
        .into_iter()
        .filter(|presentation| {
            active_node_ids.contains(&presentation.answer_node_id)
                || presentation
                    .node_output_segments()
                    .iter()
                    .any(|(_, node_id, _)| *node_id == waiting_node_id)
        })
        .filter_map(|presentation| ready_answer_output_for_plan(presentation, variable_pool))
        .max_by_key(|(ready, resolved_output_count)| {
            (
                u8::from(ready.complete),
                *resolved_output_count,
                ready.text.len(),
            )
        })
        .map(|(ready, _)| ready)
}

fn ready_answer_output_for_plan(
    presentation: AnswerPresentationPlan,
    variable_pool: &Map<String, Value>,
) -> Option<(ReadyAnswerOutput, usize)> {
    let mut text = String::new();
    let mut complete = true;
    let mut resolved_output_count = 0;

    for segment in &presentation.segments {
        match segment {
            AnswerPresentationSegment::StaticText(value) => {
                text.push_str(value);
            }
            AnswerPresentationSegment::NodeOutput {
                node_id,
                output_key,
            } => {
                let Some(value) = variable_pool
                    .get(node_id)
                    .and_then(|node_output| node_output.get(output_key))
                    .and_then(Value::as_str)
                else {
                    complete = false;
                    break;
                };
                text.push_str(value);
                resolved_output_count += 1;
            }
        }
    }

    Some((
        ReadyAnswerOutput {
            answer_node_id: presentation.answer_node_id,
            answer_output_key: presentation.answer_output_key,
            text,
            complete,
        },
        resolved_output_count,
    ))
}

pub(super) fn ready_answer_output_payload(
    ready: &ReadyAnswerOutput,
    variable_pool: &Map<String, Value>,
) -> Value {
    let mut payload = Map::new();
    payload.insert(
        ready.answer_output_key.clone(),
        Value::String(ready.text.clone()),
    );
    if let Some(sys) = variable_pool.get("sys") {
        payload.insert("sys".to_string(), sys.clone());
    }
    if let Some(env) = variable_pool.get("env") {
        payload.insert("env".to_string(), env.clone());
    }
    if let Some(conversation) = variable_pool.get("conversation") {
        payload.insert("conversation".to_string(), conversation.clone());
    }
    Value::Object(payload)
}

impl AnswerPresentationCursor {
    pub(super) fn from_plan(
        plan: &orchestration_runtime::compiled_plan::CompiledPlan,
    ) -> Option<Self> {
        let candidates = AnswerPresentationPlan::candidates_from_plan(plan)
            .into_iter()
            .map(AnswerPresentationCandidateCursor::new)
            .collect::<Vec<_>>();
        if candidates.is_empty() {
            return None;
        }
        let selected_candidate_index = (candidates.len() == 1).then_some(0);
        Some(Self {
            candidates,
            selected_candidate_index,
        })
    }

    pub(super) fn from_presentation(plan: AnswerPresentationPlan) -> Self {
        Self {
            candidates: vec![AnswerPresentationCandidateCursor::new(plan)],
            selected_candidate_index: Some(0),
        }
    }

    pub(super) fn push_provider_event(
        &mut self,
        source_node_id: &str,
        source_node_run_id: Uuid,
        event: &ProviderStreamEvent,
    ) -> Vec<RuntimeEventPayload> {
        let Some(candidate) = self.select_candidate_for_source(source_node_id) else {
            return Vec::new();
        };
        candidate.push_provider_event(source_node_id, source_node_run_id, event)
    }

    #[cfg(test)]
    pub(super) fn complete_node(
        &mut self,
        node_id: &str,
        node_run_id: Uuid,
        output_payload: &Value,
    ) -> Vec<RuntimeEventPayload> {
        self.complete_node_with_run_id(node_id, Some(node_run_id), output_payload)
    }

    pub(super) fn complete_node_with_run_id(
        &mut self,
        node_id: &str,
        node_run_id: Option<Uuid>,
        output_payload: &Value,
    ) -> Vec<RuntimeEventPayload> {
        if let Some(index) = self.selected_candidate_index {
            return self.candidates[index].complete_node_with_run_id(
                node_id,
                node_run_id,
                output_payload,
            );
        }

        let matching_candidates = self
            .candidates
            .iter()
            .enumerate()
            .filter_map(|(index, candidate)| {
                candidate.is_waiting_for_source(node_id).then_some(index)
            })
            .collect::<Vec<_>>();
        if let [index] = matching_candidates.as_slice() {
            self.selected_candidate_index = Some(*index);
            return self.candidates[*index].complete_node_with_run_id(
                node_id,
                node_run_id,
                output_payload,
            );
        }

        for candidate in &mut self.candidates {
            let _ = candidate.complete_node_with_run_id(node_id, node_run_id, output_payload);
        }
        Vec::new()
    }

    fn select_candidate_for_source(
        &mut self,
        source_node_id: &str,
    ) -> Option<&mut AnswerPresentationCandidateCursor> {
        let index = match self.selected_candidate_index {
            Some(index) => index,
            None => self
                .candidates
                .iter()
                .position(|candidate| candidate.is_waiting_for_source(source_node_id))
                .or_else(|| {
                    self.candidates
                        .iter()
                        .position(|candidate| candidate.references_source(source_node_id))
                })?,
        };
        self.selected_candidate_index = Some(index);
        self.candidates.get_mut(index)
    }
}

impl AnswerPresentationCandidateCursor {
    fn new(plan: AnswerPresentationPlan) -> Self {
        Self {
            plan,
            next_segment_index: 0,
            emitted_text: BTreeMap::new(),
            completed_outputs: BTreeMap::new(),
        }
    }

    fn is_waiting_for_source(&self, source_node_id: &str) -> bool {
        for segment in self.plan.segments.iter().skip(self.next_segment_index) {
            match segment {
                AnswerPresentationSegment::StaticText(_) => continue,
                AnswerPresentationSegment::NodeOutput {
                    node_id,
                    output_key,
                } => {
                    if self
                        .completed_outputs
                        .contains_key(&(node_id.clone(), output_key.clone()))
                    {
                        continue;
                    }
                    return node_id == source_node_id;
                }
            }
        }
        false
    }

    fn references_source(&self, source_node_id: &str) -> bool {
        self.plan.segments.iter().any(|segment| {
            matches!(
                segment,
                AnswerPresentationSegment::NodeOutput { node_id, .. }
                    if node_id == source_node_id
            )
        })
    }

    pub(super) fn push_provider_event(
        &mut self,
        source_node_id: &str,
        source_node_run_id: Uuid,
        event: &ProviderStreamEvent,
    ) -> Vec<RuntimeEventPayload> {
        let (reasoning, text) = match event {
            ProviderStreamEvent::ReasoningDelta { delta } => (true, delta.as_str()),
            ProviderStreamEvent::TextDelta { delta } => (false, delta.as_str()),
            _ => return Vec::new(),
        };

        self.push_delta(source_node_id, source_node_run_id, reasoning, text)
    }

    pub(super) fn complete_node_with_run_id(
        &mut self,
        node_id: &str,
        node_run_id: Option<Uuid>,
        output_payload: &Value,
    ) -> Vec<RuntimeEventPayload> {
        if let Some(output) = output_payload.as_object() {
            for segment in &self.plan.segments {
                let AnswerPresentationSegment::NodeOutput {
                    node_id: source_node_id,
                    output_key,
                } = segment
                else {
                    continue;
                };
                if source_node_id != node_id {
                    continue;
                }
                let Some(value) = output.get(output_key).and_then(Value::as_str) else {
                    continue;
                };
                self.completed_outputs.insert(
                    (source_node_id.clone(), output_key.clone()),
                    CompletedOutput {
                        value: value.to_string(),
                        node_run_id,
                    },
                );
            }
        }

        self.drain_ready_segments()
    }

    fn push_delta(
        &mut self,
        source_node_id: &str,
        source_node_run_id: Uuid,
        reasoning: bool,
        text: &str,
    ) -> Vec<RuntimeEventPayload> {
        let mut events = self.drain_ready_segments();
        if text.is_empty() {
            return events;
        }

        let Some((segment_index, output_key)) = self.current_node_output_segment(source_node_id)
        else {
            return events;
        };
        let output_key = output_key.to_string();
        if !reasoning {
            self.emitted_text
                .entry(segment_index)
                .or_default()
                .push_str(text);
        }

        events.push(self.answer_delta(
            segment_index,
            reasoning,
            text.to_string(),
            Some(source_node_id),
            Some(source_node_run_id),
            Some(&output_key),
        ));
        events
    }

    fn current_node_output_segment(&self, source_node_id: &str) -> Option<(usize, &str)> {
        let segment_index = self.next_segment_index;
        let segment = self.plan.segments.get(segment_index)?;
        match segment {
            AnswerPresentationSegment::NodeOutput {
                node_id,
                output_key,
            } if node_id == source_node_id => Some((segment_index, output_key.as_str())),
            _ => None,
        }
    }

    fn drain_ready_segments(&mut self) -> Vec<RuntimeEventPayload> {
        let mut events = Vec::new();

        while let Some(segment) = self.plan.segments.get(self.next_segment_index) {
            match segment {
                AnswerPresentationSegment::StaticText(text) => {
                    if !text.is_empty() {
                        events.push(self.answer_delta(
                            self.next_segment_index,
                            false,
                            text.clone(),
                            None,
                            None,
                            None,
                        ));
                    }
                    self.next_segment_index += 1;
                }
                AnswerPresentationSegment::NodeOutput {
                    node_id,
                    output_key,
                } => {
                    let key = (node_id.clone(), output_key.clone());
                    let Some(completed) = self.completed_outputs.get(&key).cloned() else {
                        break;
                    };
                    let segment_index = self.next_segment_index;
                    let already = self
                        .emitted_text
                        .get(&segment_index)
                        .map(String::as_str)
                        .unwrap_or("");
                    if already.is_empty() {
                        events.extend(self.answer_deltas_from_final_text(
                            segment_index,
                            &completed.value,
                            Some(node_id),
                            completed.node_run_id,
                            Some(output_key),
                        ));
                    } else {
                        let final_visible_text = visible_answer_text(&completed.value);
                        if let Some(suffix) = final_visible_text.strip_prefix(already) {
                            if !suffix.is_empty() {
                                events.push(self.answer_delta(
                                    segment_index,
                                    false,
                                    suffix.to_string(),
                                    Some(node_id),
                                    completed.node_run_id,
                                    Some(output_key),
                                ));
                            }
                        }
                    }
                    self.next_segment_index += 1;
                }
            }
        }

        events
    }

    fn answer_deltas_from_final_text(
        &self,
        segment_index: usize,
        text: &str,
        source_node_id: Option<&str>,
        source_node_run_id: Option<Uuid>,
        source_output_key: Option<&str>,
    ) -> Vec<RuntimeEventPayload> {
        let mut splitter = ThinkTagStreamSplitter::default();
        splitter
            .split(text)
            .into_iter()
            .chain(splitter.finish())
            .map(|part| {
                self.answer_delta(
                    segment_index,
                    part.kind == DebugDeltaKind::Reasoning,
                    part.text,
                    source_node_id,
                    source_node_run_id,
                    source_output_key,
                )
            })
            .collect()
    }

    fn answer_delta(
        &self,
        segment_index: usize,
        reasoning: bool,
        text: String,
        source_node_id: Option<&str>,
        source_node_run_id: Option<Uuid>,
        source_output_key: Option<&str>,
    ) -> RuntimeEventPayload {
        if reasoning {
            debug_stream_events::answer_reasoning_delta(
                &self.plan.answer_node_id,
                text,
                segment_index,
                source_node_id,
                source_node_run_id,
                source_output_key,
            )
        } else {
            debug_stream_events::answer_text_delta(
                &self.plan.answer_node_id,
                text,
                segment_index,
                source_node_id,
                source_node_run_id,
                source_output_key,
            )
        }
    }
}

pub(super) fn visible_answer_text(text: &str) -> String {
    let mut splitter = ThinkTagStreamSplitter::default();
    splitter
        .split(text)
        .into_iter()
        .chain(splitter.finish())
        .filter(|part| part.kind == DebugDeltaKind::Text)
        .map(|part| part.text)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn branching_answer_plan() -> orchestration_runtime::compiled_plan::CompiledPlan {
        serde_json::from_value(json!({
            "flow_id": Uuid::now_v7(),
            "source_draft_id": "draft-1",
            "schema_version": "1flowbase.flow/v2",
            "topological_order": [
                "node-llm",
                "node-llm-1",
                "node-answer",
                "node-answer-1"
            ],
            "edges": [],
            "compile_issues": [],
            "nodes": {
                "node-answer": {
                    "node_id": "node-answer",
                    "node_type": "answer",
                    "alias": "Answer",
                    "container_id": null,
                    "dependency_node_ids": ["node-llm"],
                    "downstream_node_ids": [],
                    "bindings": {
                        "answer_template": {
                            "kind": "selector",
                            "raw_value": ["node-llm", "text"],
                            "selector_paths": [["node-llm", "text"]]
                        }
                    },
                    "outputs": [{
                        "key": "answer",
                        "title": "Answer",
                        "value_type": "string",
                        "selector": ["answer"]
                    }],
                    "config": {}
                },
                "node-answer-1": {
                    "node_id": "node-answer-1",
                    "node_type": "answer",
                    "alias": "Answer 1",
                    "container_id": null,
                    "dependency_node_ids": ["node-llm-1"],
                    "downstream_node_ids": [],
                    "bindings": {
                        "answer_template": {
                            "kind": "selector",
                            "raw_value": ["node-llm-1", "text"],
                            "selector_paths": [["node-llm-1", "text"]]
                        }
                    },
                    "outputs": [{
                        "key": "answer",
                        "title": "Answer",
                        "value_type": "string",
                        "selector": ["answer"]
                    }],
                    "config": {}
                }
            }
        }))
        .expect("branching answer plan should deserialize")
    }

    fn cursor_with_segments(segments: Vec<AnswerPresentationSegment>) -> AnswerPresentationCursor {
        AnswerPresentationCursor::from_presentation(AnswerPresentationPlan {
            answer_node_id: "node-answer".to_string(),
            answer_output_key: "answer".to_string(),
            segments,
        })
    }

    #[test]
    fn leading_static_text_is_emitted_before_live_node_delta() {
        let mut cursor = cursor_with_segments(vec![
            AnswerPresentationSegment::StaticText("回答：".to_string()),
            AnswerPresentationSegment::NodeOutput {
                node_id: "node-llm".to_string(),
                output_key: "text".to_string(),
            },
        ]);
        let node_run_id = Uuid::now_v7();

        let events = cursor.push_provider_event(
            "node-llm",
            node_run_id,
            &ProviderStreamEvent::TextDelta {
                delta: "he".to_string(),
            },
        );

        let text_deltas = events
            .iter()
            .filter_map(|event| event.payload["text"].as_str())
            .collect::<Vec<_>>();
        assert_eq!(text_deltas, vec!["回答：", "he"]);
    }

    #[test]
    fn branch_specific_answer_cursor_projects_selected_source_before_terminal() {
        let mut cursor = AnswerPresentationCursor::from_plan(&branching_answer_plan())
            .expect("branching plan should expose Answer Presentation");
        let node_run_id = Uuid::now_v7();

        let reasoning = cursor.push_provider_event(
            "node-llm",
            node_run_id,
            &ProviderStreamEvent::ReasoningDelta {
                delta: "先分析".to_string(),
            },
        );
        let text = cursor.push_provider_event(
            "node-llm",
            node_run_id,
            &ProviderStreamEvent::TextDelta {
                delta: "最终回答".to_string(),
            },
        );

        assert_eq!(reasoning.len(), 1);
        assert_eq!(reasoning[0].event_type, "reasoning_delta");
        assert_eq!(text.len(), 1);
        assert_eq!(text[0].event_type, "text_delta");
        assert_eq!(text[0].payload["text"], json!("最终回答"));
    }

    #[test]
    fn ready_answer_selects_branch_with_available_source_output() {
        let variable_pool = json!({
            "node-llm": { "text": "selected branch" }
        })
        .as_object()
        .expect("variable pool fixture should be an object")
        .clone();

        let ready = ready_waiting_answer_output_from_variable_pool(
            &branching_answer_plan(),
            &variable_pool,
            &["node-answer".to_string()],
            "node-llm",
        )
        .expect("selected branch should produce a ready answer");

        assert_eq!(ready.answer_node_id, "node-answer");
        assert_eq!(ready.text, "selected branch");
        assert!(ready.complete);
    }

    #[test]
    fn final_suffix_uses_visible_text_when_completed_output_contains_think_tags() {
        let mut cursor = cursor_with_segments(vec![AnswerPresentationSegment::NodeOutput {
            node_id: "node-llm".to_string(),
            output_key: "text".to_string(),
        }]);
        let node_run_id = Uuid::now_v7();
        cursor.push_provider_event(
            "node-llm",
            node_run_id,
            &ProviderStreamEvent::TextDelta {
                delta: "he".to_string(),
            },
        );

        let events = cursor.complete_node(
            "node-llm",
            node_run_id,
            &json!({ "text": "<think>reason</think>hello" }),
        );

        let text_deltas = events
            .iter()
            .filter(|event| event.event_type == "text_delta")
            .filter_map(|event| event.payload["text"].as_str())
            .collect::<Vec<_>>();
        assert_eq!(text_deltas, vec!["llo"]);
    }
}
