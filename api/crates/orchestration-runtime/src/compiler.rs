use std::collections::{BTreeMap, BTreeSet};

use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::answer_presentation::validate_answer_presentation;
use crate::compiled_plan::{
    CodeExecutorCapability, CodeIsolationProfile, CompileIssue, CompileIssueCode, CompiledBinding,
    CompiledCodeDependency, CompiledCodeRuntime, CompiledEdge, CompiledI18nTextRef,
    CompiledLlmRouteTarget, CompiledLlmRouting, CompiledLlmRuntime, CompiledNode, CompiledOutput,
    CompiledPlan, CompiledPluginRuntime, LlmDistributionRule, LlmRoutingMode,
};
use crate::output_schema::{history_messages_schema, output_schema_is_llm_context_messages};
use crate::payload_builder::PublicOutputContract;

mod code_runtime_config;
mod node_compilation;
mod selector_paths;
mod topology;
pub(crate) mod variable_aggregator_contract;

pub use node_compilation::js_dependency_lookup_key;
pub use topology::FlowCompiler;

pub fn ensure_plan_execution_contract(plan: &CompiledPlan) -> Result<()> {
    if let Some(issue) = topology::validate_variable_scope_contracts(&plan.nodes)
        .into_iter()
        .next()
    {
        bail!(issue.message);
    }
    if let Some(issue) = topology::validate_executable_node_types(&plan.nodes)
        .into_iter()
        .next()
    {
        bail!(issue.message);
    }
    Ok(())
}

const FLOW_SCHEMA_VERSION: &str = "1flowbase.flow/v2";

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FlowCompileContext {
    pub workspace_id: Option<uuid::Uuid>,
    pub provider_families: BTreeMap<String, FlowCompileProviderFamily>,
    pub provider_instances: BTreeMap<String, FlowCompileProviderInstance>,
    pub model_routing_policies: BTreeMap<(String, String), FlowCompileModelRoutingPolicy>,
    pub node_contributions: BTreeMap<String, FlowCompileNodeContribution>,
    pub js_dependencies: BTreeMap<String, FlowCompileJsDependency>,
    pub run_level_variables: Vec<FlowCompileRunLevelVariable>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlowCompileRunLevelVariable {
    pub selector: Vec<String>,
    pub value_type: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlowCompileModelRoutingPolicy {
    pub distribution_rule: LlmDistributionRule,
    pub provider_instance_ids: Vec<String>,
    pub excluded_provider_instance_ids: BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlowCompileJsDependency {
    pub alias: String,
    pub target: String,
    pub artifact_path: String,
    pub artifact_hash: String,
    pub integrity: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlowCompileProviderFamily {
    pub provider_code: String,
    pub protocol: String,
    pub is_ready: bool,
    pub available_models: BTreeSet<String>,
    pub allow_custom_models: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlowCompileProviderInstance {
    pub provider_instance_id: String,
    pub display_name: String,
    pub provider_code: String,
    pub protocol: String,
    pub is_ready: bool,
    pub is_runnable: bool,
    pub included_in_main: bool,
    pub available_models: BTreeSet<String>,
    pub allow_custom_models: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlowCompileNodeContribution {
    pub installation_id: uuid::Uuid,
    pub plugin_unique_identifier: String,
    pub package_id: String,
    pub plugin_id: String,
    pub plugin_version: String,
    pub contribution_code: String,
    pub node_shell: String,
    pub schema_version: String,
    pub contribution_checksum: String,
    pub compiled_contribution_hash: String,
    pub output_schema_snapshot: Vec<CompiledOutput>,
    pub side_effect_policy: String,
    pub dependency_status: String,
}
