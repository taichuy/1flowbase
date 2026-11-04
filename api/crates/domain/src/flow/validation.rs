use serde_json::{json, Value};

/// Structural constraints apply to drafts even when their execution configuration is incomplete.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlowDocumentValidationError {
    pub node_id: Option<String>,
    pub node_type: String,
}

impl std::fmt::Display for FlowDocumentValidationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "{} node {} outputs must be empty",
            self.node_type,
            self.node_id.as_deref().unwrap_or("(unnamed)")
        )
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
        });
    }
    Ok(())
}

pub fn validate_flow_draft_structure(document: &Value) -> Result<(), FlowDocumentValidationError> {
    if let Some(nodes) = document.pointer("/graph/nodes").and_then(Value::as_array) {
        for node in nodes {
            validate_flow_node_structure(node)?;
        }
    }
    Ok(())
}
