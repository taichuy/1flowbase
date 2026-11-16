//! Live progress is independent of durable callback admission. Only the latter
//! closes tool blocks, so a client cannot execute a speculative tool call.
use super::*;
use std::collections::BTreeMap;

#[derive(Clone, PartialEq, Eq)]
struct BlockIdentity {
    node_run_id: Uuid,
    round_id: Uuid,
    output_index: u64,
    item_id: String,
}

impl BlockIdentity {
    fn from_payload(payload: &Value) -> Option<Self> {
        Some(Self {
            node_run_id: payload["node_run_id"].as_str()?.parse().ok()?,
            round_id: payload["response_round_id"].as_str()?.parse().ok()?,
            output_index: payload["output_index"].as_u64()?,
            item_id: payload
                .get("item_id")
                .or_else(|| payload.get("item").and_then(|item| item.get("id")))?
                .as_str()?
                .to_owned(),
        })
    }
}

struct LiveTool {
    identity: BlockIdentity,
    name: String,
    index: u32,
    arguments: String,
    closed: bool,
}

#[derive(Default)]
pub(super) struct LiveBlocks {
    content: Option<BlockIdentity>,
    tools: BTreeMap<String, LiveTool>,
}

impl LiveBlocks {
    pub(super) fn clear_content(&mut self) {
        self.content = None;
    }
}

impl AnthropicStreamMapper {
    pub(super) fn live_block_events(
        &mut self,
        envelope: &RuntimeEventEnvelope,
    ) -> Option<Vec<Result<Event, Infallible>>> {
        match envelope.event_type.as_str() {
            "provider_content_block_start" => Some(self.start_live_block(&envelope.payload)),
            "provider_content_block_done" => {
                let identity = BlockIdentity::from_payload(&envelope.payload);
                Some(
                    if identity.is_some() && identity == self.live_blocks.content {
                        self.close_active_anthropic_content_block()
                    } else {
                        Vec::new()
                    },
                )
            }
            "provider_tool_call_delta" => Some(self.live_tool_delta(&envelope.payload)),
            _ => None,
        }
    }

    fn start_live_block(&mut self, payload: &Value) -> Vec<Result<Event, Infallible>> {
        let Some(identity) = BlockIdentity::from_payload(payload) else {
            return Vec::new();
        };
        let item = &payload["item"];
        let kind = match item["type"].as_str() {
            Some("reasoning") => AnthropicContentBlockKind::Thinking,
            Some("message") => AnthropicContentBlockKind::Text,
            Some("function_call") => return self.start_live_tool(payload, identity),
            // Other provider items retain their existing terminal projection.
            _ => return Vec::new(),
        };
        if self.live_blocks.content.as_ref() == Some(&identity) {
            return Vec::new();
        }
        let mut events = self.close_active_anthropic_content_block();
        events.extend(self.ensure_anthropic_content_block(kind));
        self.live_blocks.content = Some(identity);
        events
    }

    fn start_live_tool(
        &mut self,
        payload: &Value,
        identity: BlockIdentity,
    ) -> Vec<Result<Event, Infallible>> {
        let Some(callback_id) = payload["callback_task_id"]
            .as_str()
            .and_then(|id| Uuid::parse_str(id).ok())
        else {
            return Vec::new();
        };
        let item = &payload["item"];
        let Some((call_id, name)) = item["call_id"].as_str().zip(item["name"].as_str()) else {
            return Vec::new();
        };
        if call_id.is_empty() || name.is_empty() || identity.item_id.is_empty() {
            return Vec::new();
        }
        let id = encode_anthropic_callback_tool_use_id(callback_id, call_id);
        if let Some(tool) = self.live_blocks.tools.get(&id) {
            return if tool.identity == identity && tool.name == name {
                Vec::new()
            } else {
                self.live_block_projection_error()
            };
        }
        let mut events = self.close_active_anthropic_content_block();
        let index = self.next_content_index;
        self.next_content_index += 1;
        self.live_blocks.tools.insert(
            id.clone(),
            LiveTool {
                identity,
                name: name.to_owned(),
                index,
                arguments: String::new(),
                closed: false,
            },
        );
        events.push(event_json_sse(
            "content_block_start",
            json!({"type":"content_block_start","index":index,
                "content_block":{"type":"tool_use","id":id,"name":name,"input":{}}}),
        ));
        events
    }

    fn live_tool_delta(&mut self, payload: &Value) -> Vec<Result<Event, Infallible>> {
        let Some(callback_id) = payload["callback_task_id"]
            .as_str()
            .and_then(|id| Uuid::parse_str(id).ok())
        else {
            return Vec::new();
        };
        let Some((call_id, delta)) = payload["call_id"].as_str().zip(payload["delta"].as_str())
        else {
            return Vec::new();
        };
        let id = encode_anthropic_callback_tool_use_id(callback_id, call_id);
        let Some(tool) = self.live_blocks.tools.get_mut(&id) else {
            return Vec::new();
        };
        if BlockIdentity::from_payload(payload).as_ref() != Some(&tool.identity)
            || tool.closed
            || delta.is_empty()
        {
            return Vec::new();
        }
        tool.arguments.push_str(delta);
        vec![argument_delta(tool.index, delta)]
    }

    pub(super) fn live_tools_match_committed_blocks(&self, blocks: &[Value]) -> bool {
        self.live_blocks.tools.iter().all(|(id, tool)| {
            blocks.iter().any(|block| {
                block["id"].as_str() == Some(id.as_str())
                    && block["name"].as_str() == Some(tool.name.as_str())
            })
        })
    }

    pub(super) fn finish_live_tool(
        &mut self,
        block: &Value,
    ) -> Result<Option<Vec<Result<Event, Infallible>>>, ()> {
        let Some(tool) = block["id"]
            .as_str()
            .and_then(|id| self.live_blocks.tools.get_mut(id))
        else {
            return Ok(None);
        };
        if tool.closed {
            return Ok(Some(Vec::new()));
        }
        let input = block.get("input").cloned().unwrap_or_else(|| json!({}));
        let mut events = Vec::new();
        // Preserve the producer's whitespace/key order when a complete value
        // has already streamed. Otherwise only append the missing suffix.
        if serde_json::from_str::<Value>(&tool.arguments).ok().as_ref() != Some(&input) {
            let complete = serde_json::to_string(&input).map_err(|_| ())?;
            let suffix = complete.strip_prefix(&tool.arguments).ok_or(())?;
            if !suffix.is_empty() {
                events.push(argument_delta(tool.index, suffix));
            }
        }
        events.push(event_json_sse(
            "content_block_stop",
            json!({"type":"content_block_stop","index":tool.index}),
        ));
        tool.closed = true;
        Ok(Some(events))
    }

    pub(super) fn live_block_projection_error(&mut self) -> Vec<Result<Event, Infallible>> {
        self.stream_state = AnthropicStreamState::Terminal;
        vec![event_json_sse(
            "error",
            json!({"type":"error","error":{"type":"api_error",
                "message":"Streamed tool progress does not match the committed callback"}}),
        )]
    }
}

fn argument_delta(index: u32, delta: &str) -> Result<Event, Infallible> {
    event_json_sse(
        "content_block_delta",
        json!({"type":"content_block_delta","index":index,
            "delta":{"type":"input_json_delta","partial_json":delta}}),
    )
}
