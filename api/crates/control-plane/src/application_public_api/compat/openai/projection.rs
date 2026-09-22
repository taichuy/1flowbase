//! Semantic Responses output projection, shared by delivery and durable evidence.
use serde_json::{json, Value};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputKind {
    Reasoning,
    Message,
}

pub fn output_item(run_id: Uuid, kind: OutputKind, text: Option<String>) -> Value {
    match kind {
        OutputKind::Reasoning => json!({"type":"reasoning", "id":format!("rs_{run_id}"),
            "summary":[], "content":text.map(|text| json!([{"type":"reasoning_text","text":text}])).unwrap_or_else(|| json!([])), "encrypted_content":null}),
        OutputKind::Message => {
            json!({"type":"message", "id":format!("msg_{run_id}"), "role":"assistant",
            "content":text.map(|text| json!([{"type":"output_text","text":text}])).unwrap_or_else(|| json!([]))})
        }
    }
}

pub fn function_call_items<'a>(calls: impl IntoIterator<Item = &'a Value>) -> Vec<Value> {
    calls
        .into_iter()
        .filter(|call| !llm_tool_call_is_internal(call))
        .filter_map(|call| {
            let name = call.get("name")?.as_str()?;
            let id = call
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or("tool_call");
            let arguments = call.get("arguments").cloned().unwrap_or_else(|| json!({}));
            let arguments = match arguments {
                Value::String(value) => value,
                value => value.to_string(),
            };
            Some(
                json!({"id":format!("fc_{id}"),"type":"function_call","call_id":id,
            "name":name,"arguments":arguments,"status":"completed"}),
            )
        })
        .collect()
}

/// Captures only the ordered output selected by AnswerPresentation, at production time.
/// Provider transcripts, debug logs and user metadata are not projection evidence.
#[derive(Debug, Default, Clone)]
pub(crate) struct PresentedOutput(Vec<(OutputKind, String)>);
impl PresentedOutput {
    pub(crate) fn push(&mut self, kind: OutputKind, text: &str) {
        if let Some((previous, content)) = self.0.last_mut() {
            if *previous == kind {
                content.push_str(text);
                return;
            }
        }
        self.0.push((kind, text.to_owned()));
    }
    pub(crate) fn items(&self, run_id: Uuid) -> Vec<Value> {
        self.0
            .iter()
            .map(|(kind, text)| output_item(run_id, *kind, Some(text.clone())))
            .collect()
    }
}

/// Seal only after the presentation producer and pending external tool set are final.
pub(crate) fn round_evidence(
    round_id: Uuid,
    input_history: &Value,
    mut output: Vec<Value>,
    calls: &[Value],
) -> anyhow::Result<domain::orchestration::ResponsesRoundEvidence> {
    output.extend(function_call_items(calls));
    Ok(domain::orchestration::ResponsesRoundEvidence {
        response_id: format!("resp_{round_id}"),
        history: super::history::append_items(input_history, &output)?,
        output,
    })
}

const VISIBLE_INTERNAL_LLM_TOOL_TYPE: &str = "visible_internal_llm_tool";

// Existing public visibility predicate, shared without changing classification.
pub fn llm_tool_call_is_internal(call: &Value) -> bool {
    call.get("type").and_then(Value::as_str) == Some(VISIBLE_INTERNAL_LLM_TOOL_TYPE)
        || call.get("origin").and_then(Value::as_str) == Some(VISIBLE_INTERNAL_LLM_TOOL_TYPE)
        || call.get("source").and_then(Value::as_str) == Some(VISIBLE_INTERNAL_LLM_TOOL_TYPE)
        || call.get("visibility").and_then(Value::as_str) == Some("internal")
        || call
            .get("metadata")
            .is_some_and(metadata_marks_internal_llm_tool_call)
}

fn metadata_marks_internal_llm_tool_call(metadata: &Value) -> bool {
    metadata.get("type").and_then(Value::as_str) == Some(VISIBLE_INTERNAL_LLM_TOOL_TYPE)
        || metadata.get("origin").and_then(Value::as_str) == Some(VISIBLE_INTERNAL_LLM_TOOL_TYPE)
        || metadata.get("visibility").and_then(Value::as_str) == Some("internal")
}
