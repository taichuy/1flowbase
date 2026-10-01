use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use super::{json_leaves, JsonLeaf};

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub(crate) struct ResultSelection {
    pub(crate) response_fields: Option<Vec<String>>,
    pub(crate) string_ranges: BTreeMap<String, StringRange>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct StringRange {
    #[serde(default)]
    offset: usize,
    length: usize,
}

impl ResultSelection {
    pub(crate) fn parse(
        arguments: &Value,
        default_fields: Option<&[String]>,
    ) -> Result<Self, &'static str> {
        let response_fields = match arguments.get("response_fields") {
            Some(value) => Some(
                serde_json::from_value::<Vec<String>>(value.clone())
                    .map_err(|_| "Invalid response_fields")?,
            ),
            None => default_fields.map(<[String]>::to_vec),
        };
        domain::mcp_management::validate_mcp_return_defaults(None, response_fields.as_deref())
            .map_err(|_| "Invalid response_fields: expected JSON Pointer paths")?;
        let string_ranges = match arguments.get("string_ranges") {
            Some(value) => serde_json::from_value::<BTreeMap<String, StringRange>>(value.clone())
                .map_err(|_| "Invalid string_ranges")?,
            None => BTreeMap::new(),
        };
        for (pointer, range) in &string_ranges {
            if !domain::mcp_management::is_mcp_result_pointer(pointer) || range.length == 0 {
                return Err("Invalid string_ranges");
            }
            if response_fields
                .as_ref()
                .is_some_and(|fields| !fields.iter().any(|field| includes(field, pointer)))
            {
                return Err("String range is outside response_fields");
            }
        }
        Ok(Self {
            response_fields,
            string_ranges,
        })
    }

    pub(crate) fn is_default(&self) -> bool {
        self.response_fields.is_none() && self.string_ranges.is_empty()
    }

    pub(crate) fn identity(&self) -> Option<u64> {
        if self.is_default() {
            return None;
        }
        let digest = Sha256::digest(serde_json::to_vec(self).expect("selection serializes"));
        Some(u64::from_be_bytes(
            digest[..8].try_into().expect("eight digest bytes"),
        ))
    }

    pub(super) fn leaves(&self, detail: &Value) -> Result<Vec<JsonLeaf>, Value> {
        if let Some(fields) = &self.response_fields {
            for pointer in fields {
                if detail.pointer(pointer).is_none() {
                    return Err(json!({"reason": "field_not_found", "path": pointer}));
                }
            }
        }
        for (pointer, range) in &self.string_ranges {
            let Some(value) = detail.pointer(pointer).and_then(Value::as_str) else {
                return Err(json!({"reason": "string_field_not_found", "path": pointer}));
            };
            if range.offset > value.chars().count() {
                return Err(json!({"reason": "string_offset_out_of_bounds", "path": pointer}));
            }
        }
        let mut leaves = json_leaves(detail);
        if let Some(fields) = &self.response_fields {
            leaves.retain(|leaf| fields.iter().any(|pointer| includes(pointer, &leaf.path)));
        }
        for leaf in &mut leaves {
            if let Some(range) = self.string_ranges.get(&leaf.path) {
                let value = leaf.value.as_str().expect("range string was validated");
                leaf.total_chars = Some(value.chars().count());
                leaf.base_offset = range.offset;
                leaf.value = Value::String(
                    value
                        .chars()
                        .skip(range.offset)
                        .take(range.length)
                        .collect(),
                );
            }
        }
        Ok(leaves)
    }
}

fn includes(parent: &str, pointer: &str) -> bool {
    parent == pointer
        || pointer
            .strip_prefix(parent)
            .is_some_and(|suffix| suffix.starts_with('/'))
}

#[cfg(test)]
#[path = "_tests/selection.rs"]
mod tests;
