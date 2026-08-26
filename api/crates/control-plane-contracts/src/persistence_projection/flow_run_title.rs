use serde_json::Value;

pub const FLOW_RUN_TITLE_MAX_CHARS: usize = 255;

const UNTITLED_FLOW_RUN: &str = "Untitled run";

pub fn build_flow_run_title(explicit_title: Option<&str>, fallback_text: &str) -> String {
    normalize_title(explicit_title).unwrap_or_else(|| normalize_or_default(fallback_text))
}

pub fn derive_flow_run_title_from_input_payload(input_payload: &Value) -> Option<String> {
    find_query_text(input_payload).and_then(|value| normalize_title(Some(value)))
}

pub fn display_flow_run_title(stored_title: &str, input_payload: &Value) -> String {
    normalize_title(Some(stored_title))
        .or_else(|| derive_flow_run_title_from_input_payload(input_payload))
        .unwrap_or_else(|| UNTITLED_FLOW_RUN.to_string())
}

fn normalize_or_default(value: &str) -> String {
    normalize_title(Some(value)).unwrap_or_else(|| UNTITLED_FLOW_RUN.to_string())
}

fn normalize_title(value: Option<&str>) -> Option<String> {
    let trimmed = value?.trim();
    if trimmed.is_empty() {
        return None;
    }

    Some(trimmed.chars().take(FLOW_RUN_TITLE_MAX_CHARS).collect())
}

fn find_query_text(value: &Value) -> Option<&str> {
    match value {
        Value::Object(object) => {
            if let Some(query) = object.get("query").and_then(Value::as_str) {
                return Some(query);
            }

            for key in ["node-start", "start", "input", "inputs"] {
                if let Some(query) = object.get(key).and_then(find_query_text) {
                    return Some(query);
                }
            }

            object.values().find_map(find_query_text)
        }
        Value::Array(items) => items.iter().find_map(find_query_text),
        _ => None,
    }
}
