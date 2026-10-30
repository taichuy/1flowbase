//! Configuration shared by container compilation and execution.
//! Limits come from the authored workflow, never from a guessed host capacity.

use std::collections::BTreeSet;

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::compiled_plan::CompiledNode;

pub const CONTAINER_START_NODE_TYPE: &str = "container_start";

pub fn is_container_type(node_type: &str) -> bool {
    matches!(node_type, "loop" | "iteration")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContainerErrorPolicy {
    #[default]
    Fail,
    Break,
    Continue,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LoopVariableDefinition {
    pub key: String,
    pub value_type: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContainerConfig {
    pub max_rounds: Option<u64>,
    pub round_timeout_ms: Option<u64>,
    pub parallelism: usize,
    pub flatten_output: bool,
    pub error_policy: ContainerErrorPolicy,
    pub loop_variables: Vec<LoopVariableDefinition>,
}

impl ContainerConfig {
    pub fn from_node(node: &CompiledNode) -> Result<Self> {
        if !is_container_type(&node.node_type) {
            bail!("node {} is not a container", node.node_id);
        }
        let config = node
            .config
            .as_object()
            .context("container config must be an object")?;
        let round_timeout_ms = match config.get("round_timeout_ms") {
            None | Some(Value::Null) => None,
            value => Some(positive_integer(value, "round_timeout_ms")?),
        };
        let max_rounds = if node.node_type == "loop" {
            Some(positive_integer(config.get("max_rounds"), "max_rounds")?)
        } else {
            None
        };
        let parallelism = match config.get("parallelism") {
            None => 1,
            value => usize::try_from(positive_integer(value, "parallelism")?)
                .context("parallelism cannot be represented by this runtime")?,
        };
        if node.node_type == "loop" && parallelism != 1 {
            bail!("loop variables require serial rounds (parallelism = 1)");
        }
        let flatten_output = match config.get("flatten_output") {
            None => false,
            Some(Value::Bool(value)) => *value,
            Some(_) => bail!("flatten_output must be a boolean"),
        };
        let error_policy = match config.get("error_policy") {
            None => ContainerErrorPolicy::Fail,
            Some(value) => serde_json::from_value(value.clone())
                .context("container error_policy must be fail, break, or continue")?,
        };
        let loop_variables: Vec<LoopVariableDefinition> = match config.get("loop_variables") {
            None => Vec::new(),
            Some(value) => serde_json::from_value(value.clone())
                .context("loop_variables must contain key and value_type declarations")?,
        };
        if node.node_type == "iteration" && !loop_variables.is_empty() {
            bail!("iteration items cannot declare shared loop_variables");
        }
        let mut keys = BTreeSet::new();
        for variable in &loop_variables {
            if variable.key.trim().is_empty()
                || variable.key.starts_with("__")
                || !keys.insert(&variable.key)
            {
                bail!("loop variable keys must be nonempty, unique, and not reserved");
            }
            if !matches!(
                variable.value_type.as_str(),
                "string" | "number" | "boolean" | "object" | "array"
            ) {
                bail!("unsupported loop variable type: {}", variable.value_type);
            }
        }
        Ok(Self {
            max_rounds,
            round_timeout_ms,
            parallelism,
            flatten_output,
            error_policy,
            loop_variables,
        })
    }
}

fn positive_integer(value: Option<&Value>, name: &str) -> Result<u64> {
    match value.and_then(Value::as_u64) {
        Some(value) if value > 0 => Ok(value),
        _ => bail!("{name} must be a configured positive integer"),
    }
}

/// Bindings evaluated after the body, rather than when entering the container.
pub fn is_container_body_binding(key: &str) -> bool {
    matches!(key, "output" | "variable_updates" | "break_condition")
}
