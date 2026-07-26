use plugin_framework::provider_contract::{
    ProviderFinishReason, ProviderRuntimeError, ProviderUsage,
};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CanonicalItemId(String);

impl CanonicalItemId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for CanonicalItemId {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for CanonicalItemId {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CanonicalBlockId {
    item_id: CanonicalItemId,
    value: String,
}

impl CanonicalBlockId {
    pub fn new(item_id: CanonicalItemId, value: impl Into<String>) -> Self {
        Self {
            item_id,
            value: value.into(),
        }
    }

    pub fn item_id(&self) -> &CanonicalItemId {
        &self.item_id
    }

    pub fn as_str(&self) -> &str {
        &self.value
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CanonicalCallId {
    item_id: CanonicalItemId,
    value: String,
}

impl CanonicalCallId {
    pub fn new(item_id: CanonicalItemId, value: impl Into<String>) -> Self {
        Self {
            item_id,
            value: value.into(),
        }
    }

    pub fn item_id(&self) -> &CanonicalItemId {
        &self.item_id
    }

    pub fn as_str(&self) -> &str {
        &self.value
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SegmentedString {
    segments: Vec<String>,
    materialized: String,
}

impl SegmentedString {
    pub fn append(&mut self, segment: impl Into<String>) {
        let segment = segment.into();
        self.materialized.push_str(&segment);
        self.segments.push(segment);
    }

    pub fn segments(&self) -> &[String] {
        &self.segments
    }

    pub fn as_str(&self) -> &str {
        &self.materialized
    }

    pub fn is_empty(&self) -> bool {
        self.materialized.is_empty()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CanonicalContentKind {
    Text,
    Reasoning,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonicalContentBlock {
    id: CanonicalBlockId,
    kind: CanonicalContentKind,
    content: SegmentedString,
}

impl CanonicalContentBlock {
    pub fn id(&self) -> &CanonicalBlockId {
        &self.id
    }

    pub fn kind(&self) -> CanonicalContentKind {
        self.kind
    }

    pub fn content(&self) -> &SegmentedString {
        &self.content
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonicalToolCall {
    id: CanonicalCallId,
    arguments: SegmentedString,
}

impl CanonicalToolCall {
    pub fn id(&self) -> &CanonicalCallId {
        &self.id
    }

    pub fn arguments(&self) -> &SegmentedString {
        &self.arguments
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonicalItem {
    id: CanonicalItemId,
    blocks: Vec<CanonicalContentBlock>,
    tool_calls: Vec<CanonicalToolCall>,
}

impl CanonicalItem {
    pub fn id(&self) -> &CanonicalItemId {
        &self.id
    }

    pub fn blocks(&self) -> &[CanonicalContentBlock] {
        &self.blocks
    }

    pub fn tool_calls(&self) -> &[CanonicalToolCall] {
        &self.tool_calls
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum CanonicalStreamEvent {
    TextDelta {
        block_id: CanonicalBlockId,
        delta: String,
    },
    ReasoningDelta {
        block_id: CanonicalBlockId,
        delta: String,
    },
    ToolArgumentsDelta {
        call_id: CanonicalCallId,
        delta: String,
    },
    UsageDelta {
        usage: ProviderUsage,
    },
    UsageSnapshot {
        usage: ProviderUsage,
    },
    Finish {
        reason: ProviderFinishReason,
    },
    Fail {
        error: ProviderRuntimeError,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum CanonicalTerminal {
    Finished { reason: ProviderFinishReason },
    Failed { error: ProviderRuntimeError },
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CanonicalUsage {
    value: ProviderUsage,
}

impl CanonicalUsage {
    pub fn value(&self) -> &ProviderUsage {
        &self.value
    }

    fn add_delta(&mut self, delta: ProviderUsage) -> Result<(), CanonicalStreamTransitionError> {
        let mut merged = self.value.clone();
        add_usage_field(&mut merged.input_tokens, delta.input_tokens, "input_tokens")?;
        add_usage_field(
            &mut merged.input_cache_hit_tokens,
            delta.input_cache_hit_tokens,
            "input_cache_hit_tokens",
        )?;
        add_usage_field(
            &mut merged.input_cache_miss_tokens,
            delta.input_cache_miss_tokens,
            "input_cache_miss_tokens",
        )?;
        add_usage_field(
            &mut merged.output_tokens,
            delta.output_tokens,
            "output_tokens",
        )?;
        add_usage_field(
            &mut merged.reasoning_tokens,
            delta.reasoning_tokens,
            "reasoning_tokens",
        )?;
        add_usage_field(
            &mut merged.cache_read_tokens,
            delta.cache_read_tokens,
            "cache_read_tokens",
        )?;
        add_usage_field(
            &mut merged.cache_write_tokens,
            delta.cache_write_tokens,
            "cache_write_tokens",
        )?;
        add_usage_field(&mut merged.total_tokens, delta.total_tokens, "total_tokens")?;
        self.value = merged;
        Ok(())
    }

    fn merge_snapshot(&mut self, snapshot: ProviderUsage) {
        replace_present(&mut self.value.input_tokens, snapshot.input_tokens);
        replace_present(
            &mut self.value.input_cache_hit_tokens,
            snapshot.input_cache_hit_tokens,
        );
        replace_present(
            &mut self.value.input_cache_miss_tokens,
            snapshot.input_cache_miss_tokens,
        );
        replace_present(&mut self.value.output_tokens, snapshot.output_tokens);
        replace_present(&mut self.value.reasoning_tokens, snapshot.reasoning_tokens);
        replace_present(
            &mut self.value.cache_read_tokens,
            snapshot.cache_read_tokens,
        );
        replace_present(
            &mut self.value.cache_write_tokens,
            snapshot.cache_write_tokens,
        );
        replace_present(&mut self.value.total_tokens, snapshot.total_tokens);
    }
}

fn add_usage_field(
    current: &mut Option<u64>,
    delta: Option<u64>,
    field: &'static str,
) -> Result<(), CanonicalStreamTransitionError> {
    let Some(delta) = delta else {
        return Ok(());
    };
    let value = current
        .unwrap_or(0)
        .checked_add(delta)
        .ok_or(CanonicalStreamTransitionError::UsageOverflow { field })?;
    *current = Some(value);
    Ok(())
}

fn replace_present(current: &mut Option<u64>, snapshot: Option<u64>) {
    if snapshot.is_some() {
        *current = snapshot;
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CanonicalStreamAccumulator {
    items: Vec<CanonicalItem>,
    text: SegmentedString,
    reasoning: SegmentedString,
    usage: CanonicalUsage,
}

impl CanonicalStreamAccumulator {
    pub fn items(&self) -> &[CanonicalItem] {
        &self.items
    }

    pub fn text(&self) -> &SegmentedString {
        &self.text
    }

    pub fn reasoning(&self) -> &SegmentedString {
        &self.reasoning
    }

    pub fn usage(&self) -> &CanonicalUsage {
        &self.usage
    }

    pub fn block(&self, block_id: &CanonicalBlockId) -> Option<&CanonicalContentBlock> {
        self.items
            .iter()
            .find(|item| item.id == *block_id.item_id())
            .and_then(|item| item.blocks.iter().find(|block| block.id == *block_id))
    }

    pub fn tool_call(&self, call_id: &CanonicalCallId) -> Option<&CanonicalToolCall> {
        self.items
            .iter()
            .find(|item| item.id == *call_id.item_id())
            .and_then(|item| item.tool_calls.iter().find(|call| call.id == *call_id))
    }

    fn append_content(
        &mut self,
        block_id: CanonicalBlockId,
        kind: CanonicalContentKind,
        delta: String,
    ) -> Result<(), CanonicalStreamTransitionError> {
        let item = self.item_mut(block_id.item_id().clone());
        let block = match item.blocks.iter_mut().find(|block| block.id == block_id) {
            Some(block) if block.kind != kind => {
                return Err(CanonicalStreamTransitionError::ContentKindConflict {
                    block_id,
                    existing: block.kind,
                    incoming: kind,
                });
            }
            Some(block) => block,
            None => {
                item.blocks.push(CanonicalContentBlock {
                    id: block_id,
                    kind,
                    content: SegmentedString::default(),
                });
                let appended_index = item.blocks.len() - 1;
                &mut item.blocks[appended_index]
            }
        };
        block.content.append(delta.clone());
        match kind {
            CanonicalContentKind::Text => self.text.append(delta),
            CanonicalContentKind::Reasoning => self.reasoning.append(delta),
        }
        Ok(())
    }

    fn append_tool_arguments(&mut self, call_id: CanonicalCallId, delta: String) {
        let item = self.item_mut(call_id.item_id().clone());
        let call = match item.tool_calls.iter_mut().find(|call| call.id == call_id) {
            Some(call) => call,
            None => {
                item.tool_calls.push(CanonicalToolCall {
                    id: call_id,
                    arguments: SegmentedString::default(),
                });
                let appended_index = item.tool_calls.len() - 1;
                &mut item.tool_calls[appended_index]
            }
        };
        call.arguments.append(delta);
    }

    fn item_mut(&mut self, item_id: CanonicalItemId) -> &mut CanonicalItem {
        if let Some(index) = self.items.iter().position(|item| item.id == item_id) {
            return &mut self.items[index];
        }
        self.items.push(CanonicalItem {
            id: item_id,
            blocks: Vec::new(),
            tool_calls: Vec::new(),
        });
        let appended_index = self.items.len() - 1;
        &mut self.items[appended_index]
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CanonicalTerminalState {
    accumulated: CanonicalStreamAccumulator,
    terminal: CanonicalTerminal,
}

impl CanonicalTerminalState {
    pub fn accumulated(&self) -> &CanonicalStreamAccumulator {
        &self.accumulated
    }

    pub fn terminal(&self) -> &CanonicalTerminal {
        &self.terminal
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum CanonicalStreamState {
    Streaming(CanonicalStreamAccumulator),
    Terminal(CanonicalTerminalState),
}

impl Default for CanonicalStreamState {
    fn default() -> Self {
        Self::Streaming(CanonicalStreamAccumulator::default())
    }
}

impl CanonicalStreamState {
    pub fn accumulated(&self) -> &CanonicalStreamAccumulator {
        match self {
            Self::Streaming(accumulated) => accumulated,
            Self::Terminal(terminal) => terminal.accumulated(),
        }
    }

    pub fn terminal(&self) -> Option<&CanonicalTerminal> {
        match self {
            Self::Streaming(_) => None,
            Self::Terminal(terminal) => Some(terminal.terminal()),
        }
    }

    pub fn apply(
        &mut self,
        event: CanonicalStreamEvent,
    ) -> Result<(), CanonicalStreamTransitionError> {
        let Self::Streaming(accumulated) = self else {
            return Err(CanonicalStreamTransitionError::StreamAlreadyTerminal);
        };

        match event {
            CanonicalStreamEvent::TextDelta { block_id, delta } => {
                accumulated.append_content(block_id, CanonicalContentKind::Text, delta)
            }
            CanonicalStreamEvent::ReasoningDelta { block_id, delta } => {
                accumulated.append_content(block_id, CanonicalContentKind::Reasoning, delta)
            }
            CanonicalStreamEvent::ToolArgumentsDelta { call_id, delta } => {
                accumulated.append_tool_arguments(call_id, delta);
                Ok(())
            }
            CanonicalStreamEvent::UsageDelta { usage } => accumulated.usage.add_delta(usage),
            CanonicalStreamEvent::UsageSnapshot { usage } => {
                accumulated.usage.merge_snapshot(usage);
                Ok(())
            }
            CanonicalStreamEvent::Finish { reason } => {
                let accumulated = std::mem::take(accumulated);
                *self = Self::Terminal(CanonicalTerminalState {
                    accumulated,
                    terminal: CanonicalTerminal::Finished { reason },
                });
                Ok(())
            }
            CanonicalStreamEvent::Fail { error } => {
                let accumulated = std::mem::take(accumulated);
                *self = Self::Terminal(CanonicalTerminalState {
                    accumulated,
                    terminal: CanonicalTerminal::Failed { error },
                });
                Ok(())
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CanonicalStreamTransitionError {
    #[error("canonical stream is already terminal")]
    StreamAlreadyTerminal,
    #[error(
        "canonical block {block_id:?} already has kind {existing:?}, cannot append {incoming:?}"
    )]
    ContentKindConflict {
        block_id: CanonicalBlockId,
        existing: CanonicalContentKind,
        incoming: CanonicalContentKind,
    },
    #[error("canonical usage field {field} overflowed")]
    UsageOverflow { field: &'static str },
}

#[cfg(test)]
mod _tests;
