use std::collections::BTreeSet;

use anyhow::Result;
use serde_json::{json, Map, Value};

use crate::{
    compiled_plan::{CompiledNode, CompiledPlan},
    execution_state::NodeExecutionFailure,
    node_error_policy::{
        error_default_output, node_error_policy, NodeErrorPolicy, ERROR_BRANCH_SOURCE_HANDLE,
    },
};

use super::{
    branching::activate_downstream_nodes, first_output_key, project_node_variable_payload,
    LlmFailureProjection,
};

pub(super) struct NodeErrorPolicyApplication<'a> {
    pub(super) plan: &'a CompiledPlan,
    pub(super) failed_node_index: usize,
    pub(super) active_node_ids: &'a mut BTreeSet<String>,
    pub(super) variable_pool: &'a mut Map<String, Value>,
    pub(super) pending_failure: &'a mut Option<NodeExecutionFailure>,
    pub(super) node: &'a CompiledNode,
    pub(super) output_payload: &'a Value,
    pub(super) error_payload: Value,
    pub(super) failure_projection: LlmFailureProjection,
}

pub(super) fn apply_node_error_policy(
    application: NodeErrorPolicyApplication<'_>,
) -> Result<Option<NodeExecutionFailure>> {
    let NodeErrorPolicyApplication {
        plan,
        failed_node_index,
        active_node_ids,
        variable_pool,
        pending_failure,
        node,
        output_payload,
        error_payload,
        failure_projection,
    } = application;
    let failure = NodeExecutionFailure {
        node_id: node.node_id.clone(),
        node_alias: node.alias.clone(),
        error_payload: error_payload.clone(),
    };

    match node_error_policy(node) {
        NodeErrorPolicy::DefaultValue => {
            let default_output_payload = configured_default_output_payload(node);
            variable_pool.insert(
                node.node_id.clone(),
                project_node_variable_payload(node, &default_output_payload)?,
            );
            activate_downstream_nodes(plan, active_node_ids, node, None);
            Ok(None)
        }
        NodeErrorPolicy::ErrorBranch => {
            variable_pool.insert(
                node.node_id.clone(),
                project_node_variable_payload(node, output_payload)?,
            );
            if activate_downstream_nodes(
                plan,
                active_node_ids,
                node,
                Some(ERROR_BRANCH_SOURCE_HANDLE),
            ) {
                return Ok(None);
            }

            Ok(Some(failure))
        }
        NodeErrorPolicy::None => match failure_projection {
            LlmFailureProjection::NoNodeOutput => Ok(Some(failure)),
            LlmFailureProjection::FailedNodeOutput => {
                variable_pool.insert(
                    node.node_id.clone(),
                    project_node_variable_payload(node, output_payload)?,
                );
                Ok(Some(failure))
            }
            LlmFailureProjection::LegacyTerminalFallback => {
                variable_pool.insert(
                    node.node_id.clone(),
                    project_node_variable_payload(node, output_payload)?,
                );
                // Only the legacy edge-less plan shape may carry a locally
                // detected protocol failure into an existing terminal template.
                if !plan.edges.is_empty() {
                    return Ok(Some(failure));
                }

                let mut next_active_node_ids = active_node_ids.clone();
                activate_downstream_nodes(plan, &mut next_active_node_ids, node, None);
                if legacy_terminal_templates_can_receive_failure_output(
                    plan,
                    failed_node_index,
                    &next_active_node_ids,
                ) {
                    *active_node_ids = next_active_node_ids;
                    *pending_failure = Some(failure);
                    return Ok(None);
                }

                Ok(Some(failure))
            }
        },
    }
}

fn legacy_terminal_templates_can_receive_failure_output(
    plan: &CompiledPlan,
    failed_node_index: usize,
    active_node_ids: &BTreeSet<String>,
) -> bool {
    let mut has_terminal_template_node = false;
    for node_id in plan.topological_order.iter().skip(failed_node_index + 1) {
        if !active_node_ids.contains(node_id) {
            continue;
        }

        let Some(node) = plan.nodes.get(node_id) else {
            return false;
        };
        if !matches!(node.node_type.as_str(), "template_transform" | "answer") {
            return false;
        }
        has_terminal_template_node = true;
    }
    has_terminal_template_node
}

fn configured_default_output_payload(node: &CompiledNode) -> Value {
    match error_default_output(node) {
        Some(value @ Value::Object(_)) => value,
        Some(value) => json!({ first_output_key(node): value }),
        None => json!({}),
    }
}
