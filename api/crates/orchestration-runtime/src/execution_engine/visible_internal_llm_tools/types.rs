use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(super) enum VisibleInternalLlmToolExternalToolPolicy {
    #[default]
    Forbidden,
    Inherited,
}

impl VisibleInternalLlmToolExternalToolPolicy {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Forbidden => EXTERNAL_TOOL_POLICY_FORBIDDEN,
            Self::Inherited => EXTERNAL_TOOL_POLICY_INHERITED,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(super) enum VisibleInternalLlmToolMode {
    #[default]
    Agent,
    Fusion,
}

impl VisibleInternalLlmToolMode {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Agent => TOOL_MODE_AGENT,
            Self::Fusion => TOOL_MODE_FUSION,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(super) enum VisibleInternalLlmToolExternalCallbackPolicy {
    Forbidden,
    #[default]
    Inherited,
}

impl VisibleInternalLlmToolExternalCallbackPolicy {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Forbidden => EXTERNAL_CALLBACK_POLICY_FORBIDDEN,
            Self::Inherited => EXTERNAL_CALLBACK_POLICY_INHERITED,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(super) enum VisibleInternalLlmToolExecutionMode {
    #[default]
    SequentialResume,
    BoundedParallelPanel,
}

impl VisibleInternalLlmToolExecutionMode {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::SequentialResume => EXECUTION_MODE_SEQUENTIAL_RESUME,
            Self::BoundedParallelPanel => EXECUTION_MODE_BOUNDED_PARALLEL_PANEL,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct VisibleInternalLlmTool {
    pub(super) name: String,
    pub(super) description: Option<String>,
    pub(super) target_node_id: String,
    pub(super) target_node_ids: Vec<String>,
    pub(super) input_schema: Option<Value>,
    pub(super) tool_mode: VisibleInternalLlmToolMode,
    pub(super) external_tool_policy: VisibleInternalLlmToolExternalToolPolicy,
    pub(super) external_callback_policy: VisibleInternalLlmToolExternalCallbackPolicy,
    pub(super) execution_mode: VisibleInternalLlmToolExecutionMode,
    pub(super) preconditions: Vec<VisibleInternalLlmToolPrecondition>,
}

impl VisibleInternalLlmTool {
    pub(super) fn start_node_ids(&self) -> std::collections::BTreeSet<String> {
        let target_node_ids = if self.target_node_ids.is_empty() {
            vec![self.target_node_id.clone()]
        } else {
            self.target_node_ids.clone()
        };
        target_node_ids.into_iter().collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct VisibleInternalLlmToolMediaContentPrecondition {
    pub(super) argument_path: Vec<String>,
    pub(super) media_kind: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum VisibleInternalLlmToolPrecondition {
    MediaContentAvailable(VisibleInternalLlmToolMediaContentPrecondition),
}

pub(super) fn visible_internal_llm_tool_precondition_from_value(
    value: &Value,
) -> Option<VisibleInternalLlmToolPrecondition> {
    let object = value.as_object()?;
    let kind = object
        .get("kind")
        .or_else(|| object.get("type"))
        .and_then(Value::as_str)
        .map(str::trim)?;
    if kind != VISIBLE_INTERNAL_LLM_TOOL_PRECONDITION_MEDIA_CONTENT_AVAILABLE {
        return None;
    }

    Some(VisibleInternalLlmToolPrecondition::MediaContentAvailable(
        VisibleInternalLlmToolMediaContentPrecondition {
            argument_path: media_content_precondition_argument_path(object)
                .unwrap_or_else(|| vec!["media".to_string()]),
            media_kind: object
                .get("media_kind")
                .or_else(|| object.get("mediaKind"))
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|kind| !kind.is_empty())
                .map(str::to_string),
        },
    ))
}

pub(super) fn visible_internal_llm_tool_preconditions_from_value(
    value: Option<&Value>,
) -> Vec<VisibleInternalLlmToolPrecondition> {
    value
        .and_then(Value::as_array)
        .map(|preconditions| {
            preconditions
                .iter()
                .filter_map(visible_internal_llm_tool_precondition_from_value)
                .collect()
        })
        .unwrap_or_default()
}

pub(super) fn visible_internal_llm_tool_preconditions_value(
    preconditions: &[VisibleInternalLlmToolPrecondition],
) -> Value {
    Value::Array(
        preconditions
            .iter()
            .map(visible_internal_llm_tool_precondition_value)
            .collect(),
    )
}

pub(super) fn visible_internal_llm_tool_precondition_value(
    precondition: &VisibleInternalLlmToolPrecondition,
) -> Value {
    match precondition {
        VisibleInternalLlmToolPrecondition::MediaContentAvailable(media) => json!({
            "kind": VISIBLE_INTERNAL_LLM_TOOL_PRECONDITION_MEDIA_CONTENT_AVAILABLE,
            "argument_path": media.argument_path,
            "media_kind": media.media_kind,
        }),
    }
}

fn media_content_precondition_argument_path(object: &Map<String, Value>) -> Option<Vec<String>> {
    if let Some(path) = object
        .get("argument_path")
        .or_else(|| object.get("argumentPath"))
        .and_then(argument_path_from_value)
    {
        return Some(path);
    }

    object
        .get("selector")
        .and_then(Value::as_str)
        .and_then(argument_path_from_selector)
}

fn argument_path_from_value(value: &Value) -> Option<Vec<String>> {
    if let Some(path) = value.as_array() {
        let path = path
            .iter()
            .filter_map(Value::as_str)
            .map(str::trim)
            .filter(|segment| !segment.is_empty())
            .map(str::to_string)
            .collect::<Vec<_>>();
        return (!path.is_empty()).then_some(path);
    }

    value.as_str().and_then(argument_path_from_selector)
}

fn argument_path_from_selector(selector: &str) -> Option<Vec<String>> {
    let selector = selector.trim();
    if selector.is_empty() {
        return None;
    }
    let selector = selector
        .strip_prefix("$.")
        .or_else(|| selector.strip_prefix('$'))
        .unwrap_or(selector)
        .trim_start_matches('.');
    let selector = selector.strip_suffix("[*]").unwrap_or(selector);
    let path = selector
        .split('.')
        .map(str::trim)
        .filter(|segment| !segment.is_empty() && !segment.contains('['))
        .map(str::to_string)
        .collect::<Vec<_>>();
    (!path.is_empty()).then_some(path)
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct VisibleInternalLlmToolOutput {
    pub(super) text: String,
    pub(super) provider_events: Vec<ProviderStreamEvent>,
    pub(super) route_events: Vec<Value>,
    pub(super) variable_pool: Map<String, Value>,
}

pub(super) enum VisibleInternalLlmToolBranchExecution {
    Completed(VisibleInternalLlmToolOutput),
    Waiting {
        wait: Box<LlmToolCallbackWait>,
        branch_text: String,
        route_events: Vec<Value>,
    },
    Failed {
        error_payload: Value,
        route_events: Vec<Value>,
    },
}

pub(super) struct VisibleInternalLlmToolNodeOutput {
    pub(super) input_payload: Value,
    pub(super) output_payload: Value,
    pub(super) metrics_payload: Option<Value>,
    pub(super) debug_payload: Option<Value>,
}

impl VisibleInternalLlmToolNodeOutput {
    pub(super) fn from_output_payload(output_payload: Value) -> Self {
        Self {
            input_payload: Value::Null,
            output_payload,
            metrics_payload: None,
            debug_payload: None,
        }
    }
}

pub(super) enum VisibleInternalLlmToolNodeExecution {
    Completed(Box<VisibleInternalLlmToolNodeOutput>),
    Waiting(Box<LlmToolCallbackWait>),
    Failed(Value),
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct VisibleInternalLlmToolPendingCall {
    pub(super) tool_call: Value,
    pub(super) tool: VisibleInternalLlmTool,
}

pub(super) enum VisibleInternalLlmToolRemainingExecution {
    Completed {
        tool_results: Vec<Value>,
        visible_transcript: String,
        provider_events: Vec<ProviderStreamEvent>,
        route_events: Vec<Value>,
        variable_pool: Map<String, Value>,
    },
    Waiting(Box<LlmToolCallbackWait>),
    Failed {
        error_payload: Value,
        provider_events: Vec<ProviderStreamEvent>,
        route_events: Vec<Value>,
    },
}
