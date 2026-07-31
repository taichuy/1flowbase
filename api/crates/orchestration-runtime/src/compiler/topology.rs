use std::collections::{BTreeMap, BTreeSet, VecDeque};

use crate::node_error_policy::{node_uses_error_branch, ERROR_BRANCH_SOURCE_HANDLE};

use super::node_compilation::compile_node;
use super::*;

const VISIBLE_INTERNAL_LLM_TOOL_TYPE: &str = "visible_internal_llm_tool";
const VISIBLE_INTERNAL_LLM_TOOL_SOURCE_HANDLE_PREFIX: &str = "visible_internal_llm_tool:";
const TOOL_RESULT_NODE_TYPE: &str = "tool_result";
const INTERNAL_LLM_NODE_POLICY_ALLOWED: &str = "allowed";
const TOOL_MODE_AGENT: &str = "agent";
const TOOL_MODE_FUSION: &str = "fusion";
const EXTERNAL_TOOL_POLICY_FORBIDDEN: &str = "forbidden";
const EXTERNAL_TOOL_POLICY_INHERITED: &str = "inherited";
const EXTERNAL_CALLBACK_POLICY_FORBIDDEN: &str = "forbidden";
const EXTERNAL_CALLBACK_POLICY_INHERITED: &str = "inherited";
const EXECUTION_MODE_SEQUENTIAL_RESUME: &str = "sequential_resume";
const EXECUTION_MODE_BOUNDED_PARALLEL_PANEL: &str = "bounded_parallel_panel";

const RUN_LEVEL_VARIABLE_NAMESPACES: &[&str] = &[
    "sys",
    "env",
    "conversation",
    "trigger",
    VISIBLE_INTERNAL_LLM_TOOL_TYPE,
];

const EXECUTABLE_NODE_TYPES: &[&str] = &[
    "start",
    "workflow_start",
    "workflow_end",
    "answer",
    "llm",
    "if_else",
    "code",
    "template_transform",
    "http_request",
    "tool",
    "tool_result",
    "data_model_list",
    "data_model_get",
    "data_model_create",
    "data_model_update",
    "data_model_delete",
    "sql",
    "variable_assigner",
    "human_input",
    "plugin_node",
    "unresolved_node",
];

type NodeTopologyBuild = (
    BTreeMap<String, CompiledNode>,
    Vec<CompiledEdge>,
    Vec<String>,
    Vec<CompileIssue>,
);

pub struct FlowCompiler;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FlowDocumentKind {
    AgentFlow,
    Workflow,
}

impl FlowCompiler {
    pub fn compile(
        flow_id: uuid::Uuid,
        draft_id: &str,
        document: &Value,
        context: &FlowCompileContext,
    ) -> Result<CompiledPlan> {
        Self::compile_for_kind(
            FlowDocumentKind::AgentFlow,
            flow_id,
            draft_id,
            document,
            context,
        )
    }

    pub fn compile_workflow(
        flow_id: uuid::Uuid,
        draft_id: &str,
        document: &Value,
        context: &FlowCompileContext,
    ) -> Result<CompiledPlan> {
        Self::compile_for_kind(
            FlowDocumentKind::Workflow,
            flow_id,
            draft_id,
            document,
            context,
        )
    }

    fn compile_for_kind(
        document_kind: FlowDocumentKind,
        flow_id: uuid::Uuid,
        draft_id: &str,
        document: &Value,
        context: &FlowCompileContext,
    ) -> Result<CompiledPlan> {
        let schema_version = document
            .get("schemaVersion")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow!("schemaVersion missing"))?
            .to_string();
        if schema_version != FLOW_SCHEMA_VERSION {
            bail!("unsupported flow schemaVersion: {schema_version}");
        }
        let (nodes, edges, topological_order, mut compile_issues) =
            build_nodes_and_topology(document, context)?;
        validate_document_kind(document_kind, &nodes)?;

        let mut plan = CompiledPlan {
            flow_id,
            source_draft_id: draft_id.to_string(),
            schema_version,
            topological_order,
            edges,
            nodes,
            compile_issues: Vec::new(),
        };
        compile_issues.extend(validate_answer_presentation(&plan));
        plan.compile_issues = compile_issues;

        Ok(plan)
    }
}

fn validate_document_kind(
    document_kind: FlowDocumentKind,
    nodes: &BTreeMap<String, CompiledNode>,
) -> Result<()> {
    match document_kind {
        FlowDocumentKind::AgentFlow => validate_agent_flow_node_family(nodes),
        FlowDocumentKind::Workflow => validate_workflow_node_family(nodes),
    }
}

fn validate_agent_flow_node_family(nodes: &BTreeMap<String, CompiledNode>) -> Result<()> {
    for workflow_node_type in ["workflow_start", "workflow_end"] {
        if let Some(node) = nodes
            .values()
            .find(|node| node.node_type == workflow_node_type)
        {
            bail!(
                "agent_flow document cannot contain {} node {}",
                node.node_type,
                node.node_id
            );
        }
    }

    Ok(())
}

fn validate_workflow_node_family(nodes: &BTreeMap<String, CompiledNode>) -> Result<()> {
    let mut workflow_start_count = 0;
    let mut workflow_end_count = 0;

    for node in nodes.values() {
        match node.node_type.as_str() {
            "start" | "answer" => {
                bail!(
                    "workflow document cannot contain {} node {}",
                    node.node_type,
                    node.node_id
                );
            }
            "workflow_start" => workflow_start_count += 1,
            "workflow_end" => workflow_end_count += 1,
            _ => {}
        }
    }

    if workflow_start_count != 1 {
        bail!("workflow document must contain exactly one workflow_start node");
    }

    if workflow_end_count == 0 {
        bail!("workflow document must contain at least one workflow_end node");
    }

    Ok(())
}

pub(super) fn build_nodes_and_topology(
    document: &Value,
    context: &FlowCompileContext,
) -> Result<NodeTopologyBuild> {
    let node_values = document
        .get("graph")
        .and_then(|graph| graph.get("nodes"))
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("graph.nodes missing"))?;

    let edge_values = document
        .get("graph")
        .and_then(|graph| graph.get("edges"))
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("graph.edges missing"))?;

    let mut nodes = BTreeMap::new();
    let mut node_order = Vec::with_capacity(node_values.len());
    let mut compile_issues = Vec::new();

    for node in node_values {
        let compiled = compile_node(node, context, &mut compile_issues)?;

        if nodes.contains_key(&compiled.node_id) {
            bail!("duplicate node id: {}", compiled.node_id);
        }

        node_order.push(compiled.node_id.clone());
        nodes.insert(compiled.node_id.clone(), compiled);
    }

    let mut adjacency = BTreeMap::<String, Vec<String>>::new();
    let mut indegree = BTreeMap::<String, usize>::new();
    let mut compiled_edges = Vec::with_capacity(edge_values.len());

    for node_id in &node_order {
        adjacency.insert(node_id.clone(), Vec::new());
        indegree.insert(node_id.clone(), 0);
    }

    let if_else_source_handles = collect_if_else_branch_source_handles(&nodes)?;

    for edge in edge_values {
        let edge_id = edge
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or("unknown-edge");
        let source = edge
            .get("source")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow!("edge {edge_id} missing source"))?;
        let target = edge
            .get("target")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow!("edge {edge_id} missing target"))?;

        if !nodes.contains_key(source) {
            bail!("edge {edge_id} references unknown source node: {source}");
        }

        if !nodes.contains_key(target) {
            bail!("edge {edge_id} references unknown target node: {target}");
        }

        let source_handle = edge_handle(edge, "sourceHandle", "source_handle")?;
        let target_handle = edge_handle(edge, "targetHandle", "target_handle")?;
        validate_edge_source_handle(
            edge_id,
            nodes
                .get(source)
                .expect("validated source node must exist before edge compilation"),
            source_handle.as_deref(),
            &if_else_source_handles,
        )?;
        compiled_edges.push(CompiledEdge {
            edge_id: edge_id.to_string(),
            source: source.to_string(),
            target: target.to_string(),
            source_handle,
            target_handle,
        });

        let dependency_node_ids = &mut nodes
            .get_mut(target)
            .expect("validated target node must exist")
            .dependency_node_ids;
        if !dependency_node_ids.iter().any(|node_id| node_id == source) {
            dependency_node_ids.push(source.to_string());
        }

        let downstream_node_ids = &mut nodes
            .get_mut(source)
            .expect("validated source node must exist")
            .downstream_node_ids;
        if !downstream_node_ids.iter().any(|node_id| node_id == target) {
            downstream_node_ids.push(target.to_string());
        }

        adjacency
            .get_mut(source)
            .expect("validated source adjacency must exist")
            .push(target.to_string());
        *indegree
            .get_mut(target)
            .expect("validated target indegree must exist") += 1;
    }

    compile_issues.extend(materialize_visible_internal_llm_tool_targets(
        &mut nodes,
        &compiled_edges,
    ));
    compile_issues.extend(validate_visible_internal_llm_tool_branches(
        &nodes,
        &compiled_edges,
    ));

    for node in nodes.values_mut() {
        node.dependency_node_ids
            .sort_by_key(|node_id| node_order_index(&node_order, node_id));
        node.downstream_node_ids
            .sort_by_key(|node_id| node_order_index(&node_order, node_id));
    }

    let mut queue = VecDeque::new();

    for node_id in &node_order {
        if indegree.get(node_id).copied().unwrap_or_default() == 0 {
            queue.push_back(node_id.clone());
        }
    }

    let mut topological_order = Vec::with_capacity(node_order.len());

    while let Some(node_id) = queue.pop_front() {
        topological_order.push(node_id.clone());

        if let Some(neighbors) = adjacency.get(&node_id) {
            for neighbor in neighbors {
                let remaining = indegree
                    .get_mut(neighbor)
                    .expect("neighbor indegree must exist after validation");
                *remaining -= 1;

                if *remaining == 0 {
                    queue.push_back(neighbor.clone());
                }
            }
        }
    }

    if topological_order.len() != node_order.len() {
        let visited = topological_order.iter().cloned().collect::<BTreeSet<_>>();
        let cycle_nodes = node_order
            .into_iter()
            .filter(|node_id| !visited.contains(node_id))
            .collect::<Vec<_>>();
        bail!(
            "graph contains a cycle involving nodes: {}",
            cycle_nodes.join(", ")
        );
    }

    compile_issues.extend(validate_variable_scope_contracts(&nodes));
    compile_issues.extend(validate_llm_context_policies(&nodes));
    compile_issues.extend(validate_executable_node_types(&nodes));

    Ok((nodes, compiled_edges, topological_order, compile_issues))
}

pub(super) fn validate_variable_scope_contracts(
    nodes: &BTreeMap<String, CompiledNode>,
) -> Vec<CompileIssue> {
    let mut issues = Vec::new();

    for node in nodes.values() {
        for selector in node
            .bindings
            .values()
            .flat_map(|binding| binding.selector_paths.iter())
        {
            validate_variable_selector(nodes, node, selector, &mut issues);
        }

        if let Some(selector) = context_policy_selector(node) {
            validate_variable_selector(nodes, node, &selector, &mut issues);
        }

        if node.node_type == "llm" {
            match node.protocol_context_reference() {
                Ok(Some(reference)) => {
                    validate_variable_selector(nodes, node, reference.selector_path(), &mut issues);
                    validate_protocol_context_reference(nodes, node, &reference, &mut issues);
                }
                Ok(None) => {}
                Err(error) => issues.push(CompileIssue {
                    node_id: node.node_id.clone(),
                    code: CompileIssueCode::InvalidLlmContextSelector,
                    message: format!(
                        "node {} protocol_context is not a valid VariableReference: {error}",
                        node.node_id
                    ),
                }),
            }
        }
    }

    issues
}

fn validate_protocol_context_reference(
    nodes: &BTreeMap<String, CompiledNode>,
    target: &CompiledNode,
    reference: &crate::compiled_plan::VariableReference,
    issues: &mut Vec<CompileIssue>,
) {
    if reference.is_system_protocol_context() {
        return;
    }

    let selector = reference.selector_path();
    let Some(source_id) = selector.first() else {
        return;
    };
    if RUN_LEVEL_VARIABLE_NAMESPACES.contains(&source_id.as_str()) {
        issues.push(CompileIssue {
            node_id: target.node_id.clone(),
            code: CompileIssueCode::InvalidLlmContextSelector,
            message: format!(
                "node {} protocol_context {} must reference sys.protocol_context or an upstream Start/Code JSON output",
                target.node_id,
                selector.join(".")
            ),
        });
        return;
    }

    let Some(source) = nodes.get(source_id) else {
        return;
    };
    if !node_depends_on(nodes, &target.node_id, source_id) {
        return;
    }
    if !matches!(
        source.node_type.as_str(),
        "start" | "workflow_start" | "code"
    ) {
        issues.push(CompileIssue {
            node_id: target.node_id.clone(),
            code: CompileIssueCode::InvalidLlmContextSelector,
            message: format!(
                "node {} protocol_context {} must reference sys.protocol_context or an upstream Start/Code JSON output",
                target.node_id,
                selector.join(".")
            ),
        });
        return;
    }

    let Some(output) = output_for_selector(nodes, selector) else {
        issues.push(CompileIssue {
            node_id: target.node_id.clone(),
            code: CompileIssueCode::InvalidLlmContextSelector,
            message: format!(
                "node {} protocol_context {} is not a declared Start/Code output",
                target.node_id,
                selector.join(".")
            ),
        });
        return;
    };
    if output.value_type != "json" {
        issues.push(CompileIssue {
            node_id: target.node_id.clone(),
            code: CompileIssueCode::IncompatibleLlmContextSchema,
            message: format!(
                "node {} protocol_context {} must reference a JSON output",
                target.node_id,
                selector.join(".")
            ),
        });
    }
}

fn validate_variable_selector(
    nodes: &BTreeMap<String, CompiledNode>,
    target: &CompiledNode,
    selector: &[String],
    issues: &mut Vec<CompileIssue>,
) {
    let Some(source_id) = selector.first() else {
        return;
    };
    if RUN_LEVEL_VARIABLE_NAMESPACES.contains(&source_id.as_str()) {
        return;
    }

    let Some(source) = nodes.get(source_id) else {
        issues.push(CompileIssue {
            node_id: target.node_id.clone(),
            code: CompileIssueCode::InvalidSelectorSource,
            message: format!(
                "node {} selector {} references an unknown source node",
                target.node_id,
                selector.join(".")
            ),
        });
        return;
    };

    if !node_depends_on(nodes, &target.node_id, source_id) {
        issues.push(CompileIssue {
            node_id: target.node_id.clone(),
            code: CompileIssueCode::SelectorSourceNotReachable,
            message: format!(
                "node {} selector source {} is not reachable through connected upstream edges",
                target.node_id, source_id
            ),
        });
        return;
    }

    let Some(output_key) = selector.get(1) else {
        return;
    };
    if matches!(source.node_type.as_str(), "start" | "workflow_start") {
        return;
    }
    if source
        .outputs
        .iter()
        .any(|output| output.key == *output_key || output.selector.first() == Some(output_key))
    {
        return;
    }

    issues.push(CompileIssue {
        node_id: target.node_id.clone(),
        code: CompileIssueCode::SelectorOutputNotFound,
        message: format!(
            "node {} selector {} is missing from source node {} output contract",
            target.node_id,
            selector.join("."),
            source.node_id
        ),
    });
}

fn node_depends_on(
    nodes: &BTreeMap<String, CompiledNode>,
    node_id: &str,
    dependency_node_id: &str,
) -> bool {
    let mut stack = vec![node_id];
    let mut visited = BTreeSet::new();

    while let Some(current) = stack.pop() {
        if !visited.insert(current.to_string()) {
            continue;
        }
        let Some(node) = nodes.get(current) else {
            continue;
        };
        for dependency in &node.dependency_node_ids {
            if dependency == dependency_node_id {
                return true;
            }
            stack.push(dependency);
        }
    }

    false
}

pub(super) fn validate_executable_node_types(
    nodes: &BTreeMap<String, CompiledNode>,
) -> Vec<CompileIssue> {
    nodes
        .values()
        .filter(|node| !EXECUTABLE_NODE_TYPES.contains(&node.node_type.as_str()))
        .map(|node| CompileIssue {
            node_id: node.node_id.clone(),
            code: CompileIssueCode::UnsupportedNodeType,
            message: format!(
                "node {} type {} is not executable by the orchestration runtime",
                node.node_id, node.node_type
            ),
        })
        .collect()
}

fn edge_handle(edge: &Value, camel_key: &str, snake_key: &str) -> Result<Option<String>> {
    let value = edge.get(camel_key).or_else(|| edge.get(snake_key));

    match value {
        Some(Value::String(handle)) if !handle.trim().is_empty() => Ok(Some(handle.to_string())),
        Some(Value::String(_)) | Some(Value::Null) | None => Ok(None),
        Some(_) => bail!("edge handle {camel_key} must be a string or null"),
    }
}

fn validate_edge_source_handle(
    edge_id: &str,
    source_node: &CompiledNode,
    source_handle: Option<&str>,
    if_else_source_handles: &BTreeMap<String, BTreeSet<String>>,
) -> Result<()> {
    if source_handle == Some(ERROR_BRANCH_SOURCE_HANDLE) {
        if node_uses_error_branch(source_node) {
            return Ok(());
        }

        bail!(
            "edge {edge_id} uses error sourceHandle on node {} without error_branch policy",
            source_node.node_id
        );
    }

    if let Some(connector_id) =
        visible_internal_llm_tool_connector_id_from_source_handle(source_handle)
    {
        if source_node.node_type != "llm" {
            bail!(
                "edge {edge_id} uses visible internal LLM tool sourceHandle on non-LLM node {}",
                source_node.node_id
            );
        }
        if !visible_internal_llm_tools_enabled(source_node) {
            bail!(
                "edge {edge_id} uses visible internal LLM tool sourceHandle on node {} without mount tools enabled",
                source_node.node_id
            );
        }
        if !visible_internal_llm_tool_connector_ids(source_node).contains(connector_id) {
            bail!(
                "edge {edge_id} references unknown visible internal LLM tool connector {connector_id} for node {}",
                source_node.node_id
            );
        }

        return Ok(());
    }

    if source_node.node_type != "if_else" {
        return Ok(());
    }

    let Some(source_handle) = source_handle else {
        bail!(
            "edge {edge_id} from if_else node {} missing sourceHandle",
            source_node.node_id
        );
    };
    let source_handles = if_else_source_handles
        .get(&source_node.node_id)
        .ok_or_else(|| {
            anyhow!(
                "if_else node {} missing branch contract",
                source_node.node_id
            )
        })?;

    if !source_handles.contains(source_handle) {
        bail!(
            "edge {edge_id} references unknown if_else sourceHandle {source_handle} for node {}",
            source_node.node_id
        );
    }

    Ok(())
}

fn visible_internal_llm_tool_connector_id_from_source_handle(
    source_handle: Option<&str>,
) -> Option<&str> {
    source_handle?.strip_prefix(VISIBLE_INTERNAL_LLM_TOOL_SOURCE_HANDLE_PREFIX)
}

fn visible_internal_llm_tool_source_handle(connector_id: &str) -> String {
    format!("{VISIBLE_INTERNAL_LLM_TOOL_SOURCE_HANDLE_PREFIX}{connector_id}")
}

fn collect_if_else_branch_source_handles(
    nodes: &BTreeMap<String, CompiledNode>,
) -> Result<BTreeMap<String, BTreeSet<String>>> {
    let mut source_handles = BTreeMap::new();

    for node in nodes.values().filter(|node| node.node_type == "if_else") {
        source_handles.insert(node.node_id.clone(), if_else_branch_source_handles(node)?);
    }

    Ok(source_handles)
}

fn if_else_branch_source_handles(node: &CompiledNode) -> Result<BTreeSet<String>> {
    let binding = node
        .bindings
        .get("branches")
        .ok_or_else(|| anyhow!("if_else node {} missing branches binding", node.node_id))?;
    if binding.kind != "if_else_branches" {
        bail!(
            "if_else node {} branches binding must be if_else_branches",
            node.node_id
        );
    }

    let branches = binding
        .raw_value
        .get("branches")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("if_else node {} branches must be an array", node.node_id))?;
    let mut source_handles = BTreeSet::new();
    let mut else_branch_count = 0;
    for branch in branches {
        let source_handle = branch
            .get("sourceHandle")
            .or_else(|| branch.get("source_handle"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|handle| !handle.is_empty())
            .ok_or_else(|| anyhow!("if_else node {} branch missing sourceHandle", node.node_id))?;
        if branch.get("kind").and_then(Value::as_str) == Some("else") {
            else_branch_count += 1;
        } else if !condition_group_has_complete_rules(
            branch.get("condition").unwrap_or(&Value::Null),
        ) {
            bail!(
                "if_else node {} branch {source_handle} must include a complete condition",
                node.node_id
            );
        }

        if !source_handles.insert(source_handle.to_string()) {
            bail!(
                "if_else node {} duplicate branch sourceHandle {source_handle}",
                node.node_id
            );
        }
    }

    if else_branch_count == 0 {
        bail!("if_else node {} must include an else branch", node.node_id);
    }

    if else_branch_count > 1 {
        bail!(
            "if_else node {} must include only one else branch",
            node.node_id
        );
    }

    Ok(source_handles)
}

fn condition_group_has_complete_rules(group: &Value) -> bool {
    let Some(conditions) = group.get("conditions").and_then(Value::as_array) else {
        return false;
    };

    !conditions.is_empty()
        && conditions
            .iter()
            .all(condition_expression_has_required_input)
}

fn condition_expression_has_required_input(condition: &Value) -> bool {
    if condition
        .get("conditions")
        .and_then(Value::as_array)
        .is_some()
    {
        return condition_group_has_complete_rules(condition);
    }

    condition_rule_has_required_input(condition)
}

fn condition_rule_has_required_input(rule: &Value) -> bool {
    if !selector_has_required_input(rule.get("left").unwrap_or(&Value::Null)) {
        return false;
    }

    if matches!(
        rule.get("comparator").and_then(Value::as_str),
        Some("exists" | "empty")
    ) {
        return true;
    }

    let Some(right) = rule.get("right") else {
        return false;
    };

    right.get("kind").and_then(Value::as_str) != Some("selector")
        || selector_has_required_input(right.get("selector").unwrap_or(&Value::Null))
}

fn selector_has_required_input(selector: &Value) -> bool {
    selector.as_array().is_some_and(|segments| {
        segments.len() >= 2
            && segments.iter().all(|segment| {
                segment
                    .as_str()
                    .is_some_and(|value| !value.trim().is_empty())
            })
    })
}

fn validate_llm_context_policies(nodes: &BTreeMap<String, CompiledNode>) -> Vec<CompileIssue> {
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

fn materialize_visible_internal_llm_tool_targets(
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

fn validate_visible_internal_llm_tool_branches(
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
                    node_id: node.node_id.clone(),
                    code: CompileIssueCode::InvalidVisibleInternalLlmTool,
                    message: format!(
                        "node {} visible_internal_llm_tool connector {connector_id} is missing a reachable tool_result node",
                        node.node_id
                    ),
                });
            } else if tool_result_node_ids.len() > 1 {
                issues.push(CompileIssue {
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

fn materialize_visible_internal_llm_tool_policy(tool: &mut Value) {
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

fn visible_internal_llm_tool_entries(node: &CompiledNode) -> Vec<&Value> {
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

fn visible_internal_llm_tool_mode_value(tool: &Value) -> Option<String> {
    tool.get("tool_mode")
        .or_else(|| tool.get("toolMode"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn visible_internal_llm_tool_connector_id(tool: &Value) -> Option<String> {
    tool.get("connector_id")
        .or_else(|| tool.get("connectorId"))
        .or_else(|| tool.get("tool_name"))
        .or_else(|| tool.get("name"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn visible_internal_llm_tool_target_node_ids(tool: &Value) -> Vec<String> {
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

fn visible_internal_llm_tool_branch_node_ids(
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

fn visible_internal_llm_tool_allows_internal_llm_node(tool: &Value) -> bool {
    tool.get("internal_llm_node_policy")
        .or_else(|| tool.get("internalLlmNodePolicy"))
        .and_then(Value::as_str)
        .map(str::trim)
        == Some(INTERNAL_LLM_NODE_POLICY_ALLOWED)
}

fn visible_internal_llm_tool_external_tool_policy_value(tool: &Value) -> Option<String> {
    tool.get("external_tool_policy")
        .or_else(|| tool.get("externalToolPolicy"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn visible_internal_llm_tools_array_mut(config: &mut Value) -> Option<&mut Vec<Value>> {
    let object = config.as_object_mut()?;
    let key = if object.contains_key("visible_internal_llm_tools") {
        "visible_internal_llm_tools"
    } else {
        "visibleInternalLlmTools"
    };

    object.get_mut(key).and_then(Value::as_array_mut)
}

fn visible_internal_llm_tool_connector_ids(node: &CompiledNode) -> BTreeSet<String> {
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

fn visible_internal_llm_tools_enabled(node: &CompiledNode) -> bool {
    node.config
        .get("visible_internal_llm_tools_enabled")
        .or_else(|| node.config.get("visibleInternalLlmToolsEnabled"))
        .and_then(Value::as_bool)
        == Some(true)
}

fn context_policy_selector(node: &CompiledNode) -> Option<Vec<String>> {
    node.config
        .get("context_policy")
        .and_then(|policy| policy.get("context_selector"))
        .and_then(Value::as_array)
        .map(|selector| {
            selector
                .iter()
                .filter_map(Value::as_str)
                .map(ToOwned::to_owned)
                .collect::<Vec<_>>()
        })
        .filter(|selector| selector.len() >= 2)
}

fn output_for_selector(
    nodes: &BTreeMap<String, CompiledNode>,
    selector: &[String],
) -> Option<CompiledOutput> {
    if selector.len() == crate::compiled_plan::SYSTEM_PROTOCOL_CONTEXT_SELECTOR.len()
        && selector
            .iter()
            .zip(crate::compiled_plan::SYSTEM_PROTOCOL_CONTEXT_SELECTOR)
            .all(|(actual, expected)| actual.as_str() == expected)
    {
        return Some(CompiledOutput {
            key: "protocol_context".to_string(),
            title: "sys.protocol_context".to_string(),
            value_type: "json".to_string(),
            selector: Vec::new(),
            json_schema: None,
        });
    }

    let node_id = selector.first()?;
    let selector_tail = selector.get(1..)?;
    let output_key = selector_tail.first()?;
    let node = nodes.get(node_id)?;

    if node.node_type == "start" && output_key == "history" {
        return Some(CompiledOutput {
            key: "history".to_string(),
            title: "userinput.history".to_string(),
            value_type: "array".to_string(),
            selector: Vec::new(),
            json_schema: Some(history_messages_schema()),
        });
    }

    if matches!(node.node_type.as_str(), "start" | "workflow_start") {
        if let Some(output) = start_input_field_output(node, output_key) {
            return Some(output);
        }
    }

    node.outputs
        .iter()
        .find(|output| {
            output.selector == selector_tail
                || (selector_tail.len() == 1 && output.key == *output_key)
        })
        .cloned()
}

fn start_input_field_output(node: &CompiledNode, output_key: &str) -> Option<CompiledOutput> {
    let input_fields = node.config.get("input_fields")?.as_array()?;

    input_fields.iter().find_map(|field| {
        let key = field.get("key")?.as_str()?.trim();
        if key.is_empty() || key != output_key {
            return None;
        }

        let value_type = field
            .get("valueType")
            .or_else(|| field.get("value_type"))
            .and_then(Value::as_str)
            .filter(|value_type| !value_type.trim().is_empty())
            .unwrap_or("string");

        Some(CompiledOutput {
            key: key.to_string(),
            title: format!("userinput.{key}"),
            value_type: value_type.to_string(),
            selector: Vec::new(),
            json_schema: None,
        })
    })
}

fn node_order_index(node_order: &[String], node_id: &str) -> usize {
    node_order
        .iter()
        .position(|candidate| candidate == node_id)
        .unwrap_or(usize::MAX)
}
