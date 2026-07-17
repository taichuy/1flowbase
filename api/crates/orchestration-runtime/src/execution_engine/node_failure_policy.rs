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
};

pub(super) struct NodeErrorPolicyApplication<'a> {
    pub(super) plan: &'a CompiledPlan,
    pub(super) active_node_ids: &'a mut BTreeSet<String>,
    pub(super) variable_pool: &'a mut Map<String, Value>,
    pub(super) node: &'a CompiledNode,
    pub(super) output_payload: &'a Value,
    pub(super) error_payload: Value,
}

pub(super) fn apply_node_error_policy(
    application: NodeErrorPolicyApplication<'_>,
) -> Result<Option<NodeExecutionFailure>> {
    let NodeErrorPolicyApplication {
        plan,
        active_node_ids,
        variable_pool,
        node,
        output_payload,
        error_payload,
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
        NodeErrorPolicy::None => Ok(Some(failure)),
    }
}

fn configured_default_output_payload(node: &CompiledNode) -> Value {
    match error_default_output(node) {
        Some(value @ Value::Object(_)) => value,
        Some(value) => json!({ first_output_key(node): value }),
        None => json!({}),
    }
}
