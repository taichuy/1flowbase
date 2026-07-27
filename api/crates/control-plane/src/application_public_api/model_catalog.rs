use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use super::native::{
    NativeExecutionModelParameters, NativeReasoningMode, NativeReasoningParameters,
};

const DEFAULT_AGENT_MODEL_ID: &str = "1flowbase";
const DEFAULT_AGENT_CONTEXT_WINDOW: u64 = 257_000;
const DEFAULT_AGENT_MAX_CONTEXT_WINDOW: u64 = 128_000;
const DEFAULT_AGENT_MAX_OUTPUT_TOKENS: u64 = 32_000;
const DEFAULT_AGENT_AUTO_COMPACT_PERCENT: u64 = 85;
const DEFAULT_AGENT_REASONING_EFFORT: &str = "medium";
const DEFAULT_AGENT_REASONING_EFFORTS: [&str; 5] = ["minimal", "low", "medium", "high", "xhigh"];

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentModelCapabilities {
    pub reasoning: bool,
    pub tool_call: bool,
    pub multimodal: bool,
    pub structured_output: bool,
}

impl AgentModelCapabilities {
    pub fn is_empty(&self) -> bool {
        !self.reasoning && !self.tool_call && !self.multimodal && !self.structured_output
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentModelReasoning {
    pub default_effort: Option<String>,
    pub supported_efforts: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentModelDescriptor {
    pub id: String,
    pub name: Option<String>,
    pub context_window: Option<u64>,
    pub max_context_window: Option<u64>,
    pub max_output_tokens: Option<u64>,
    pub auto_compact_token_limit: Option<u64>,
    pub capabilities: AgentModelCapabilities,
    pub reasoning: Option<AgentModelReasoning>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelectedLlmModelParameterError {
    RequestedContextWindowUnavailable { requested: u64 },
    RequestedContextWindowExceeded { requested: u64, supported: u64 },
    ReasoningUnsupported,
    ReasoningEffortUnsupported { requested: String },
}

/// Validates explicit request intent against the descriptor resolved by an LLM
/// node. Callers must not substitute the public request `model` for the model
/// actually selected by that node.
pub fn validate_selected_llm_model_parameters(
    selected_model: &AgentModelDescriptor,
    parameters: &NativeExecutionModelParameters,
) -> Result<(), SelectedLlmModelParameterError> {
    if let Some(requested) = parameters.requested_context_window() {
        let supported = selected_model
            .max_context_window
            .or(selected_model.context_window)
            .ok_or(
                SelectedLlmModelParameterError::RequestedContextWindowUnavailable { requested },
            )?;
        if requested > supported {
            return Err(
                SelectedLlmModelParameterError::RequestedContextWindowExceeded {
                    requested,
                    supported,
                },
            );
        }
    }

    let Some(reasoning) = parameters.reasoning() else {
        return Ok(());
    };
    validate_selected_llm_reasoning(selected_model, reasoning)
}

fn validate_selected_llm_reasoning(
    selected_model: &AgentModelDescriptor,
    reasoning: &NativeReasoningParameters,
) -> Result<(), SelectedLlmModelParameterError> {
    let requested_effort = reasoning.effort();
    let requests_reasoning = reasoning.effective_mode() != NativeReasoningMode::Disabled
        || requested_effort.is_some()
        || reasoning.budget_tokens().is_some();
    if !requests_reasoning {
        return Ok(());
    }

    let reasoning_descriptor = selected_model.reasoning.as_ref();
    let supports_reasoning = selected_model.capabilities.reasoning
        || reasoning_descriptor.is_some_and(|descriptor| {
            descriptor.default_effort.is_some() || !descriptor.supported_efforts.is_empty()
        });
    if !supports_reasoning {
        return Err(SelectedLlmModelParameterError::ReasoningUnsupported);
    }

    if let (Some(requested), Some(descriptor)) = (requested_effort, reasoning_descriptor) {
        if !descriptor.supported_efforts.is_empty()
            && !descriptor
                .supported_efforts
                .iter()
                .any(|supported| supported == requested)
        {
            return Err(SelectedLlmModelParameterError::ReasoningEffortUnsupported {
                requested: requested.to_string(),
            });
        }
    }
    Ok(())
}

pub fn extract_agent_model_catalog_from_start_node(document: &Value) -> Vec<AgentModelDescriptor> {
    let Some(nodes) = document
        .get("graph")
        .and_then(|graph| graph.get("nodes"))
        .and_then(Value::as_array)
    else {
        return default_model_catalog();
    };
    let Some(start_node) = nodes
        .iter()
        .find(|node| node.get("type").and_then(Value::as_str) == Some("start"))
    else {
        return default_model_catalog();
    };
    let Some(model_list) = start_node
        .get("config")
        .and_then(|config| config.get("model_list"))
        .and_then(Value::as_array)
    else {
        return default_model_catalog();
    };

    let mut models = Vec::new();
    for value in model_list {
        if let Some(model) = normalize_model_descriptor(value) {
            if !models
                .iter()
                .any(|existing: &AgentModelDescriptor| existing.id == model.id)
            {
                models.push(model);
            }
        }
    }
    if models.is_empty() {
        default_model_catalog()
    } else {
        models
    }
}

fn default_model_catalog() -> Vec<AgentModelDescriptor> {
    vec![AgentModelDescriptor {
        id: DEFAULT_AGENT_MODEL_ID.to_string(),
        name: Some(DEFAULT_AGENT_MODEL_ID.to_string()),
        context_window: Some(DEFAULT_AGENT_CONTEXT_WINDOW),
        max_context_window: Some(DEFAULT_AGENT_MAX_CONTEXT_WINDOW),
        max_output_tokens: Some(DEFAULT_AGENT_MAX_OUTPUT_TOKENS),
        auto_compact_token_limit: Some(
            (DEFAULT_AGENT_CONTEXT_WINDOW * DEFAULT_AGENT_AUTO_COMPACT_PERCENT) / 100,
        ),
        capabilities: AgentModelCapabilities {
            reasoning: true,
            tool_call: true,
            multimodal: true,
            structured_output: true,
        },
        reasoning: Some(AgentModelReasoning {
            default_effort: Some(DEFAULT_AGENT_REASONING_EFFORT.to_string()),
            supported_efforts: DEFAULT_AGENT_REASONING_EFFORTS
                .iter()
                .map(|effort| (*effort).to_string())
                .collect(),
        }),
    }]
}

fn normalize_model_descriptor(value: &Value) -> Option<AgentModelDescriptor> {
    if let Some(id) = value.as_str().map(str::trim).filter(|id| !id.is_empty()) {
        return Some(AgentModelDescriptor {
            id: id.to_string(),
            name: None,
            context_window: None,
            max_context_window: None,
            max_output_tokens: None,
            auto_compact_token_limit: None,
            capabilities: AgentModelCapabilities::default(),
            reasoning: None,
        });
    }

    let object = value.as_object()?;
    let id = object
        .get("id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|id| !id.is_empty())?;
    let name = object
        .get("name")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(ToOwned::to_owned);

    Some(AgentModelDescriptor {
        id: id.to_string(),
        name,
        context_window: model_token_u64(object, "context_window"),
        max_context_window: model_token_u64(object, "max_context_window"),
        max_output_tokens: model_token_u64(object, "max_output_tokens"),
        auto_compact_token_limit: model_token_u64(object, "auto_compact_token_limit"),
        capabilities: normalize_capabilities(object),
        reasoning: normalize_reasoning(object.get("reasoning")),
    })
}

fn normalize_capabilities(object: &Map<String, Value>) -> AgentModelCapabilities {
    let capabilities = object.get("capabilities").and_then(Value::as_object);

    AgentModelCapabilities {
        reasoning: capabilities
            .and_then(|value| value.get("reasoning"))
            .and_then(Value::as_bool)
            .unwrap_or(false),
        tool_call: capabilities
            .and_then(|value| value.get("tool_call"))
            .and_then(Value::as_bool)
            .unwrap_or(false),
        multimodal: capabilities
            .and_then(|value| value.get("multimodal"))
            .and_then(Value::as_bool)
            .unwrap_or(false),
        structured_output: capabilities
            .and_then(|value| value.get("structured_output"))
            .and_then(Value::as_bool)
            .unwrap_or(false),
    }
}

fn normalize_reasoning(value: Option<&Value>) -> Option<AgentModelReasoning> {
    let object = value.and_then(Value::as_object)?;
    let default_effort = object
        .get("default_effort")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    let supported_efforts = object
        .get("supported_efforts")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    Some(AgentModelReasoning {
        default_effort,
        supported_efforts,
    })
}

fn model_token_u64(object: &Map<String, Value>, key: &str) -> Option<u64> {
    object.get(key).and_then(Value::as_u64)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::application_public_api::native::NativeRunRequest;

    fn selected_model(
        max_context_window: Option<u64>,
        reasoning: bool,
        supported_efforts: &[&str],
    ) -> AgentModelDescriptor {
        AgentModelDescriptor {
            id: "selected-provider-model".to_string(),
            name: None,
            context_window: None,
            max_context_window,
            max_output_tokens: Some(32_000),
            auto_compact_token_limit: None,
            capabilities: AgentModelCapabilities {
                reasoning,
                ..AgentModelCapabilities::default()
            },
            reasoning: reasoning.then(|| AgentModelReasoning {
                default_effort: None,
                supported_efforts: supported_efforts
                    .iter()
                    .map(|effort| (*effort).to_string())
                    .collect(),
            }),
        }
    }

    fn request_with_model_parameters(model_parameters: Value) -> NativeRunRequest {
        serde_json::from_value(json!({
            "query": "hello",
            "execution": {"model_parameters": model_parameters}
        }))
        .expect("model-parameter fixture should be valid Native input")
    }

    #[test]
    fn extracts_start_node_model_catalog_with_capabilities() {
        let document = json!({
            "graph": {
                "nodes": [
                    {
                        "id": "node-start",
                        "type": "start",
                        "config": {
                            "model_list": [
                                {
                                    "id": "qwen3.6-35b-a3b",
                                    "name": "Qwen 3.6 35B",
                                    "context_window": 128000,
                                    "max_output_tokens": 32000,
                                    "auto_compact_token_limit": 110000,
                                    "capabilities": {
                                        "reasoning": true,
                                        "tool_call": true,
                                        "multimodal": false,
                                        "structured_output": true
                                    },
                                    "reasoning": {
                                        "default_effort": "medium",
                                        "supported_efforts": ["low", "medium", "high"]
                                    }
                                },
                                "deepseek-v4-flash",
                                {"id": "deepseek-v4-flash", "name": "Duplicate"}
                            ]
                        }
                    }
                ]
            }
        });

        let models = extract_agent_model_catalog_from_start_node(&document);

        assert_eq!(models.len(), 2);
        assert_eq!(models[0].id, "qwen3.6-35b-a3b");
        assert_eq!(models[0].context_window, Some(128000));
        assert_eq!(models[0].max_output_tokens, Some(32000));
        assert!(models[0].capabilities.reasoning);
        assert_eq!(
            models[0]
                .reasoning
                .as_ref()
                .expect("reasoning should be present")
                .supported_efforts,
            vec!["low", "medium", "high"]
        );
        assert_eq!(models[1].id, "deepseek-v4-flash");
    }

    #[test]
    fn wp_d2a_selected_llm_catalog_accepts_exact_explicit_window_and_reasoning() {
        let selected = selected_model(Some(1_000_000), true, &["low", "max"]);
        let request = request_with_model_parameters(json!({
            "requested_context_window": 1_000_000,
            "reasoning": {"mode": "adaptive", "effort": "max"}
        }));
        let parameters = request
            .execution
            .model_parameters()
            .expect("fixture has model parameters");

        validate_selected_llm_model_parameters(&selected, parameters)
            .expect("the actual selected model advertises the explicit request");
        assert_eq!(
            parameters.canonical_value(),
            json!({
                "requested_context_window": 1_000_000,
                "reasoning": {"mode": "adaptive", "effort": "max"}
            })
        );
    }

    #[test]
    fn wp_d2a_selected_llm_catalog_rejects_instead_of_inferring_or_downgrading() {
        let selected = selected_model(Some(128_000), true, &["high", "xhigh"]);
        let explicit = request_with_model_parameters(json!({
            "requested_context_window": 1_000_000,
            "reasoning": {"mode": "enabled", "effort": "max"}
        }));
        let explicit_parameters = explicit
            .execution
            .model_parameters()
            .expect("fixture has explicit model parameters");

        assert_eq!(
            validate_selected_llm_model_parameters(&selected, explicit_parameters),
            Err(
                SelectedLlmModelParameterError::RequestedContextWindowExceeded {
                    requested: 1_000_000,
                    supported: 128_000,
                }
            )
        );
        assert_eq!(
            explicit_parameters.canonical_value()["requested_context_window"],
            json!(1_000_000),
            "validation must not replace an unsupported request with the catalog limit"
        );

        let absent = request_with_model_parameters(json!({"max_output_tokens": 4096}));
        let absent_parameters = absent
            .execution
            .model_parameters()
            .expect("fixture has a non-context model parameter");
        validate_selected_llm_model_parameters(&selected, absent_parameters)
            .expect("an absent context request must remain absent");
        assert!(absent_parameters
            .canonical_value()
            .get("requested_context_window")
            .is_none());
    }

    #[test]
    fn wp_d2a_selected_llm_catalog_requires_reasoning_capability_and_exact_effort() {
        let request = request_with_model_parameters(json!({
            "reasoning": {"mode": "adaptive", "effort": "max"}
        }));
        let parameters = request
            .execution
            .model_parameters()
            .expect("fixture has reasoning parameters");

        assert_eq!(
            validate_selected_llm_model_parameters(
                &selected_model(Some(1_000_000), false, &[]),
                parameters,
            ),
            Err(SelectedLlmModelParameterError::ReasoningUnsupported)
        );
        assert_eq!(
            validate_selected_llm_model_parameters(
                &selected_model(Some(1_000_000), true, &["high", "xhigh"]),
                parameters,
            ),
            Err(SelectedLlmModelParameterError::ReasoningEffortUnsupported {
                requested: "max".to_string(),
            })
        );
        assert_eq!(
            parameters.canonical_value()["reasoning"]["effort"],
            json!("max"),
            "validation must not downgrade max to xhigh"
        );
    }
}
