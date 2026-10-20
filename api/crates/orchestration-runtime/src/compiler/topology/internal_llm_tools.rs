use super::*;

pub(super) fn validate_llm_context_policies(
    nodes: &BTreeMap<String, CompiledNode>,
) -> Vec<CompileIssue> {
    nodes
        .values()
        .filter(|node| node.node_type == "llm")
        .filter_map(|node| {
            let selector = context_policy_selector(node)?;
            if !RUN_LEVEL_VARIABLE_NAMESPACES.contains(&selector[0].as_str())
                && !node_depends_on(nodes, &node.node_id, &selector[0])
            {
                return None;
            }
            let Some(output) = output_for_selector(nodes, &selector) else {
                return Some(CompileIssue {
                    field_path: None,
                    node_id: node.node_id.clone(),
                    code: CompileIssueCode::InvalidLlmContextSelector,
                    message: format!(
                        "node {} context_selector references unavailable output {}",
                        node.node_id,
                        selector.join(".")
                    ),
                });
            };

            (!output_schema_is_llm_context_messages(&output)).then(|| CompileIssue {
                field_path: None,
                node_id: node.node_id.clone(),
                code: CompileIssueCode::IncompatibleLlmContextSchema,
                message: format!(
                    "node {} context_selector {} must reference an output with an LLM history-compatible jsonSchema",
                    node.node_id,
                    selector.join(".")
                ),
            })
        })
        .collect()
}

pub(super) fn materialize_visible_internal_llm_tool_targets(
    nodes: &mut BTreeMap<String, CompiledNode>,
    edges: &[CompiledEdge],
) -> Vec<CompileIssue> {
    let mut issues = Vec::new();

    for node in nodes.values_mut().filter(|node| node.node_type == "llm") {
        if !visible_internal_llm_tools_enabled(node) {
            continue;
        }

        let node_id = node.node_id.clone();
        let Some(tools) = visible_internal_llm_tools_array_mut(&mut node.config) else {
            continue;
        };

        for tool in tools {
            if tool.get("type").and_then(Value::as_str) != Some(VISIBLE_INTERNAL_LLM_TOOL_TYPE) {
                continue;
            }
            materialize_visible_internal_llm_tool_policy(tool);
            let connector_id = tool
                .get("connector_id")
                .or_else(|| tool.get("connectorId"))
                .or_else(|| tool.get("tool_name"))
                .or_else(|| tool.get("name"))
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty());
            let Some(connector_id) = connector_id else {
                issues.push(CompileIssue {
                    field_path: None,
                    node_id: node_id.clone(),
                    code: CompileIssueCode::InvalidVisibleInternalLlmTool,
                    message: format!(
                        "node {} visible_internal_llm_tools entry is missing connector_id",
                        node_id
                    ),
                });
                continue;
            };
            let source_handle = visible_internal_llm_tool_source_handle(connector_id);
            let matching_edges = edges
                .iter()
                .filter(|edge| {
                    edge.source == node_id && edge.source_handle.as_deref() == Some(&source_handle)
                })
                .collect::<Vec<_>>();

            if matching_edges.is_empty() {
                issues.push(CompileIssue {
                    field_path: None,
                    node_id: node_id.clone(),
                    code: CompileIssueCode::InvalidVisibleInternalLlmTool,
                    message: format!(
                        "node {} visible_internal_llm_tool connector {connector_id} is missing a graph edge",
                        node_id
                    ),
                });
                continue;
            };
            let tool_mode = visible_internal_llm_tool_mode_value(tool)
                .unwrap_or_else(|| TOOL_MODE_AGENT.to_string());
            if matching_edges.len() > 1 && tool_mode != TOOL_MODE_FUSION {
                issues.push(CompileIssue {
                    field_path: None,
                    node_id: node_id.clone(),
                    code: CompileIssueCode::InvalidVisibleInternalLlmTool,
                    message: format!(
                        "node {} visible_internal_llm_tool connector {connector_id} has multiple graph edges",
                        node_id
                    ),
                });
                continue;
            }

            if let Some(tool_object) = tool.as_object_mut() {
                tool_object.insert(
                    "target_node_id".to_string(),
                    Value::String(matching_edges[0].target.clone()),
                );
                if tool_mode == TOOL_MODE_FUSION {
                    tool_object.insert(
                        "target_node_ids".to_string(),
                        Value::Array(
                            matching_edges
                                .iter()
                                .map(|edge| Value::String(edge.target.clone()))
                                .collect(),
                        ),
                    );
                }
            }
        }
    }

    issues
}

pub(super) fn validate_visible_internal_llm_tool_branches(
    nodes: &BTreeMap<String, CompiledNode>,
    edges: &[CompiledEdge],
) -> Vec<CompileIssue> {
    let mut issues = Vec::new();

    for node in nodes.values().filter(|node| node.node_type == "llm") {
        if !visible_internal_llm_tools_enabled(node) {
            continue;
        }

        for tool in visible_internal_llm_tool_entries(node) {
            let allow_internal_llm = visible_internal_llm_tool_allows_internal_llm_node(tool);
            let connector_id = visible_internal_llm_tool_connector_id(tool)
                .unwrap_or_else(|| "unknown".to_string());
            if let Some(mode) = visible_internal_llm_tool_mode_value(tool) {
                if mode != TOOL_MODE_AGENT && mode != TOOL_MODE_FUSION {
                    issues.push(CompileIssue {
                        field_path: None,
                        node_id: node.node_id.clone(),
                        code: CompileIssueCode::InvalidVisibleInternalLlmTool,
                        message: format!(
                            "node {} visible_internal_llm_tool connector {connector_id} has invalid tool_mode {mode}; expected agent or fusion",
                            node.node_id
                        ),
                    });
                }
            }
            if let Some(policy) = visible_internal_llm_tool_external_tool_policy_value(tool) {
                if policy != EXTERNAL_TOOL_POLICY_FORBIDDEN
                    && policy != EXTERNAL_TOOL_POLICY_INHERITED
                {
                    issues.push(CompileIssue {
                        field_path: None,
                        node_id: node.node_id.clone(),
                        code: CompileIssueCode::InvalidVisibleInternalLlmTool,
                        message: format!(
                            "node {} visible_internal_llm_tool connector {connector_id} has invalid external_tool_policy {policy}; expected forbidden or inherited",
                            node.node_id
                        ),
                    });
                }
            }
            let target_node_ids = visible_internal_llm_tool_target_node_ids(tool);
            if target_node_ids.is_empty() {
                continue;
            }

            let reachable_node_ids = target_node_ids
                .iter()
                .flat_map(|target_node_id| {
                    visible_internal_llm_tool_branch_node_ids(target_node_id, nodes, edges)
                })
                .collect::<BTreeSet<_>>();
            let tool_result_node_ids = reachable_node_ids
                .iter()
                .filter(|node_id| {
                    nodes
                        .get(*node_id)
                        .map(|reachable| reachable.node_type == TOOL_RESULT_NODE_TYPE)
                        .unwrap_or(false)
                })
                .cloned()
                .collect::<Vec<_>>();

            if tool_result_node_ids.is_empty() {
                issues.push(CompileIssue {
                    field_path: None,
                    node_id: node.node_id.clone(),
                    code: CompileIssueCode::InvalidVisibleInternalLlmTool,
                    message: format!(
                        "node {} visible_internal_llm_tool connector {connector_id} is missing a reachable tool_result node",
                        node.node_id
                    ),
                });
            } else if tool_result_node_ids.len() > 1 {
                issues.push(CompileIssue {
                    field_path: None,
                    node_id: node.node_id.clone(),
                    code: CompileIssueCode::InvalidVisibleInternalLlmTool,
                    message: format!(
                        "node {} visible_internal_llm_tool connector {connector_id} has multiple reachable tool_result nodes",
                        node.node_id
                    ),
                });
            }

            if !allow_internal_llm {
                for reachable_node_id in &reachable_node_ids {
                    if reachable_node_id == &node.node_id {
                        continue;
                    }

                    let Some(reachable_node) = nodes.get(reachable_node_id) else {
                        continue;
                    };

                    if reachable_node.node_type == "llm" {
                        issues.push(CompileIssue {
                            field_path: None,
                            node_id: node.node_id.clone(),
                            code: CompileIssueCode::InvalidVisibleInternalLlmTool,
                            message: format!(
                                "node {} visible_internal_llm_tool connector {connector_id} reaches LLM node {} without internal_llm_node_policy allowed",
                                node.node_id, reachable_node.node_id
                            ),
                        });
                        break;
                    }
                }
            }
        }
    }

    issues
}

pub(super) fn materialize_visible_internal_llm_tool_policy(tool: &mut Value) {
    let configured_mode = visible_internal_llm_tool_mode_value(tool);
    let mode = configured_mode
        .as_deref()
        .filter(|mode| *mode == TOOL_MODE_AGENT || *mode == TOOL_MODE_FUSION)
        .unwrap_or(TOOL_MODE_AGENT);
    let existing_external_tool_policy = visible_internal_llm_tool_external_tool_policy_value(tool);
    let Some(object) = tool.as_object_mut() else {
        return;
    };

    if configured_mode.is_none() {
        object.insert(
            "tool_mode".to_string(),
            Value::String(TOOL_MODE_AGENT.to_string()),
        );
    }
    if mode == TOOL_MODE_FUSION {
        object.insert(
            "external_tool_policy".to_string(),
            Value::String(EXTERNAL_TOOL_POLICY_FORBIDDEN.to_string()),
        );
        object.insert(
            "external_callback_policy".to_string(),
            Value::String(EXTERNAL_CALLBACK_POLICY_FORBIDDEN.to_string()),
        );
        object.insert(
            "execution_mode".to_string(),
            Value::String(EXECUTION_MODE_BOUNDED_PARALLEL_PANEL.to_string()),
        );
        return;
    }

    if existing_external_tool_policy.is_none() {
        object.insert(
            "external_tool_policy".to_string(),
            Value::String(EXTERNAL_TOOL_POLICY_FORBIDDEN.to_string()),
        );
    }
    object.insert(
        "external_callback_policy".to_string(),
        Value::String(EXTERNAL_CALLBACK_POLICY_INHERITED.to_string()),
    );
    object.insert(
        "execution_mode".to_string(),
        Value::String(EXECUTION_MODE_SEQUENTIAL_RESUME.to_string()),
    );
}

pub(super) fn visible_internal_llm_tool_entries(node: &CompiledNode) -> Vec<&Value> {
    node.config
        .get("visible_internal_llm_tools")
        .or_else(|| node.config.get("visibleInternalLlmTools"))
        .and_then(Value::as_array)
        .map(|tools| {
            tools
                .iter()
                .filter(|tool| {
                    tool.get("type").and_then(Value::as_str) == Some(VISIBLE_INTERNAL_LLM_TOOL_TYPE)
                })
                .collect()
        })
        .unwrap_or_default()
}

pub(super) fn visible_internal_llm_tool_mode_value(tool: &Value) -> Option<String> {
    tool.get("tool_mode")
        .or_else(|| tool.get("toolMode"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

pub(super) fn visible_internal_llm_tool_connector_id(tool: &Value) -> Option<String> {
    tool.get("connector_id")
        .or_else(|| tool.get("connectorId"))
        .or_else(|| tool.get("tool_name"))
        .or_else(|| tool.get("name"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

pub(super) fn visible_internal_llm_tool_target_node_ids(tool: &Value) -> Vec<String> {
    let mut target_node_ids = tool
        .get("target_node_ids")
        .or_else(|| tool.get("targetNodeIds"))
        .and_then(Value::as_array)
        .map(|targets| {
            targets
                .iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if !target_node_ids.is_empty() {
        return target_node_ids;
    }

    if let Some(target_node_id) = tool
        .get("target_node_id")
        .or_else(|| tool.get("targetNodeId"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        target_node_ids.push(target_node_id.to_string());
    }
    target_node_ids
}

pub(super) fn visible_internal_llm_tool_branch_node_ids(
    target_node_id: &str,
    nodes: &BTreeMap<String, CompiledNode>,
    edges: &[CompiledEdge],
) -> BTreeSet<String> {
    let mut visited = BTreeSet::new();
    let mut queue = VecDeque::from([target_node_id.to_string()]);

    while let Some(node_id) = queue.pop_front() {
        if !visited.insert(node_id.clone()) {
            continue;
        }

        let Some(node) = nodes.get(&node_id) else {
            continue;
        };

        if node.node_type == TOOL_RESULT_NODE_TYPE {
            continue;
        }

        for edge in edges.iter().filter(|edge| edge.source == node_id) {
            if edge.source_handle.as_deref() == Some(ERROR_BRANCH_SOURCE_HANDLE)
                || visible_internal_llm_tool_connector_id_from_source_handle(
                    edge.source_handle.as_deref(),
                )
                .is_some()
            {
                continue;
            }

            queue.push_back(edge.target.clone());
        }
    }

    visited
}

pub(super) fn visible_internal_llm_tool_allows_internal_llm_node(tool: &Value) -> bool {
    tool.get("internal_llm_node_policy")
        .or_else(|| tool.get("internalLlmNodePolicy"))
        .and_then(Value::as_str)
        .map(str::trim)
        == Some(INTERNAL_LLM_NODE_POLICY_ALLOWED)
}

pub(super) fn visible_internal_llm_tool_external_tool_policy_value(tool: &Value) -> Option<String> {
    tool.get("external_tool_policy")
        .or_else(|| tool.get("externalToolPolicy"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

pub(super) fn visible_internal_llm_tools_array_mut(config: &mut Value) -> Option<&mut Vec<Value>> {
    let object = config.as_object_mut()?;
    let key = if object.contains_key("visible_internal_llm_tools") {
        "visible_internal_llm_tools"
    } else {
        "visibleInternalLlmTools"
    };

    object.get_mut(key).and_then(Value::as_array_mut)
}

pub(super) fn visible_internal_llm_tool_connector_ids(node: &CompiledNode) -> BTreeSet<String> {
    node.config
        .get("visible_internal_llm_tools")
        .or_else(|| node.config.get("visibleInternalLlmTools"))
        .and_then(Value::as_array)
        .map(|tools| {
            tools
                .iter()
                .filter_map(|tool| {
                    if tool.get("type").and_then(Value::as_str)
                        != Some(VISIBLE_INTERNAL_LLM_TOOL_TYPE)
                    {
                        return None;
                    }

                    tool.get("connector_id")
                        .or_else(|| tool.get("connectorId"))
                        .or_else(|| tool.get("tool_name"))
                        .or_else(|| tool.get("name"))
                        .and_then(Value::as_str)
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .map(str::to_string)
                })
                .collect()
        })
        .unwrap_or_default()
}

pub(super) fn visible_internal_llm_tools_enabled(node: &CompiledNode) -> bool {
    node.config
        .get("visible_internal_llm_tools_enabled")
        .or_else(|| node.config.get("visibleInternalLlmToolsEnabled"))
        .and_then(Value::as_bool)
        == Some(true)
}
