//! Runtime owners reserve the root `env` namespace for frozen application secrets
//! (`freeze_run_input_environment` and `ready_answer_output_payload`). Select only
//! those owner slots; never scan arbitrary business JSON for secret-looking keys.
use serde_json::{json, Value};
use std::sync::Arc;

pub(crate) fn payload(mut value: Value) -> Value {
    if let Some(fields) = value.as_object_mut() {
        fields.remove("env");
    }
    value
}

pub(crate) fn payload_schema() -> Value {
    json!({"not":{"type":"object","required":["env"]}})
}

pub(crate) fn document<T: serde::Serialize>(value: &T) -> Result<Value, Arc<str>> {
    let mut value = crate::event_center::api_projection::to_value(value)?;
    project(&mut value);
    Ok(value)
}

/// Descend only through named runtime DTO containers. Payloads are leaves: a
/// user's nested object called `env` retains its ordinary business meaning.
pub(crate) fn project(value: &mut Value) {
    if let Some(items) = value.as_array_mut() {
        for item in items {
            project(item);
        }
    } else if let Some(fields) = value.as_object_mut() {
        for (key, value) in fields {
            match key.as_str() {
                "input_payload" | "output_payload" | "variable_snapshot" | "payload" => {
                    if let Some(payload) = value.as_object_mut() {
                        payload.remove("env");
                    }
                }
                "flow_run" | "source_flow_run" | "flow_run_fact" | "detail" | "node_runs"
                | "checkpoints" | "answer_snapshot" | "stitched_trace" | "entries"
                | "trace_tree" | "nodes" | "children" => project(value),
                "artifacts" => {
                    if let Some(artifacts) = value.as_array_mut() {
                        for artifact in artifacts {
                            if let Some(fields) =
                                artifact.get_mut("value").and_then(Value::as_object_mut)
                            {
                                fields.remove("env");
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }
}

pub(crate) fn schema<T: utoipa::ToSchema>() -> Value {
    let mut schema = crate::event_center::api_projection::schema::<T>();
    restrict_schema(&mut schema);
    schema
}

pub(crate) fn request_schema(
    operation: &crate::event_center::api_projection::ApiEventOperationContext,
    body: Value,
    query: Option<Value>,
) -> Value {
    let mut path = serde_json::Map::new();
    for parameter in operation.path.split('/') {
        if let Some(name) = parameter.strip_prefix(':').or_else(|| {
            parameter
                .strip_prefix('{')
                .and_then(|value| value.strip_suffix('}'))
        }) {
            // These debug command path contracts use UUID identifiers except
            // the editor's string node ID. The source is the compiled template.
            path.insert(
                name.into(),
                if name == "node_id" {
                    json!({"type":"string"})
                } else {
                    json!({"type":"string","format":"uuid"})
                },
            );
        }
    }
    let mut properties = serde_json::Map::new();
    let mut required = vec![json!("body")];
    if !path.is_empty() {
        required.push(json!("path"));
        properties.insert("path".into(),json!({"type":"object","required":path.keys().collect::<Vec<_>>(),"properties":path,"additionalProperties":false}));
    }
    if let Some(query) = query {
        properties.insert("query".into(), query);
    }
    properties.insert("body".into(), body);
    json!({"type":"object","properties":properties,"required":required,"additionalProperties":false})
}

fn restrict_schema(schema: &mut Value) {
    match schema {
        Value::Array(items) => {
            for item in items {
                restrict_schema(item);
            }
        }
        Value::Object(fields) => {
            if let Some(properties) = fields.get_mut("properties").and_then(Value::as_object_mut) {
                for (key, value) in properties {
                    if matches!(
                        key.as_str(),
                        "input_payload" | "output_payload" | "variable_snapshot" | "payload"
                    ) {
                        *value = json!({"allOf":[value.clone(),payload_schema()]});
                    } else if key == "artifacts" {
                        if let Some(payload) = value.pointer_mut("/items/properties/value") {
                            *payload = json!({"allOf":[payload.clone(),payload_schema()]});
                        }
                        restrict_schema(value);
                    } else {
                        restrict_schema(value);
                    }
                }
            }
            for (key, value) in fields {
                if key != "properties" {
                    restrict_schema(value);
                }
            }
        }
        _ => {}
    }
}
