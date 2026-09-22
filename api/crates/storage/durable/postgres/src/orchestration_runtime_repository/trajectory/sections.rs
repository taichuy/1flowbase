use super::*;
use control_plane_contracts::ports::WorkflowEventSection;

/// Presentation categories are projected by the backend from the observed snapshot.
/// They do not rerun translation or fill missing execution data from client history.
pub(super) fn native_sections(
    metadata: &Value,
    items: &[ProviderTrajectoryEvidence],
) -> Vec<WorkflowEventSection> {
    let Some(body) = items
        .first()
        .and_then(|item| serde_json::from_str::<Value>(&item.body).ok())
    else {
        return vec![];
    };
    let mut sections = Vec::new();
    let mut add = |kind: &str, value: Value| {
        if !value.is_null() {
            sections.push(WorkflowEventSection {
                kind: kind.into(),
                value,
            });
        }
    };
    match metadata["kind"].as_str() {
        Some("model_call") => {
            let wire = &body["native_request"]["wire_body"];
            let mut configuration = body.clone();
            if let Some(map) = configuration.as_object_mut() {
                for name in ["system", "messages", "tools", "native_request"] {
                    map.remove(name);
                }
                if let Some(wire) = wire.as_object() {
                    let mut parameters = wire.clone();
                    for name in ["instructions", "input", "tools"] {
                        parameters.remove(name);
                    }
                    map.insert("native_request".into(), json!({"protocol":body["native_request"]["protocol"],"wire_body":parameters}));
                }
            }
            add("configuration", configuration);
            let mut system = serde_json::Map::new();
            if let Some(value) = body.get("system") {
                system.insert("system".into(), value.clone());
            }
            if let Some(value) = wire.get("instructions") {
                system.insert("instructions".into(), value.clone());
            }
            if let Some(messages) = body.get("messages").and_then(Value::as_array) {
                let values: Vec<_> = messages
                    .iter()
                    .filter(|message| {
                        matches!(message["role"].as_str(), Some("system" | "developer"))
                    })
                    .cloned()
                    .collect();
                if !values.is_empty() {
                    system.insert("messages".into(), json!(values));
                }
            }
            if let Some(messages) = wire.get("input").and_then(Value::as_array) {
                let values: Vec<_> = messages
                    .iter()
                    .filter(|message| {
                        matches!(message["role"].as_str(), Some("system" | "developer"))
                    })
                    .cloned()
                    .collect();
                if !values.is_empty() {
                    system.insert(
                        "native_request".into(),
                        json!({"wire_body":{"input":values}}),
                    );
                }
            }
            if !system.is_empty() {
                add("system", Value::Object(system));
            }
            let mut context = serde_json::Map::new();
            if let Some(value) = body.get("messages") {
                context.insert("messages".into(), value.clone());
            }
            if let Some(value) = wire.get("input") {
                context.insert("input".into(), value.clone());
            }
            if !context.is_empty() {
                add("context", Value::Object(context));
            }
            let mut tools = serde_json::Map::new();
            if let Some(value) = body.get("tools") {
                tools.insert("tools".into(), value.clone());
            }
            if let Some(value) = wire.get("tools") {
                tools.insert(
                    "native_request".into(),
                    json!({"wire_body":{"tools":value}}),
                );
            }
            if !tools.is_empty() {
                add("tools", Value::Object(tools));
            }
        }
        Some("error" | "observation_gap") => add("error", body),
        Some("tool_call") => add("tools", body),
        Some("tool_result" | "model_reply") => add("output", body),
        _ => add("configuration", body),
    }
    sections
}
