//! Ephemeral provider progress is separate from committed executable tool delivery.
use super::{ProviderOutputItemPhase, ProviderStreamEvent};
use crate::orchestration_runtime::debug_stream_events;
use crate::ports::RuntimeEventPayload;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

struct PublicTool {
    item_id: String,
    call_id: String,
}

pub(super) struct ClientProgress {
    callback_task_id: Uuid,
    public_names: BTreeSet<String>,
    tools: BTreeMap<usize, PublicTool>,
    aliases: BTreeMap<String, usize>,
    response_round_id: Option<Uuid>,
    source_active: bool,
}

impl ClientProgress {
    pub(super) fn new(
        public_names: BTreeSet<String>,
        response_round_id: Option<Uuid>,
        source_active: bool,
    ) -> Self {
        Self {
            // Reserve an identity, not admission. The final callback transaction
            // consumes this same UUID only if the matching client calls survive.
            callback_task_id: Uuid::now_v7(),
            public_names,
            tools: BTreeMap::new(),
            aliases: BTreeMap::new(),
            response_round_id,
            source_active,
        }
    }

    pub(super) fn observe(
        &mut self,
        node_id: &str,
        node_run_id: Uuid,
        event: &ProviderStreamEvent,
    ) -> Option<RuntimeEventPayload> {
        if !self.source_active {
            return None;
        }
        let (event_type, mut payload) = match event {
            ProviderStreamEvent::OutputItem {
                phase,
                output_index,
                item,
            } => {
                let kind = item.get("type").and_then(Value::as_str)?;
                if matches!(kind, "function_call" | "custom_tool_call") {
                    // Done stays in the original durable callback buffer. A
                    // complete JSON argument is not permission to execute it.
                    if *phase != ProviderOutputItemPhase::Added
                        || !item
                            .get("name")
                            .and_then(Value::as_str)
                            .is_some_and(|name| self.public_names.contains(name))
                    {
                        return None;
                    }
                    let item_id = item.get("id").and_then(Value::as_str)?.to_string();
                    let call_id = item.get("call_id").and_then(Value::as_str)?;
                    if item_id.is_empty() || call_id.is_empty() {
                        return None;
                    }
                    self.aliases.insert(item_id.clone(), *output_index);
                    self.aliases.insert(call_id.to_string(), *output_index);
                    self.tools.insert(
                        *output_index,
                        PublicTool {
                            item_id,
                            call_id: call_id.to_string(),
                        },
                    );
                    (
                        "provider_content_block_start",
                        json!({
                            "output_index": output_index,
                            "item": item,
                            "callback_task_id": self.callback_task_id,
                        }),
                    )
                } else if matches!(kind, "reasoning" | "message") {
                    (
                        match phase {
                            ProviderOutputItemPhase::Added => "provider_content_block_start",
                            ProviderOutputItemPhase::Done => "provider_content_block_done",
                        },
                        json!({"output_index":output_index,"item":{"id":item["id"],"type":kind}}),
                    )
                } else {
                    return None;
                }
            }
            ProviderStreamEvent::ToolCallDelta { call_id, delta } => {
                let index = self.aliases.get(call_id)?;
                let tool = self.tools.get(index)?;
                let delta = super::tool_argument_delta(delta)?;
                (
                    "provider_tool_call_delta",
                    json!({
                        "output_index": index,
                        "item_id": tool.item_id,
                        "call_id": tool.call_id,
                        "delta": delta,
                        "callback_task_id": self.callback_task_id,
                    }),
                )
            }
            // Responses suppliers also emit ToolCallDelta for these argument
            // fragments. Consume only that semantic lane to avoid duplication.
            _ => return None,
        };
        payload["response_round_id"] = json!(self.response_round_id);
        Some(debug_stream_events::provider_content_progress(
            event_type,
            node_id,
            node_run_id,
            payload,
        ))
    }

    pub(super) fn bind_delivery(&self, delivery: &mut RuntimeEventPayload) {
        let item = &delivery.payload["item"];
        let admitted = item
            .get("call_id")
            .and_then(Value::as_str)
            .and_then(|id| self.aliases.get(id))
            .and_then(|index| self.tools.get(index));
        if admitted.is_some() {
            // This value is authored only by the host. Persistence checks the
            // final node/call/name scope before consuming the reservation.
            delivery.payload["reserved_callback_task_id"] = json!(self.callback_task_id);
        }
    }
}

#[cfg(test)]
#[path = "_tests/mod.rs"]
mod tests;
