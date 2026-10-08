use crate::compiled_plan::CompileIssue;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Public, compiler-owned configuration diagnostics. Paths are JSON Pointers
/// relative to the identified node, or the document when node_id is absent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeDiagnostic {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node_id: Option<String>,
    pub code: String,
    pub field_path: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlowValidationError {
    pub diagnostics: Vec<NodeDiagnostic>,
}

impl std::fmt::Display for FlowValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.diagnostics.first() {
            Some(diagnostic) => f.write_str(&diagnostic.message),
            None => f.write_str("Flow configuration is invalid"),
        }
    }
}

impl std::error::Error for FlowValidationError {}

impl FlowValidationError {
    pub fn from_issues<'a>(issues: impl IntoIterator<Item = &'a CompileIssue>) -> Self {
        Self {
            diagnostics: issues.into_iter().map(NodeDiagnostic::from).collect(),
        }
    }

    pub(super) fn node(
        node_id: &str,
        code: &str,
        field_path: &str,
        message: impl Into<String>,
        expected: Option<Value>,
    ) -> Self {
        Self {
            diagnostics: vec![NodeDiagnostic {
                node_id: Some(node_id.to_owned()),
                code: code.to_owned(),
                field_path: field_path.to_owned(),
                message: message.into(),
                expected,
            }],
        }
    }

    pub(super) fn at_node(node_id: &str, field_path: &str, error: anyhow::Error) -> anyhow::Error {
        if error.downcast_ref::<Self>().is_some() {
            return error;
        }
        Self::node(
            node_id,
            "invalid_node_configuration",
            field_path,
            format!("{error:#}"),
            None,
        )
        .into()
    }

    pub(super) fn at_document(error: anyhow::Error) -> anyhow::Error {
        if error.downcast_ref::<Self>().is_some() {
            return error;
        }
        Self {
            diagnostics: vec![NodeDiagnostic {
                node_id: None,
                code: "invalid_flow_document".to_owned(),
                field_path: String::new(),
                message: format!("{error:#}"),
                expected: None,
            }],
        }
        .into()
    }
}

impl From<&CompileIssue> for NodeDiagnostic {
    fn from(issue: &CompileIssue) -> Self {
        // CompileIssueCode serializes as a stable snake_case string.
        let code = serde_json::to_value(issue.code).expect("compile issue code is serializable");
        Self {
            node_id: Some(issue.node_id.clone()),
            code: code
                .as_str()
                .expect("compile issue code is a string")
                .to_owned(),
            field_path: issue.field_path.clone().unwrap_or_default(),
            message: issue.message.clone(),
            expected: None,
        }
    }
}

pub(super) fn pointer_segment(value: &str) -> String {
    value.replace('~', "~0").replace('/', "~1")
}
