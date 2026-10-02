//! Chat wire semantics; raw framing and capture lifetime remain transport owned.
use super::{Classifier, FactSink};
use crate::ports::{ClientTrajectoryFact as Fact, ClientTrajectoryFrameKind};
use serde_json::{json, Value};

impl Classifier {
    pub(super) async fn chat_message(
        &mut self,
        message: Value,
        origin: &str,
        at: &str,
        facts: &mut impl FactSink,
    ) {
        if let Some(reasoning) = message
            .get("reasoning_content")
            .or_else(|| message.get("reasoning"))
        {
            if !reasoning.is_null() {
                self.item(
                    json!({"type":"reasoning","summary":reasoning}),
                    origin,
                    at,
                    facts,
                )
                .await;
            }
        }
        // Preserve the original message overview, including structured content.
        if !message["content"].is_null() || message["tool_calls"].is_null() {
            self.item(message.clone(), origin, at, facts).await;
        }
        if let Some(calls) = message["tool_calls"].as_array() {
            for call in calls {
                self.item(json!({"type":"function_call","id":call["id"],"call_id":call["id"],"name":call["function"]["name"],"arguments":call["function"]["arguments"]}), origin, at, facts).await;
            }
        }
    }

    pub(super) async fn chat_response(
        &mut self,
        kind: ClientTrajectoryFrameKind,
        value: Value,
        at: &str,
        facts: &mut impl FactSink,
    ) {
        if value.get("__client_sse_done").is_some() {
            // A transport terminator does not repair unfinished choice deltas.
            self.incomplete |= !self.chat_choices.is_empty();
            self.completed = true;
            return;
        }
        if let Some(id) = value["id"].as_str() {
            if self.response_id.as_deref() != Some(id) {
                self.response_id = Some(id.into());
                facts
                    .push(Fact::ResponseLink {
                        response_id: id.into(),
                    })
                    .await;
            }
        }
        let Some(choices) = value["choices"].as_array() else {
            self.incomplete = true;
            return;
        };
        for choice in choices {
            let Some(index) = choice["index"].as_u64() else {
                self.incomplete = true;
                continue;
            };
            if self.chat_finished.contains(&index) {
                continue;
            }
            if let Some(message) = choice.get("message") {
                self.chat_message(message.clone(), "emitted", at, facts)
                    .await;
                self.chat_finished.insert(index);
            } else if let Some(delta) = choice.get("delta").filter(|v| v.is_object()) {
                let message = self
                    .chat_choices
                    .entry(index)
                    .or_insert_with(|| json!({"role":"assistant"}));
                for field in ["content", "reasoning_content", "reasoning", "refusal"] {
                    if let Some(text) = delta[field].as_str() {
                        append(message, field, text);
                    }
                }
                if let Some(role) = delta["role"].as_str() {
                    message["role"] = Value::String(role.into());
                }
                if let Some(calls) = delta["tool_calls"].as_array() {
                    for call in calls {
                        let Some(call_index) = call["index"].as_u64() else {
                            self.incomplete = true;
                            continue;
                        };
                        let key = call_index.to_string();
                        if message.get("tool_fragments").is_none() {
                            message["tool_fragments"] = json!({});
                        }
                        let fragments = message["tool_fragments"].as_object_mut().unwrap();
                        let accumulated = fragments
                            .entry(key)
                            .or_insert_with(|| json!({"function":{}}));
                        for field in ["id", "type"] {
                            if let Some(text) = call[field].as_str() {
                                append(accumulated, field, text);
                            }
                        }
                        for field in ["name", "arguments"] {
                            if let Some(text) = call["function"][field].as_str() {
                                append(&mut accumulated["function"], field, text);
                            }
                        }
                    }
                }
                if !choice["finish_reason"].is_null() {
                    let mut message = self.chat_choices.remove(&index).unwrap();
                    if let Some(Value::Object(calls)) =
                        message.as_object_mut().unwrap().remove("tool_fragments")
                    {
                        let mut calls: Vec<_> = calls.into_iter().collect();
                        calls.sort_by_key(|(index, _)| index.parse::<u64>().unwrap_or_default());
                        message["tool_calls"] =
                            Value::Array(calls.into_iter().map(|(_, call)| call).collect());
                    }
                    self.chat_message(message, "emitted", at, facts).await;
                    self.chat_finished.insert(index);
                }
            } else {
                self.incomplete = true;
            }
        }
        if !value["usage"].is_null() && !self.usage_seen {
            self.usage_seen = true;
            let step = self.step(
                uuid::Uuid::now_v7(),
                "usage",
                "Usage",
                "emitted",
                at,
                &value["usage"],
            );
            self.emit(step, vec![("usage", value["usage"].clone())], at, facts)
                .await;
        }
        if kind == ClientTrajectoryFrameKind::ResponseJson {
            self.completed = true;
        }
    }
}
fn append(value: &mut Value, field: &str, text: &str) {
    if value.get(field).is_none() {
        value[field] = Value::String(String::new());
    }
    if let Some(current) = value[field].as_str() {
        value[field] = Value::String(format!("{current}{text}"));
    }
}
