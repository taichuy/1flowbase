use serde_json::Value;
use std::collections::BTreeMap;

fn identity_character(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '_' | '-')
}
/// Replace only complete known identity tokens, never substrings or unrelated UUID literals.
/// One pass prevents a new target identity from being rewritten a second time.
pub fn rewrite_template_text(text: &str, ids: &BTreeMap<String, String>) -> String {
    let mut result = String::with_capacity(text.len());
    let mut start = 0;
    while start < text.len() {
        let tail = &text[start..];
        let replacement = ids
            .iter()
            .filter(|(old, _)| !old.is_empty())
            .find(|(old, _)| {
                tail.starts_with(old.as_str())
                    && (start == 0
                        || !text[..start]
                            .chars()
                            .next_back()
                            .is_some_and(identity_character))
                    && !tail[old.len()..]
                        .chars()
                        .next()
                        .is_some_and(identity_character)
            });
        if let Some((old, new)) = replacement {
            result.push_str(new);
            start += old.len();
        } else {
            let Some(c) = tail.chars().next() else {
                break;
            };
            result.push(c);
            start += c.len_utf8();
        }
    }
    result
}
pub fn rewrite_template_value(value: &mut Value, ids: &BTreeMap<String, String>) {
    match value {
        Value::String(text) => *text = rewrite_template_text(text, ids),
        Value::Array(items) => items
            .iter_mut()
            .for_each(|v| rewrite_template_value(v, ids)),
        Value::Object(object) => {
            let old = std::mem::take(object);
            for (key, mut v) in old {
                rewrite_template_value(&mut v, ids);
                object.insert(rewrite_template_text(&key, ids), v);
            }
        }
        _ => {}
    }
}
