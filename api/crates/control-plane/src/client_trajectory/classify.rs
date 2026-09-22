use crate::ports::{
    ClientTrajectoryFact as Fact, ClientTrajectoryFrameKind, ClientTrajectoryStep,
    ClientTrajectoryTransport,
};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;
const MAX_IDENTITIES: usize = 4096;
const MAX_SCHEMAS: usize = 128;
// A step can reference its original item, extracted content and one tool schema.
// The input JSON and schema indexes are independently bounded.
const MAX_STEP_FACT_BYTES: usize = 3 * super::decode::AGGREGATE_BYTES;

#[async_trait::async_trait]
pub(super) trait FactSink: Send {
    async fn push(&mut self, fact: Fact);
}

pub(super) struct Classifier {
    request: Uuid,
    flow: Uuid,
    node: Option<Uuid>,
    transport: ClientTrajectoryTransport,
    response_id: Option<String>,
    turn_id: Option<String>,
    pub request_seen: bool,
    root_seen: bool,
    root_at: Option<String>,
    output_seen: BTreeSet<String>,
    calls: BTreeMap<String, (Uuid, String, Option<String>)>,
    schemas: super::schemas::SchemaIndex,
    usage_seen: bool,
    pub incomplete: bool,
    pub completed: bool,
}
impl Classifier {
    pub fn new(
        request: Uuid,
        flow: Uuid,
        node: Option<Uuid>,
        transport: ClientTrajectoryTransport,
    ) -> Self {
        Self {
            request,
            flow,
            node,
            transport,
            response_id: None,
            turn_id: None,
            request_seen: false,
            root_seen: false,
            root_at: None,
            output_seen: BTreeSet::new(),
            calls: BTreeMap::new(),
            schemas: Default::default(),
            usage_seen: false,
            incomplete: false,
            completed: false,
        }
    }
    pub async fn begin_request_into(&mut self, at: &str, facts: &mut impl FactSink) {
        if self.root_seen {
            return;
        }
        self.root_seen = true;
        self.root_at = Some(at.into());
        let mut step = self.step(
            self.request,
            "request",
            "Responses request",
            "submitted",
            at,
            &Value::Null,
        );
        step.parent_id = None;
        self.emit(step, vec![], at, facts).await;
    }
    pub async fn observe_into(
        &mut self,
        kind: ClientTrajectoryFrameKind,
        value: Value,
        at: &str,
        facts: &mut impl FactSink,
    ) {
        if !value.is_object() {
            self.incomplete = true;
            return;
        }
        if kind == ClientTrajectoryFrameKind::Request {
            self.request(value, at, facts).await;
            return;
        }
        let event_type = value["type"].as_str().unwrap_or("");
        if let Some(id) = value["response"]["id"].as_str() {
            self.response_id = bounded_id(id);
        }
        match event_type {
            "response.output_item.done" => {
                if let Some(item) = value.get("item") {
                    self.output(
                        item.clone(),
                        value["output_index"].as_u64().map(|n| n as usize),
                        at,
                        facts,
                    )
                    .await;
                } else {
                    self.incomplete = true;
                }
            }
            "response.completed" | "response.failed" | "response.incomplete" => {
                if let Some(response) = value.get("response") {
                    self.response(response, at, facts).await;
                } else {
                    self.incomplete = true;
                }
                self.completed = true;
            }
            "error" => {
                self.item(value, "emitted", at, facts).await;
                self.completed = true;
            }
            _ if value.get("output").is_some() || value["object"] == "response" => {
                self.response(&value, at, facts).await;
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
                self.emit(step, vec![("overview", value["error"].clone())], at, facts)
                    .await;
                self.completed = true;
            }
            // Deltas are not semantic items. Their exact frames are retained as raw evidence.
            _ => {}
        }
    }
    async fn request(&mut self, mut value: Value, at: &str, facts: &mut impl FactSink) {
        if self.request_seen {
            self.incomplete = true;
            return;
        }
        self.request_seen = true;
        self.turn_id = value
            .get("turn_id")
            .and_then(Value::as_str)
            .and_then(bounded_id);
        // WebSocket response.create has the same fields as HTTP Responses input.
        let Some(map) = value.as_object_mut() else {
            return;
        };
        let instructions = map.remove("instructions");
        let input = map.remove("input");
        let tools = map.remove("tools");
        let root_at = self.root_at.clone().unwrap_or_else(|| at.into());
        let mut root = self.step(
            self.request,
            "request",
            "Responses request",
            "submitted",
            &root_at,
            &Value::Null,
        );
        root.parent_id = None;
        let mut labels = Vec::new();
        if let Some(model) = value["model"].as_str() {
            labels.push(model.to_owned());
        }
        if let Some(effort) = value["reasoning"]["effort"].as_str() {
            labels.push(format!("reasoning.effort={effort}"));
        }
        if let Some(stream) = value["stream"].as_bool() {
            labels.push(format!("stream={stream}"));
        }
        root.preview = preview_text(&labels.join(" · "));
        self.emit(root, vec![("overview", value)], &root_at, facts)
            .await;
        if let Some(instructions) = instructions.filter(|value| !value.is_null()) {
            let mut step = self.step(
                Uuid::now_v7(),
                "system",
                "Instructions",
                "submitted",
                at,
                &instructions,
            );
            step.parameters_preview = Some(preview(&instructions));
            self.emit(step, vec![("parameters", instructions)], at, facts)
                .await;
        }
        if let Some(Value::Array(tools)) = tools {
            if tools.len() > MAX_SCHEMAS {
                self.incomplete = true;
            }
            for tool in tools.into_iter().take(MAX_SCHEMAS) {
                let name = tool_name(&tool)
                    .unwrap_or_else(|| tool["type"].as_str().unwrap_or("tool").to_owned());
                self.schemas.insert_root(&tool);
                self.incomplete |= self.schemas.incomplete;
                let step = self.step(
                    Uuid::now_v7(),
                    "tool_definition",
                    &name,
                    "submitted",
                    at,
                    &tool,
                );
                self.emit(step, vec![("schema", tool)], at, facts).await;
            }
        }
        match input {
            Some(Value::String(text)) => {
                self.item(
                    json!({"role":"user","content":text}),
                    "submitted",
                    at,
                    facts,
                )
                .await
            }
            Some(Value::Array(items)) => {
                for item in items {
                    self.item(item, "submitted", at, facts).await;
                }
            }
            Some(Value::Null) | None => {}
            Some(_) => self.incomplete = true,
        }
    }
    async fn response(&mut self, value: &Value, at: &str, facts: &mut impl FactSink) {
        if let Some(id) = value["id"].as_str() {
            self.response_id = bounded_id(id);
        }
        if let Some(response_id) = &self.response_id {
            facts
                .push(Fact::ResponseLink {
                    response_id: response_id.clone(),
                })
                .await;
        }
        if let Some(items) = value["output"].as_array() {
            for (index, item) in items.iter().enumerate() {
                self.output(item.clone(), Some(index), at, facts).await;
            }
        }
        if !value["usage"].is_null() && !self.usage_seen {
            self.usage_seen = true;
            let step = self.step(
                Uuid::now_v7(),
                "usage",
                "Usage",
                "emitted",
                at,
                &value["usage"],
            );
            self.emit(step, vec![("usage", value["usage"].clone())], at, facts)
                .await;
        }
        if !value["error"].is_null() {
            let mut step = self.step(
                Uuid::now_v7(),
                "error",
                "Error",
                "emitted",
                at,
                &value["error"],
            );
            step.status = "failed".into();
            self.emit(step, vec![("overview", value["error"].clone())], at, facts)
                .await;
        }
    }
    async fn output(
        &mut self,
        item: Value,
        index: Option<usize>,
        at: &str,
        facts: &mut impl FactSink,
    ) {
        let id_key = item["id"]
            .as_str()
            .and_then(bounded_id)
            .map(|id| format!("id:{id}"));
        let index_key = index.map(|index| format!("index:{index}"));
        if id_key.is_none() && index_key.is_none() {
            self.incomplete = true;
            return;
        }
        if id_key
            .iter()
            .chain(index_key.iter())
            .any(|key| self.output_seen.contains(key))
        {
            return;
        }
        if self.output_seen.len() + 2 > MAX_IDENTITIES {
            self.incomplete = true;
            return;
        }
        self.output_seen.extend(id_key.into_iter().chain(index_key));
        self.item(item, "emitted", at, facts).await;
    }
    async fn item(&mut self, item: Value, origin: &str, at: &str, facts: &mut impl FactSink) {
        if ["id", "call_id", "tool_call_id", "name", "namespace"]
            .iter()
            .any(|key| item[*key].as_str().is_some_and(|value| value.len() > 1024))
        {
            self.incomplete = true;
        }
        let kind = item["type"].as_str().unwrap_or("");
        let role = item["role"].as_str().unwrap_or("");
        let is_result =
            kind.ends_with("_call_output") || kind == "function_call_output" || role == "tool";
        let is_call = kind.ends_with("_call") || kind == "function_call";
        let category = if is_result {
            "tool_result"
        } else if is_call {
            "tool_call"
        } else if kind == "reasoning" {
            "reasoning"
        } else if kind == "error" || item.get("code").is_some() && item.get("message").is_some() {
            "error"
        } else {
            match role {
                "system" | "developer" => "system",
                "assistant" => "assistant",
                "user" => "user",
                _ => "unknown",
            }
        };
        let call_id = item["call_id"]
            .as_str()
            .or_else(|| item["tool_call_id"].as_str())
            .and_then(bounded_id);
        let declared_namespace = item["namespace"].as_str().and_then(bounded_id);
        let valid_namespace = item["namespace"].is_null() || declared_namespace.is_some();
        if !valid_namespace {
            self.incomplete = true;
        }
        let related = if is_result && valid_namespace {
            call_id
                .as_ref()
                .and_then(|id| self.calls.get(id))
                .filter(|(_, _, namespace)| {
                    declared_namespace
                        .as_ref()
                        .is_none_or(|declared| namespace.as_ref() == Some(declared))
                })
        } else {
            None
        };
        let namespace =
            declared_namespace.or_else(|| related.and_then(|(_, _, namespace)| namespace.clone()));
        let name = tool_name(&item)
            .or_else(|| related.map(|(_, name, _)| name.clone()))
            .unwrap_or_else(|| {
                if is_result || is_call {
                    call_id.clone().unwrap_or_else(|| kind.into())
                } else {
                    category.into()
                }
            });
        let related_step_id = related.map(|(id, _, _)| *id);
        let mut step = self.step(Uuid::now_v7(), category, &name, origin, at, &item);
        step.namespace = namespace.clone();
        step.call_id = call_id.clone();
        step.item_id = item["id"].as_str().and_then(bounded_id);
        let mut sections = vec![("overview", item.clone())];
        if is_call {
            if let Some(args) = item.get("arguments").or_else(|| item.get("input")) {
                step.parameters_preview = Some(preview(args));
                sections.push(("parameters", args.clone()));
            }
            if let Some(schema) = valid_namespace
                .then(|| self.schemas.get(namespace.as_deref(), &name))
                .flatten()
            {
                sections.push(("schema", schema.clone()));
            }
            if let Some(id) = call_id {
                if self.calls.len() < MAX_IDENTITIES {
                    self.calls.insert(id, (step.id, name, namespace));
                } else {
                    self.incomplete = true;
                }
            }
        } else if is_result {
            if let Some(value) = item.get("output").or_else(|| item.get("content")) {
                step.result_preview = Some(preview(value));
                step.preview = preview(value);
                sections.push(("result", value.clone()));
            }
            step.related_step_id = related_step_id;
        } else if let Some(value) = item.get("content").or_else(|| item.get("summary")) {
            step.result_preview = Some(preview(value));
            sections.push(("result", value.clone()));
        }
        if category == "error" {
            step.status = "failed".into();
        }
        self.emit(step, sections, at, facts).await;
    }
    fn step(
        &self,
        id: Uuid,
        category: &str,
        name: &str,
        origin: &str,
        at: &str,
        item: &Value,
    ) -> ClientTrajectoryStep {
        ClientTrajectoryStep {
            id,
            request_id: self.request,
            sequence: 0,
            created_at: at.into(),
            category: category.into(),
            name: preview_text(name),
            namespace: item["namespace"].as_str().and_then(bounded_id),
            preview: preview(item),
            parameters_preview: None,
            result_preview: None,
            status: if origin == "submitted" {
                "submitted"
            } else {
                item.get("status")
                    .and_then(Value::as_str)
                    .filter(|value| value.len() <= 64)
                    .unwrap_or("recorded")
            }
            .into(),
            origin: origin.into(),
            protocol: "responses".into(),
            transport: self.transport,
            flow_run_id: self.flow,
            node_run_id: self.node,
            parent_id: Some(self.request),
            call_id: None,
            item_id: None,
            response_id: if origin == "emitted" {
                self.response_id.clone()
            } else {
                None
            },
            turn_id: self.turn_id.clone(),
            related_step_id: None,
            available_sections: Vec::new(),
        }
    }
    async fn emit(
        &mut self,
        mut step: ClientTrajectoryStep,
        sections: Vec<(&str, Value)>,
        at: &str,
        facts: &mut impl FactSink,
    ) {
        let bytes = sections
            .iter()
            .map(|(_, value)| {
                serde_json::to_vec(value)
                    .map(|v| v.len())
                    .unwrap_or(usize::MAX / 2)
            })
            .sum::<usize>()
            + 4096;
        if bytes > MAX_STEP_FACT_BYTES {
            self.incomplete = true;
            return;
        }
        step.available_sections = sections
            .iter()
            .map(|(name, _)| (*name).into())
            .chain(["timing".into(), "raw".into()])
            .collect();
        let id = step.id;
        facts
            .push(Fact::Step {
                step: Box::new(step),
            })
            .await;
        for (name, value) in sections {
            facts
                .push(Fact::Section {
                    step_id: id,
                    section: name.into(),
                    value,
                })
                .await;
        }
        facts
            .push(Fact::Section {
                step_id: id,
                section: "timing".into(),
                value: json!({"observed_at":at}),
            })
            .await;
    }
}
pub(super) fn bounded_id(value: &str) -> Option<String> {
    if value.len() <= 1024 {
        Some(value.into())
    } else {
        None
    }
}
pub(super) fn tool_name(item: &Value) -> Option<String> {
    item["name"]
        .as_str()
        .or_else(|| item["function"]["name"].as_str())
        .and_then(bounded_id)
}
fn preview_text(value: &str) -> String {
    value
        .chars()
        .take(240)
        .map(|c| if c == '\0' { '�' } else { c })
        .collect()
}
fn preview(value: &Value) -> String {
    fn collect(value: &Value, out: &mut String, remaining: &mut usize, depth: usize) {
        if *remaining == 0 || depth > 8 {
            return;
        }
        match value {
            Value::String(text) => {
                if !out.is_empty() {
                    *remaining -= 1;
                    out.push(' ');
                }
                for ch in text.chars().take(*remaining) {
                    out.push(if ch == '\0' { '�' } else { ch });
                    *remaining -= 1;
                }
            }
            Value::Array(values) => {
                for value in values {
                    collect(value, out, remaining, depth + 1);
                    if *remaining == 0 {
                        break;
                    }
                }
            }
            Value::Object(map) => {
                let mut found = false;
                for key in ["text", "content", "summary", "output", "message", "name"] {
                    if let Some(value) = map.get(key) {
                        found = true;
                        collect(value, out, remaining, depth + 1);
                    }
                }
                if !found {
                    for (key, value) in map.iter().take(8) {
                        collect(&Value::String(key.clone()), out, remaining, depth + 1);
                        collect(value, out, remaining, depth + 1);
                    }
                }
            }
            Value::Number(number) => collect(
                &Value::String(number.to_string()),
                out,
                remaining,
                depth + 1,
            ),
            Value::Bool(boolean) => collect(
                &Value::String(boolean.to_string()),
                out,
                remaining,
                depth + 1,
            ),
            Value::Null => {}
        }
    }
    let mut out = String::new();
    collect(value, &mut out, &mut 240, 0);
    out
}
