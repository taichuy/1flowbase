//! Chunk boundaries for native SSE events, independent of the semantic delta channels.
use std::collections::BTreeMap;

use extension_contracts::provider_contract::ProviderStreamEvent;
use serde_json::Value;

use super::stream_restorer::{RestorationStack, StreamingTextRestorer};

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) enum NativeLane {
    Native(String),
    Responses,
}

impl NativeLane {
    fn wrap(&self, event: Value) -> ProviderStreamEvent {
        match self {
            Self::Native(protocol) => ProviderStreamEvent::NativeEvent {
                protocol: protocol.clone(),
                event,
            },
            Self::Responses => ProviderStreamEvent::ResponsesOutputDelta { event },
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum DeltaKind {
    Text,
    Refusal,
    Reasoning,
    ReasoningSummary,
    Arguments,
    CustomInput,
    BlockText,
    BlockThinking,
    BlockJson,
}

impl DeltaKind {
    fn responses(kind: &str) -> Option<Self> {
        Some(match kind {
            "response.output_text" => Self::Text,
            "response.refusal" => Self::Refusal,
            "response.reasoning_text" => Self::Reasoning,
            "response.reasoning_summary_text" => Self::ReasoningSummary,
            "response.function_call_arguments" => Self::Arguments,
            "response.custom_tool_call_input" => Self::CustomInput,
            _ => return None,
        })
    }

    fn delta(event: &Value) -> Option<Self> {
        let kind = event["type"].as_str()?;
        if kind == "content_block_delta" {
            return Some(match event["delta"]["type"].as_str()? {
                "text_delta" => Self::BlockText,
                "thinking_delta" => Self::BlockThinking,
                "input_json_delta" => Self::BlockJson,
                _ => return None,
            });
        }
        Self::responses(kind.strip_suffix(".delta")?)
    }

    fn path(self) -> &'static str {
        match self {
            Self::BlockText => "/delta/text",
            Self::BlockThinking => "/delta/thinking",
            Self::BlockJson => "/delta/partial_json",
            _ => "/delta",
        }
    }

    fn is_json(self) -> bool {
        matches!(self, Self::Arguments | Self::BlockJson)
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct Channel {
    lane: NativeLane,
    kind: DeltaKind,
    item_id: Option<String>,
    output_index: Option<u64>,
    part_index: Option<u64>,
}

fn part_index(event: &Value) -> Option<u64> {
    event["content_index"]
        .as_u64()
        .or_else(|| event["summary_index"].as_u64())
        .or_else(|| event["index"].as_u64())
}

impl Channel {
    fn ends_at(&self, lane: &NativeLane, event: &Value) -> bool {
        if &self.lane != lane {
            return false;
        }
        let Some(ty) = event["type"].as_str() else {
            return false;
        };
        if matches!(
            ty,
            "response.completed"
                | "response.incomplete"
                | "response.failed"
                | "message_stop"
                | "error"
        ) {
            return true;
        }
        let is_boundary = matches!(
            ty,
            "content_block_stop"
                | "response.output_item.done"
                | "response.content_part.done"
                | "response.reasoning_summary_part.done"
        );
        if !is_boundary
            && ty.strip_suffix(".done").and_then(DeltaKind::responses) != Some(self.kind)
        {
            return false;
        }
        let item = event["item_id"]
            .as_str()
            .or_else(|| event["item"]["id"].as_str());
        let output = event["output_index"].as_u64();
        let part = part_index(event);
        item.is_none_or(|id| {
            self.item_id
                .as_deref()
                .is_none_or(|candidate| candidate == id)
        }) && output.is_none_or(|index| self.output_index == Some(index))
            && part.is_none_or(|index| self.part_index == Some(index))
    }
}

struct PendingDelta {
    restorer: StreamingTextRestorer,
    template: Value,
}

#[derive(Default)]
pub(super) struct NativeDeltaChannels(BTreeMap<Channel, PendingDelta>);

impl NativeDeltaChannels {
    pub(super) fn process(
        &mut self,
        stack: &RestorationStack,
        lane: NativeLane,
        mut event: Value,
    ) -> Vec<ProviderStreamEvent> {
        if let Some(kind) = DeltaKind::delta(&event) {
            if let Some(fragment) = event.pointer(kind.path()).and_then(Value::as_str) {
                let channel = Channel {
                    lane: lane.clone(),
                    kind,
                    item_id: event["item_id"].as_str().map(str::to_owned),
                    output_index: event["output_index"].as_u64(),
                    part_index: part_index(&event),
                };
                let pending = self.0.entry(channel).or_insert_with(|| PendingDelta {
                    restorer: if kind.is_json() {
                        StreamingTextRestorer::for_json(stack.clone())
                    } else {
                        StreamingTextRestorer::new(stack.clone())
                    },
                    template: Value::Null,
                });
                let ready = pending.restorer.push(fragment);
                *event.pointer_mut(kind.path()).expect("known delta field") = Value::String(ready);
                pending.template = event.clone();
                // Preserve the provider's sequence metadata even for a temporarily empty delta.
                return vec![lane.wrap(event)];
            }
        }
        let mut out = Vec::new();
        self.0.retain(|key, pending| {
            if !key.ends_at(&lane, &event) {
                return true;
            }
            out.extend(flush_channel(key, pending));
            false
        });
        restore_wire_event(stack, &mut event);
        out.push(lane.wrap(event));
        out
    }

    pub(super) fn flush(&mut self) -> Vec<ProviderStreamEvent> {
        std::mem::take(&mut self.0)
            .into_iter()
            .filter_map(|(key, mut pending)| flush_channel(&key, &mut pending))
            .collect()
    }
}

fn flush_channel(key: &Channel, pending: &mut PendingDelta) -> Option<ProviderStreamEvent> {
    let tail = pending.restorer.flush();
    if tail.is_empty() {
        return None;
    }
    let mut event = std::mem::take(&mut pending.template);
    *event
        .pointer_mut(key.kind.path())
        .expect("known delta field") = Value::String(tail);
    Some(key.lane.wrap(event))
}

/// Wire arguments are JSON encoded in a string; structured argument objects
/// keep ordinary leaf replacement, as do text/refusal/reasoning bodies.
pub(super) fn restore_wire_event(stack: &RestorationStack, event: &mut Value) {
    match event {
        Value::Object(fields) => {
            let is_call = fields
                .get("type")
                .and_then(Value::as_str)
                .is_some_and(|kind| {
                    matches!(
                        kind,
                        "function_call" | "mcp_call" | "response.function_call_arguments.done"
                    )
                });
            for (key, value) in fields {
                if matches!(
                    key.as_str(),
                    "id" | "item_id"
                        | "response_id"
                        | "previous_response_id"
                        | "call_id"
                        | "type"
                        | "role"
                        | "encrypted_content"
                        | "signature"
                ) {
                    continue;
                }
                if is_call && key == "arguments" {
                    if let Value::String(encoded) = value {
                        *encoded = stack.restore_json_fragment(encoded);
                        continue;
                    }
                }
                if matches!(key.as_str(), "arguments" | "input" | "metadata")
                    || (key == "output" && !value.is_array())
                {
                    stack.restore_json(value);
                } else {
                    restore_wire_event(stack, value);
                }
            }
        }
        Value::Array(items) => items
            .iter_mut()
            .for_each(|item| restore_wire_event(stack, item)),
        _ => stack.restore_json(event),
    }
}
