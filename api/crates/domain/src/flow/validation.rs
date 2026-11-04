use serde_json::{json, Value};

/// Structural constraints apply to drafts even when their execution configuration is incomplete.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlowDocumentValidationError {
    pub node_id: Option<String>,
    pub node_type: String,
    pub field_path: String,
    pub message: String,
    pub expected: Option<Value>,
}

impl std::fmt::Display for FlowDocumentValidationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for FlowDocumentValidationError {}

fn derives_outputs_from_input_fields(node_type: &str) -> bool {
    matches!(node_type, "start" | "workflow_start")
}

/// A node-document schema fragment, projected by node discovery alongside its field catalog.
pub fn flow_node_document_schema(node_type: &str) -> Option<Value> {
    derives_outputs_from_input_fields(node_type).then(|| {
        json!({
            "type": "object",
            "required": ["outputs"],
            "properties": {"outputs": {"type": "array", "maxItems": 0}}
        })
    })
}

pub fn validate_flow_node_structure(node: &Value) -> Result<(), FlowDocumentValidationError> {
    let node_type = node.get("type").and_then(Value::as_str).unwrap_or_default();
    if derives_outputs_from_input_fields(node_type)
        && !node
            .get("outputs")
            .and_then(Value::as_array)
            .is_some_and(Vec::is_empty)
    {
        return Err(FlowDocumentValidationError {
            node_id: node.get("id").and_then(Value::as_str).map(str::to_owned),
            node_type: node_type.to_owned(),
            field_path: "/outputs".into(),
            message: format!(
                "{} node {} outputs must be empty",
                node_type,
                node.get("id")
                    .and_then(Value::as_str)
                    .unwrap_or("(unnamed)")
            ),
            expected: Some(json!([])),
        });
    }
    Ok(())
}

pub fn validate_flow_draft_structure(document: &Value) -> Result<(), FlowDocumentValidationError> {
    let nodes = document
        .pointer("/graph/nodes")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid(None, "/graph/nodes", "graph.nodes must be an array"))?;
    let edges = document
        .pointer("/graph/edges")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid(None, "/graph/edges", "graph.edges must be an array"))?;
    if document.get("schemaVersion").and_then(Value::as_str) != Some(super::FLOW_SCHEMA_VERSION) {
        return Err(invalid(
            None,
            "/schemaVersion",
            "unsupported or missing flow schemaVersion",
        ));
    }
    let mut node_ids = std::collections::BTreeSet::new();
    for node in nodes {
        validate_flow_node_structure(node)?;
        for key in ["id", "type"] {
            if !node
                .get(key)
                .and_then(Value::as_str)
                .is_some_and(|s| !s.trim().is_empty())
            {
                return Err(invalid(
                    Some(node),
                    &format!("/{key}"),
                    "node id and type must be nonempty strings",
                ));
            }
        }
        if !node_ids.insert(node["id"].as_str().unwrap()) {
            return Err(invalid(Some(node), "/id", "node ids must be unique"));
        }
        for key in ["config", "bindings"] {
            if !node.get(key).is_some_and(Value::is_object) {
                return Err(invalid(
                    Some(node),
                    &format!("/{key}"),
                    "node config and bindings must be objects",
                ));
            }
        }
        let outputs = node
            .get("outputs")
            .and_then(Value::as_array)
            .ok_or_else(|| invalid(Some(node), "/outputs", "node outputs must be an array"))?;
        let mut keys = std::collections::BTreeSet::new();
        for (index, output) in outputs.iter().enumerate() {
            let path = format!("/outputs/{index}");
            for field in ["key", "title", "valueType"] {
                if !output
                    .get(field)
                    .and_then(Value::as_str)
                    .is_some_and(|s| !s.trim().is_empty())
                {
                    return Err(invalid(
                        Some(node),
                        &format!("{path}/{field}"),
                        "output key, title and valueType must be nonempty strings",
                    ));
                }
            }
            let key = output["key"].as_str().unwrap();
            if key.starts_with("__") || !keys.insert(key) {
                return Err(invalid(
                    Some(node),
                    &format!("{path}/key"),
                    "output keys must be unique and cannot use internal __ names",
                ));
            }
            if let Some(selector) = output.get("selector") {
                if !selector.as_array().is_some_and(|segments| {
                    !segments.is_empty()
                        && segments
                            .iter()
                            .all(|s| s.as_str().is_some_and(|s| !s.trim().is_empty()))
                }) {
                    return Err(invalid(
                        Some(node),
                        &format!("{path}/selector"),
                        "an explicit output selector must be a nonempty array of nonempty strings",
                    ));
                }
            }
            if output
                .get("jsonSchema")
                .is_some_and(|schema| !schema.is_object())
            {
                return Err(invalid(
                    Some(node),
                    &format!("{path}/jsonSchema"),
                    "output jsonSchema must be an object",
                ));
            }
        }
    }
    let mut edge_ids = std::collections::BTreeSet::new();
    for edge in edges {
        for key in ["id", "source", "target"] {
            if !edge
                .get(key)
                .and_then(Value::as_str)
                .is_some_and(|s| !s.trim().is_empty())
            {
                return Err(invalid(
                    None,
                    "/graph/edges",
                    "edge id, source and target must be nonempty strings",
                ));
            }
        }
        if !edge_ids.insert(edge["id"].as_str().unwrap()) {
            return Err(invalid(None, "/graph/edges", "edge ids must be unique"));
        }
    }
    Ok(())
}

fn invalid(node: Option<&Value>, path: &str, message: &str) -> FlowDocumentValidationError {
    FlowDocumentValidationError {
        node_id: node
            .and_then(|node| node.get("id"))
            .and_then(Value::as_str)
            .map(str::to_owned),
        node_type: node
            .and_then(|node| node.get("type"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .into(),
        field_path: path.into(),
        message: message.into(),
        expected: None,
    }
}
