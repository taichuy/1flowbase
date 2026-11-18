//! Canonical capabilities for flow-document node kinds. Consumers project this
//! registry for authoring, compilation and portable application resolution.
use crate::ApplicationType;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApplicationNodeRuntimeStatus {
    Ready,
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApplicationNodeAuthoringStatus {
    Published,
    Hidden,
}

impl ApplicationNodeRuntimeStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::Unavailable => "unavailable",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlowNodeKind {
    Builtin,
    ContainerEntry,
    Plugin,
    Placeholder,
}

#[derive(Debug, Clone, Copy)]
pub struct FlowNodeCapability {
    pub node_type: &'static str,
    pub title: &'static str,
    pub category: &'static str,
    pub authoring_status: ApplicationNodeAuthoringStatus,
    pub runtime_status: ApplicationNodeRuntimeStatus,
    pub kind: FlowNodeKind,
    application_type: Option<ApplicationType>,
}

impl FlowNodeCapability {
    pub fn supports_application(self, application_type: ApplicationType) -> bool {
        application_type != ApplicationType::AgentLogs
            && self
                .application_type
                .is_none_or(|kind| kind == application_type)
    }

    pub fn appears_in_catalog(self) -> bool {
        self.kind == FlowNodeKind::Builtin
    }

    pub fn is_importable_builtin(self, application_type: ApplicationType) -> bool {
        matches!(
            self.kind,
            FlowNodeKind::Builtin | FlowNodeKind::ContainerEntry
        ) && self.runtime_status == ApplicationNodeRuntimeStatus::Ready
            && self.supports_application(application_type)
    }

    pub fn is_compiler_supported(self) -> bool {
        // Unresolved documents must reach the existing diagnostic path, not
        // acquire the execution eligibility of a ready built-in node.
        self.runtime_status == ApplicationNodeRuntimeStatus::Ready
            || self.kind == FlowNodeKind::Placeholder
    }
}

const fn published_ready(
    node_type: &'static str,
    title: &'static str,
    category: &'static str,
) -> FlowNodeCapability {
    FlowNodeCapability {
        node_type,
        title,
        category,
        authoring_status: ApplicationNodeAuthoringStatus::Published,
        runtime_status: ApplicationNodeRuntimeStatus::Ready,
        kind: FlowNodeKind::Builtin,
        application_type: None,
    }
}
const fn hidden_ready(
    node_type: &'static str,
    title: &'static str,
    category: &'static str,
) -> FlowNodeCapability {
    FlowNodeCapability {
        authoring_status: ApplicationNodeAuthoringStatus::Hidden,
        ..published_ready(node_type, title, category)
    }
}
const fn hidden_unavailable(
    node_type: &'static str,
    title: &'static str,
    category: &'static str,
) -> FlowNodeCapability {
    FlowNodeCapability {
        runtime_status: ApplicationNodeRuntimeStatus::Unavailable,
        ..hidden_ready(node_type, title, category)
    }
}
const fn boundary(
    node_type: &'static str,
    title: &'static str,
    application_type: ApplicationType,
) -> FlowNodeCapability {
    FlowNodeCapability {
        application_type: Some(application_type),
        ..published_ready(node_type, title, "io")
    }
}

static FLOW_NODE_CAPABILITIES: &[FlowNodeCapability] = &[
    boundary("start", "Start", ApplicationType::AgentFlow),
    boundary("answer", "Answer", ApplicationType::AgentFlow),
    boundary(
        "workflow_start",
        "Workflow Start",
        ApplicationType::Workflow,
    ),
    boundary("workflow_end", "Workflow End", ApplicationType::Workflow),
    published_ready("llm", "LLM", "generation"),
    hidden_unavailable("knowledge_retrieval", "Knowledge Retrieval", "generation"),
    hidden_unavailable("question_classifier", "Question Classifier", "control"),
    published_ready("if_else", "If / Else", "control"),
    published_ready("code", "Code", "data"),
    published_ready("template_transform", "Template Transform", "generation"),
    published_ready("http_request", "HTTP Request", "external"),
    published_ready("notification", "Notification", "external"),
    hidden_ready("tool", "Tool", "external"),
    published_ready("tool_result", "Tool Result", "io"),
    published_ready("data_model_list", "Data Model List", "data"),
    published_ready("data_model_get", "Data Model Get", "data"),
    published_ready("data_model_create", "Data Model Create", "data"),
    published_ready("data_model_update", "Data Model Update", "data"),
    published_ready("data_model_delete", "Data Model Delete", "data"),
    published_ready("sql", "SQL", "data"),
    published_ready("variable_assigner", "Variable Assigner", "data"),
    published_ready("variable_aggregator", "Variable Aggregator", "control"),
    hidden_unavailable("parameter_extractor", "Parameter Extractor", "data"),
    published_ready("iteration", "Iteration", "control"),
    published_ready("loop", "Loop", "control"),
    hidden_ready("human_input", "Human Input", "io"),
    FlowNodeCapability {
        kind: FlowNodeKind::ContainerEntry,
        ..hidden_ready("container_start", "Container Start", "control")
    },
    FlowNodeCapability {
        kind: FlowNodeKind::Plugin,
        ..hidden_ready("plugin_node", "Plugin Node", "external")
    },
    FlowNodeCapability {
        kind: FlowNodeKind::Placeholder,
        ..hidden_unavailable("unresolved_node", "Unresolved Node", "control")
    },
];

pub fn flow_node_capabilities() -> &'static [FlowNodeCapability] {
    FLOW_NODE_CAPABILITIES
}

pub fn flow_node_capability(node_type: &str) -> Option<&'static FlowNodeCapability> {
    FLOW_NODE_CAPABILITIES
        .iter()
        .find(|node| node.node_type == node_type)
}

#[cfg(test)]
#[path = "_tests/node_capabilities.rs"]
mod tests;
