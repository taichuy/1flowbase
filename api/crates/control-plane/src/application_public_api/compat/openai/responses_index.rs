use std::collections::BTreeMap;

use serde_json::Value;

pub(crate) const MAX_NATIVE_RESPONSES_INPUT_ITEMS: usize = 4_096;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ResponsesInputIndexError {
    InputKind,
    TooManyItems,
    ItemKind,
    ItemType,
}

impl ResponsesInputIndexError {
    pub(crate) const fn message(self) -> &'static str {
        match self {
            Self::InputKind => "input must be text or an array",
            Self::TooManyItems => "Responses input has too many items",
            Self::ItemKind => "input items must be objects",
            Self::ItemType => "input item type must be text",
        }
    }
}

/// Bounded, content-free index over an opaque Responses input.
///
/// The raw body remains owned by `ProviderTransportPayload`. This index stores
/// only positions and identity fields required by the protocol adapter, so AI
/// Native does not need an exhaustive Responses item enum.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ResponsesInputIndex {
    item_count: usize,
    item_types: Vec<Option<String>>,
    user_message_positions: Vec<usize>,
    tool_output_positions: Vec<usize>,
    outputs_by_call_id: BTreeMap<String, Vec<usize>>,
    last_tool_call_position: Option<usize>,
}

impl ResponsesInputIndex {
    pub(crate) fn build(input: &Value) -> Result<Self, ResponsesInputIndexError> {
        if input.is_string() {
            return Ok(Self {
                item_count: 0,
                item_types: Vec::new(),
                user_message_positions: Vec::new(),
                tool_output_positions: Vec::new(),
                outputs_by_call_id: BTreeMap::new(),
                last_tool_call_position: None,
            });
        }
        let items = input
            .as_array()
            .ok_or(ResponsesInputIndexError::InputKind)?;
        if items.len() > MAX_NATIVE_RESPONSES_INPUT_ITEMS {
            return Err(ResponsesInputIndexError::TooManyItems);
        }

        let mut index = Self {
            item_count: items.len(),
            item_types: Vec::with_capacity(items.len()),
            user_message_positions: Vec::new(),
            tool_output_positions: Vec::new(),
            outputs_by_call_id: BTreeMap::new(),
            last_tool_call_position: None,
        };
        for (position, item) in items.iter().enumerate() {
            let object = item.as_object().ok_or(ResponsesInputIndexError::ItemKind)?;
            let item_type = match object.get("type") {
                Some(Value::String(value)) => Some(value.clone()),
                Some(_) => return Err(ResponsesInputIndexError::ItemType),
                None => None,
            };
            if object.get("role").and_then(Value::as_str) == Some("user") {
                index.user_message_positions.push(position);
            }
            match item_type.as_deref() {
                Some("function_call" | "custom_tool_call") => {
                    index.last_tool_call_position = Some(position);
                }
                Some("function_call_output" | "custom_tool_call_output") => {
                    index.tool_output_positions.push(position);
                    if let Some(call_id) = object
                        .get("call_id")
                        .and_then(Value::as_str)
                        .filter(|value| !value.is_empty())
                    {
                        index
                            .outputs_by_call_id
                            .entry(call_id.to_owned())
                            .or_default()
                            .push(position);
                    }
                }
                _ => {}
            }
            index.item_types.push(item_type);
        }
        Ok(index)
    }

    pub(crate) const fn item_count(&self) -> usize {
        self.item_count
    }

    pub(crate) fn item_types(&self) -> &[Option<String>] {
        &self.item_types
    }

    pub(crate) fn outputs_by_call_id(&self) -> &BTreeMap<String, Vec<usize>> {
        &self.outputs_by_call_id
    }

    /// Selects the latest tool-call segment while allowing opaque context items
    /// after its outputs. A later user message starts a new turn and suppresses
    /// callback admission.
    pub(crate) fn current_tool_output_positions(&self) -> Vec<usize> {
        let start = self
            .last_tool_call_position
            .map_or(0, |position| position + 1);
        let positions = self
            .tool_output_positions
            .iter()
            .copied()
            .filter(|position| *position >= start)
            .collect::<Vec<_>>();
        let Some(last_output) = positions.last().copied() else {
            return positions;
        };
        if self
            .user_message_positions
            .iter()
            .any(|position| *position > last_output)
        {
            return Vec::new();
        }
        positions
    }
}
