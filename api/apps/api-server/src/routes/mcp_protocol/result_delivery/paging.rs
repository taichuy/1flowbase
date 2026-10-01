use serde_json::{json, Value};

pub(crate) const DEFAULT_INLINE_CHARS: usize = 4_000;
const PAGE_ENVELOPE_RESERVE_CHARS: usize = 768;

pub(crate) fn inline_limit(arguments: &Value) -> Result<usize, &'static str> {
    let Some(value) = arguments.get("max_inline_chars") else {
        return Ok(DEFAULT_INLINE_CHARS);
    };
    let Some(value) = value.as_u64().and_then(|value| usize::try_from(value).ok()) else {
        return Err("Invalid max_inline_chars");
    };
    if value == 0 {
        return Err("Invalid max_inline_chars");
    }
    Ok(value)
}

pub(super) fn bounded_page(
    leaves: &[JsonLeaf],
    cursor: ContinuationCursor,
    inline_chars: usize,
    mut envelope: Value,
) -> Value {
    envelope["entries"] = json!([]);
    let maximum_cursor = ContinuationCursor {
        leaf_index: leaves.len(),
        char_offset: leaves
            .iter()
            .filter_map(|leaf| leaf.value.as_str())
            .map(|value| value.chars().count())
            .max()
            .unwrap_or(0),
        selection_id: cursor.selection_id,
    }
    .encode();
    envelope["next_cursor"] = json!(maximum_cursor);
    if envelope.get("detail").is_some() {
        envelope["detail"]["next_cursor"] = envelope["next_cursor"].clone();
    }
    let envelope_chars = serialized(&envelope).chars().count();
    let available = inline_chars.saturating_sub(envelope_chars);
    let Some((entries, next_cursor)) = page_leaves(
        leaves,
        cursor,
        available.saturating_add(PAGE_ENVELOPE_RESERVE_CHARS),
    ) else {
        envelope["next_cursor"] = json!(cursor.encode());
        envelope["detail_status"] = json!("page_budget_too_small");
        return envelope;
    };
    envelope["entries"] = json!(entries);
    envelope["next_cursor"] = json!(next_cursor.map(ContinuationCursor::encode));
    if envelope.get("detail").is_some() {
        envelope["detail"]["next_cursor"] = envelope["next_cursor"].clone();
    }
    envelope
}

#[derive(Clone)]
pub(super) struct JsonLeaf {
    pub(super) path: String,
    pub(super) value: Value,
    pub(super) base_offset: usize,
    pub(super) total_chars: Option<usize>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct ContinuationCursor {
    pub(super) leaf_index: usize,
    pub(super) char_offset: usize,
    pub(super) selection_id: Option<u64>,
}

impl ContinuationCursor {
    pub(crate) fn parse(value: &str) -> Option<Self> {
        if let Some(encoded) = value.strip_prefix("v3:") {
            let mut parts = encoded.split(':');
            let selection_id = u64::from_str_radix(parts.next()?, 16).ok()?;
            let leaf_index = parts.next()?.parse().ok()?;
            let char_offset = parts.next()?.parse().ok()?;
            if parts.next().is_some() {
                return None;
            }
            return Some(Self {
                leaf_index,
                char_offset,
                selection_id: Some(selection_id),
            });
        }
        if let Some(encoded) = value.strip_prefix("v2:") {
            let (leaf_index, char_offset) = encoded.split_once(':')?;
            return Some(Self {
                leaf_index: leaf_index.parse().ok()?,
                char_offset: char_offset.parse().ok()?,
                selection_id: None,
            });
        }
        Some(Self {
            leaf_index: value.parse().ok()?,
            char_offset: 0,
            selection_id: None,
        })
    }

    pub(super) fn encode(self) -> String {
        if let Some(selection_id) = self.selection_id {
            return format!(
                "v3:{selection_id:016x}:{}:{}",
                self.leaf_index, self.char_offset
            );
        }
        if self.char_offset == 0 {
            self.leaf_index.to_string()
        } else {
            format!("v2:{}:{}", self.leaf_index, self.char_offset)
        }
    }

    pub(super) fn is_valid_for(self, leaves: &[JsonLeaf]) -> bool {
        if self.leaf_index == leaves.len() {
            return self.char_offset == 0;
        }
        let Some(leaf) = leaves.get(self.leaf_index) else {
            return false;
        };
        match &leaf.value {
            Value::String(value) => {
                self.char_offset == 0 || self.char_offset < value.chars().count()
            }
            _ => self.char_offset == 0,
        }
    }
}

pub(super) fn json_leaves(value: &Value) -> Vec<JsonLeaf> {
    let mut leaves = Vec::new();
    collect_json_leaves(value, String::new(), &mut leaves);
    leaves
}

fn collect_json_leaves(value: &Value, path: String, leaves: &mut Vec<JsonLeaf>) {
    match value {
        Value::Object(values) if !values.is_empty() => {
            let mut fields = values.iter().collect::<Vec<_>>();
            fields.sort_by_key(|(field, _)| *field);
            for (field, value) in fields {
                collect_json_leaves(
                    value,
                    format!("{path}/{}", escape_json_pointer(field)),
                    leaves,
                );
            }
        }
        Value::Array(values) if !values.is_empty() => {
            for (index, value) in values.iter().enumerate() {
                collect_json_leaves(value, format!("{path}/{index}"), leaves);
            }
        }
        _ => leaves.push(JsonLeaf {
            path,
            value: value.clone(),
            base_offset: 0,
            total_chars: None,
        }),
    }
}

fn escape_json_pointer(segment: &str) -> String {
    segment.replace('~', "~0").replace('/', "~1")
}

pub(super) fn page_leaves(
    leaves: &[JsonLeaf],
    cursor: ContinuationCursor,
    inline_chars: usize,
) -> Option<(Vec<Value>, Option<ContinuationCursor>)> {
    if cursor.leaf_index == leaves.len() {
        return Some((Vec::new(), None));
    }
    let entry_budget = inline_chars.saturating_sub(PAGE_ENVELOPE_RESERVE_CHARS);
    let mut entries = Vec::new();
    let mut used = 2;
    let mut next = cursor;
    while let Some(leaf) = leaves.get(next.leaf_index) {
        let entry = match (&leaf.value, leaf.total_chars) {
            (Value::String(value), Some(total_chars)) => {
                string_chunk_entry(leaf, value, 0, value.chars().count(), total_chars)
            }
            _ => json!({ "path": leaf.path, "value": leaf.value }),
        };
        let entry_chars = serialized(&entry).chars().count() + usize::from(!entries.is_empty());
        if next.char_offset == 0 && used + entry_chars <= entry_budget {
            used += entry_chars;
            entries.push(entry);
            next.leaf_index += 1;
            continue;
        }

        let largest_regular_entry = entry_budget.saturating_sub(2);
        if next.char_offset == 0 && serialized(&entry).chars().count() <= largest_regular_entry {
            break;
        }

        let Value::String(value) = &leaf.value else {
            break;
        };
        let separator_chars = usize::from(!entries.is_empty());
        let available = entry_budget.saturating_sub(used + separator_chars);
        let Some((chunk, chunk_chars)) =
            fitting_string_chunk(leaf, value, next.char_offset, available)
        else {
            break;
        };
        used += serialized(&chunk).chars().count() + separator_chars;
        entries.push(chunk);
        next.char_offset += chunk_chars;
        if next.char_offset == value.chars().count() {
            next.leaf_index += 1;
            next.char_offset = 0;
        }
    }
    if entries.is_empty() {
        return None;
    }
    Some((entries, (next.leaf_index < leaves.len()).then_some(next)))
}

fn fitting_string_chunk(
    leaf: &JsonLeaf,
    value: &str,
    char_offset: usize,
    available_chars: usize,
) -> Option<(Value, usize)> {
    let total_chars = value.chars().count();
    let remaining_chars = total_chars.checked_sub(char_offset)?;
    let mut low = 1;
    let mut high = remaining_chars;
    let mut best = None;
    while low <= high {
        let candidate_chars = low + (high - low) / 2;
        let candidate = string_chunk_entry(leaf, value, char_offset, candidate_chars, total_chars);
        if serialized(&candidate).chars().count() <= available_chars {
            best = Some((candidate, candidate_chars));
            low = candidate_chars + 1;
        } else {
            high = candidate_chars.saturating_sub(1);
        }
    }
    best
}

fn string_chunk_entry(
    leaf: &JsonLeaf,
    value: &str,
    char_offset: usize,
    char_count: usize,
    total_chars: usize,
) -> Value {
    let chunk = value
        .chars()
        .skip(char_offset)
        .take(char_count)
        .collect::<String>();
    json!({
        "path": leaf.path,
        "value_type": "string_chunk",
        "value": chunk,
        "char_offset": leaf.base_offset + char_offset,
        "char_count": char_count,
        "total_chars": leaf.total_chars.unwrap_or(total_chars),
        "next_offset": (leaf.base_offset + char_offset + char_count < leaf.total_chars.unwrap_or(total_chars))
            .then_some(leaf.base_offset + char_offset + char_count),
        "complete": leaf.base_offset + char_offset + char_count == leaf.total_chars.unwrap_or(total_chars)
    })
}

pub(super) fn serialized(value: &Value) -> String {
    serde_json::to_string(value).expect("serde_json::Value serialization must be infallible")
}

#[cfg(test)]
use super::ResultSelection;

#[cfg(test)]
#[path = "_tests/paging.rs"]
mod tests;
