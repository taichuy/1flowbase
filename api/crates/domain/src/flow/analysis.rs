use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;

/// A read-only interpretation of a client draft. Warnings do not block execution.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct FlowDocumentAnalysis {
    pub diagnostics: Vec<FlowAnalysisDiagnostic>,
    pub node_outputs: Vec<FlowNodeOutputs>,
    pub variable_catalogs: Vec<FlowVariableCatalog>,
}

impl FlowDocumentAnalysis {
    pub fn has_errors(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|diagnostic| diagnostic.level == "error")
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct FlowAnalysisDiagnostic {
    pub node_id: Option<String>,
    pub code: String,
    pub level: String,
    /// JSON Pointer relative to the node, or to the document when node_id is absent.
    pub field_path: String,
    pub message: String,
    pub selector: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct FlowNodeOutputs {
    pub node_id: String,
    pub outputs: Vec<FlowResolvedOutput>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct FlowResolvedOutput {
    pub key: String,
    pub title: String,
    #[serde(rename = "valueType")]
    pub value_type: String,
    pub selector: Vec<String>,
    #[serde(rename = "jsonSchema")]
    pub json_schema: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct FlowVariableCatalog {
    pub node_id: String,
    pub field_path: String,
    pub variables: Vec<FlowVariableOption>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct FlowVariableOption {
    pub node_id: String,
    pub node_label: String,
    pub output_key: String,
    pub output_label: String,
    pub value_type: String,
    pub json_schema: Option<Value>,
    pub selector: Vec<String>,
}
