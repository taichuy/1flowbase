use serde_json::Value;
use std::collections::BTreeSet;

pub use control_plane_contracts::portable_template::{
    rewrite_template_text, rewrite_template_value,
};
fn identity_character(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '_' | '-')
}
pub(crate) fn string_values(value: &Value, out: &mut Vec<String>) {
    match value {
        Value::String(s) => out.push(s.clone()),
        Value::Array(a) => a.iter().for_each(|v| string_values(v, out)),
        Value::Object(o) => o.values().for_each(|v| string_values(v, out)),
        _ => {}
    }
}
pub(crate) fn route_references(text: &str, prefix: &str) -> BTreeSet<String> {
    text.match_indices(prefix)
        .filter_map(|(offset, _)| {
            let suffix = &text[offset + prefix.len()..];
            let segment: String = suffix
                .chars()
                .take_while(|c| identity_character(*c))
                .collect();
            (!segment.is_empty()).then_some(segment)
        })
        .collect()
}
pub(crate) fn contains_identity(text: &str, id: &str) -> bool {
    text.match_indices(id).any(|(start, _)| {
        (start == 0
            || !text[..start]
                .chars()
                .next_back()
                .is_some_and(identity_character))
            && !text[start + id.len()..]
                .chars()
                .next()
                .is_some_and(identity_character)
    })
}
