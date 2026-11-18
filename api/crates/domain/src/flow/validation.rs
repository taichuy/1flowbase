use serde_json::{json, Map, Value};

/// Structural constraints apply to drafts even when their execution configuration is incomplete.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlowDocumentValidationError {
    pub node_id: Option<String>,
    pub node_type: String,
    pub field_path: String,
    pub message: String,
    pub expected: Option<Box<Value>>,
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
            expected: Some(Box::new(json!([]))),
        });
    }
    Ok(())
}

/// Borrowed structural facts produced in the same order as draft validation.
/// Private fields keep consumers from constructing an unvalidated view.
#[derive(Debug)]
pub struct ValidatedFlowDraft<'a> {
    nodes: Vec<ValidatedFlowNode<'a>>,
    edges: Vec<ValidatedFlowEdge<'a>>,
}

#[derive(Debug)]
pub struct ValidatedFlowNode<'a> {
    value: &'a Value,
    id: &'a str,
    node_type: &'a str,
    outputs: &'a [Value],
    bindings: &'a Map<String, Value>,
}

#[derive(Debug)]
pub struct ValidatedFlowEdge<'a> {
    source: &'a str,
    target: &'a str,
}

impl<'a> ValidatedFlowDraft<'a> {
    pub fn nodes(&self) -> &[ValidatedFlowNode<'a>] {
        &self.nodes
    }

    pub fn edges(&self) -> &[ValidatedFlowEdge<'a>] {
        &self.edges
    }
}

impl<'a> ValidatedFlowNode<'a> {
    pub fn value(&self) -> &'a Value {
        self.value
    }

    pub fn id(&self) -> &'a str {
        self.id
    }

    pub fn node_type(&self) -> &'a str {
        self.node_type
    }

    pub fn outputs(&self) -> &'a [Value] {
        self.outputs
    }

    pub fn bindings(&self) -> &'a Map<String, Value> {
        self.bindings
    }
}

impl<'a> ValidatedFlowEdge<'a> {
    pub fn source(&self) -> &'a str {
        self.source
    }

    pub fn target(&self) -> &'a str {
        self.target
    }
}

pub fn validate_flow_draft_structure(document: &Value) -> Result<(), FlowDocumentValidationError> {
    validated_flow_draft_structure(document).map(|_| ())
}

pub fn validated_flow_draft_structure(
    document: &Value,
) -> Result<ValidatedFlowDraft<'_>, FlowDocumentValidationError> {
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
    let mut validated_nodes = Vec::with_capacity(nodes.len());
    let mut validated_edges = Vec::with_capacity(edges.len());
    let mut node_ids = std::collections::BTreeSet::new();
    for node in nodes {
        validate_flow_node_structure(node)?;
        let node_string = |key| {
            nonempty_string(node, key).ok_or_else(|| {
                invalid(
                    Some(node),
                    &format!("/{key}"),
                    "node id and type must be nonempty strings",
                )
            })
        };
        let id = node_string("id")?;
        let node_type = node_string("type")?;
        if !node_ids.insert(id) {
            return Err(invalid(Some(node), "/id", "node ids must be unique"));
        }
        let node_object = |key| {
            node.get(key).and_then(Value::as_object).ok_or_else(|| {
                invalid(
                    Some(node),
                    &format!("/{key}"),
                    "node config and bindings must be objects",
                )
            })
        };
        node_object("config")?;
        let bindings = node_object("bindings")?;
        let outputs = node
            .get("outputs")
            .and_then(Value::as_array)
            .ok_or_else(|| invalid(Some(node), "/outputs", "node outputs must be an array"))?;
        let mut keys = std::collections::BTreeSet::new();
        for (index, output) in outputs.iter().enumerate() {
            let path = format!("/outputs/{index}");
            let output_string = |field| {
                nonempty_string(output, field).ok_or_else(|| {
                    invalid(
                        Some(node),
                        &format!("{path}/{field}"),
                        "output key, title and valueType must be nonempty strings",
                    )
                })
            };
            let key = output_string("key")?;
            output_string("title")?;
            output_string("valueType")?;
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
        validated_nodes.push(ValidatedFlowNode {
            value: node,
            id,
            node_type,
            outputs,
            bindings,
        });
    }
    let mut edge_ids = std::collections::BTreeSet::new();
    for edge in edges {
        let edge_string = |key| {
            nonempty_string(edge, key).ok_or_else(|| {
                invalid(
                    None,
                    "/graph/edges",
                    "edge id, source and target must be nonempty strings",
                )
            })
        };
        let id = edge_string("id")?;
        let source = edge_string("source")?;
        let target = edge_string("target")?;
        if !edge_ids.insert(id) {
            return Err(invalid(None, "/graph/edges", "edge ids must be unique"));
        }
        validated_edges.push(ValidatedFlowEdge { source, target });
    }
    Ok(ValidatedFlowDraft {
        nodes: validated_nodes,
        edges: validated_edges,
    })
}

fn nonempty_string<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
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
