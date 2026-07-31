use std::collections::{BTreeMap, BTreeSet};

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::{
    errors::ControlPlaneError,
    ports::{ModelProviderRepository, NodeContributionRepository},
};

pub const AGENT_FLOW_TEMPLATE_SCHEMA_VERSION: &str = "1flowbase.application-template/v1";
pub const UNRESOLVED_NODE_TYPE: &str = "unresolved_node";
const MISSING_DEPENDENCY_STATUS: &str = "missing_dependency";
const READY_STATUS: &str = "ready";

const BUILTIN_NODE_TYPES: &[&str] = &[
    "start",
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
    "variable_assigner",
    "human_input",
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentFlowTemplatePackage {
    pub schema_version: String,
    pub application: AgentFlowTemplateApplication,
    pub flow_document: Value,
    #[serde(default)]
    pub dependencies: Vec<AgentFlowTemplateDependency>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentFlowTemplateApplication {
    pub application_type: String,
    pub name: String,
    pub description: String,
    pub icon: Option<String>,
    pub icon_type: Option<String>,
    pub icon_background: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentFlowTemplateDependency {
    pub kind: String,
    pub node_id: Option<String>,
    pub node_type: Option<String>,
    pub config_version: Option<i64>,
    pub provider_code: Option<String>,
    pub model_id: Option<String>,
    pub plugin_id: Option<String>,
    pub plugin_version: Option<String>,
    pub contribution_code: Option<String>,
    pub node_shell: Option<String>,
    pub schema_version: Option<String>,
    pub plugin_unique_identifier: Option<String>,
    pub package_id: Option<String>,
    pub contribution_checksum: Option<String>,
    pub compiled_contribution_hash: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentFlowTemplateDependencyStatus {
    pub dependency: AgentFlowTemplateDependency,
    pub status: String,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentFlowTemplateUnresolvedNode {
    pub node_id: String,
    pub alias: String,
    pub original_type: String,
    pub dependency_status: String,
    pub reason: String,
    pub original_node: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentFlowTemplatePreview {
    pub schema_version: String,
    pub application: AgentFlowTemplateApplication,
    pub dependencies: Vec<AgentFlowTemplateDependencyStatus>,
    pub unresolved_nodes: Vec<AgentFlowTemplateUnresolvedNode>,
    pub document: Value,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AgentFlowTemplateResourceSnapshot {
    pub model_providers: Vec<AgentFlowTemplateModelProviderResource>,
    pub node_contributions: Vec<AgentFlowTemplateNodeContributionResource>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentFlowTemplateModelProviderResource {
    pub provider_code: String,
    pub ready: bool,
    pub allow_custom_models: bool,
    pub model_ids: BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentFlowTemplateNodeContributionResource {
    pub plugin_id: String,
    pub plugin_version: String,
    pub contribution_code: String,
    pub node_shell: String,
    pub schema_version: String,
    pub plugin_unique_identifier: String,
    pub package_id: String,
    pub contribution_checksum: String,
    pub compiled_contribution_hash: String,
    pub dependency_status: String,
}

impl AgentFlowTemplateResourceSnapshot {
    pub fn from_ready_model(provider_code: &str, model_id: &str) -> Self {
        Self {
            model_providers: vec![AgentFlowTemplateModelProviderResource {
                provider_code: provider_code.to_string(),
                ready: true,
                allow_custom_models: false,
                model_ids: BTreeSet::from([model_id.to_string()]),
            }],
            node_contributions: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NodeResolution {
    status: String,
    reason: Option<String>,
}

pub async fn load_agent_flow_template_resources<R>(
    repository: &R,
    workspace_id: Uuid,
) -> Result<AgentFlowTemplateResourceSnapshot>
where
    R: ModelProviderRepository + NodeContributionRepository,
{
    let mut model_providers = BTreeMap::<String, AgentFlowTemplateModelProviderResource>::new();
    for instance in repository.list_instances(workspace_id).await? {
        if !instance.included_in_main {
            continue;
        }

        let configured_model_ids = instance
            .configured_models
            .iter()
            .filter(|model| model.enabled)
            .map(|model| model.model_id.clone())
            .chain(instance.enabled_model_ids.iter().cloned())
            .collect::<BTreeSet<_>>();
        let cached_model_ids = match repository.get_catalog_cache(instance.id).await? {
            Some(cache) => model_ids_from_catalog_json(&cache.models_json),
            None => BTreeSet::new(),
        };
        let allow_custom_models = instance.enabled_model_ids.is_empty();
        let entry = model_providers
            .entry(instance.provider_code.clone())
            .or_insert(AgentFlowTemplateModelProviderResource {
                provider_code: instance.provider_code.clone(),
                ready: false,
                allow_custom_models: false,
                model_ids: BTreeSet::new(),
            });

        entry.ready |= instance.status == domain::ModelProviderInstanceStatus::Ready;
        entry.allow_custom_models |= allow_custom_models;
        entry.model_ids.extend(configured_model_ids);
        entry.model_ids.extend(cached_model_ids);
    }

    let node_contributions = repository
        .list_node_contributions(workspace_id)
        .await?
        .into_iter()
        .map(|entry| AgentFlowTemplateNodeContributionResource {
            plugin_id: entry.plugin_id,
            plugin_version: entry.plugin_version,
            contribution_code: entry.contribution_code,
            node_shell: entry.node_shell,
            schema_version: entry.schema_version,
            plugin_unique_identifier: entry.plugin_unique_identifier,
            package_id: entry.package_id,
            contribution_checksum: entry.contribution_checksum,
            compiled_contribution_hash: entry.compiled_contribution_hash,
            dependency_status: entry.dependency_status.as_str().to_string(),
        })
        .collect();

    Ok(AgentFlowTemplateResourceSnapshot {
        model_providers: model_providers.into_values().collect(),
        node_contributions,
    })
}

pub fn build_agent_flow_template_package(
    application: &domain::ApplicationRecord,
    flow_document: &Value,
) -> AgentFlowTemplatePackage {
    let sanitized_document = sanitize_template_value(flow_document);

    AgentFlowTemplatePackage {
        schema_version: AGENT_FLOW_TEMPLATE_SCHEMA_VERSION.to_string(),
        application: AgentFlowTemplateApplication {
            application_type: application.application_type.as_str().to_string(),
            name: application.name.clone(),
            description: application.description.clone(),
            icon: application.icon.clone(),
            icon_type: application.icon_type.clone(),
            icon_background: application.icon_background.clone(),
        },
        dependencies: collect_template_dependencies(&sanitized_document),
        flow_document: sanitized_document,
    }
}

pub fn preview_agent_flow_template_package(
    template: AgentFlowTemplatePackage,
    resources: &AgentFlowTemplateResourceSnapshot,
) -> Result<AgentFlowTemplatePreview> {
    ensure_agent_flow_template(&template)?;
    preview_validated_application_template_package(template, resources)
}

pub fn preview_application_template_package(
    template: AgentFlowTemplatePackage,
    resources: &AgentFlowTemplateResourceSnapshot,
) -> Result<AgentFlowTemplatePreview> {
    ensure_application_template(&template)?;
    preview_validated_application_template_package(template, resources)
}

fn preview_validated_application_template_package(
    template: AgentFlowTemplatePackage,
    resources: &AgentFlowTemplateResourceSnapshot,
) -> Result<AgentFlowTemplatePreview> {
    let dependencies = dependency_statuses(&template.flow_document, resources);
    let (document, unresolved_nodes) =
        resolve_template_document(&template.flow_document, resources)?;

    Ok(AgentFlowTemplatePreview {
        schema_version: template.schema_version,
        application: template.application,
        dependencies,
        unresolved_nodes,
        document,
    })
}

pub fn import_agent_flow_template_document(
    template: &AgentFlowTemplatePackage,
    flow_id: Uuid,
    resources: &AgentFlowTemplateResourceSnapshot,
) -> Result<(Value, Vec<AgentFlowTemplateUnresolvedNode>)> {
    ensure_agent_flow_template(template)?;
    import_validated_application_template_document(template, flow_id, resources)
}

pub fn import_application_template_document(
    template: &AgentFlowTemplatePackage,
    flow_id: Uuid,
    resources: &AgentFlowTemplateResourceSnapshot,
) -> Result<(Value, Vec<AgentFlowTemplateUnresolvedNode>)> {
    ensure_application_template(template)?;
    import_validated_application_template_document(template, flow_id, resources)
}

fn import_validated_application_template_document(
    template: &AgentFlowTemplatePackage,
    flow_id: Uuid,
    resources: &AgentFlowTemplateResourceSnapshot,
) -> Result<(Value, Vec<AgentFlowTemplateUnresolvedNode>)> {
    let (mut document, unresolved_nodes) =
        resolve_template_document(&template.flow_document, resources)?;
    let meta = document
        .get_mut("meta")
        .and_then(Value::as_object_mut)
        .ok_or(ControlPlaneError::InvalidInput("flow_document.meta"))?;
    meta.insert("flowId".to_string(), Value::String(flow_id.to_string()));
    Ok((document, unresolved_nodes))
}

fn ensure_agent_flow_template(template: &AgentFlowTemplatePackage) -> Result<()> {
    ensure_application_template(template)?;
    if template.application.application_type != domain::ApplicationType::AgentFlow.as_str() {
        return Err(ControlPlaneError::InvalidInput("application.application_type").into());
    }
    Ok(())
}

fn ensure_application_template(template: &AgentFlowTemplatePackage) -> Result<()> {
    if template.schema_version != AGENT_FLOW_TEMPLATE_SCHEMA_VERSION {
        return Err(ControlPlaneError::InvalidInput("schema_version").into());
    }
    if !matches!(
        template.application.application_type.as_str(),
        "agent_flow" | "workflow"
    ) {
        return Err(ControlPlaneError::InvalidInput("application.application_type").into());
    }
    if template
        .flow_document
        .get("schemaVersion")
        .and_then(Value::as_str)
        != Some(domain::FLOW_SCHEMA_VERSION)
    {
        return Err(ControlPlaneError::InvalidInput("flow_document.schemaVersion").into());
    }
    if template
        .flow_document
        .get("graph")
        .and_then(|graph| graph.get("nodes"))
        .and_then(Value::as_array)
        .is_none()
    {
        return Err(ControlPlaneError::InvalidInput("flow_document.graph.nodes").into());
    }
    if template
        .flow_document
        .get("graph")
        .and_then(|graph| graph.get("edges"))
        .and_then(Value::as_array)
        .is_none()
    {
        return Err(ControlPlaneError::InvalidInput("flow_document.graph.edges").into());
    }
    validate_template_graph(&template.flow_document)?;
    Ok(())
}

fn validate_template_graph(flow_document: &Value) -> Result<()> {
    let nodes = flow_document
        .get("graph")
        .and_then(|graph| graph.get("nodes"))
        .and_then(Value::as_array)
        .ok_or(ControlPlaneError::InvalidInput("flow_document.graph.nodes"))?;
    let edges = flow_document
        .get("graph")
        .and_then(|graph| graph.get("edges"))
        .and_then(Value::as_array)
        .ok_or(ControlPlaneError::InvalidInput("flow_document.graph.edges"))?;

    let mut node_ids = BTreeSet::new();
    for node in nodes {
        let Some(node_id) = required_node_string(node, "id") else {
            return Err(ControlPlaneError::InvalidInput("flow_document.graph.nodes.id").into());
        };
        if !node_ids.insert(node_id) {
            return Err(ControlPlaneError::InvalidInput("flow_document.graph.nodes.id").into());
        }
    }

    let mut edge_ids = BTreeSet::new();
    for edge in edges {
        let Some(edge_id) = required_node_string(edge, "id") else {
            return Err(ControlPlaneError::InvalidInput("flow_document.graph.edges.id").into());
        };
        if !edge_ids.insert(edge_id) {
            return Err(ControlPlaneError::InvalidInput("flow_document.graph.edges.id").into());
        }

        let Some(source) = required_node_string(edge, "source") else {
            return Err(ControlPlaneError::InvalidInput("flow_document.graph.edges.source").into());
        };
        if !node_ids.contains(&source) {
            return Err(ControlPlaneError::InvalidInput("flow_document.graph.edges.source").into());
        }

        let Some(target) = required_node_string(edge, "target") else {
            return Err(ControlPlaneError::InvalidInput("flow_document.graph.edges.target").into());
        };
        if !node_ids.contains(&target) {
            return Err(ControlPlaneError::InvalidInput("flow_document.graph.edges.target").into());
        }
    }

    Ok(())
}

fn resolve_template_document(
    flow_document: &Value,
    resources: &AgentFlowTemplateResourceSnapshot,
) -> Result<(Value, Vec<AgentFlowTemplateUnresolvedNode>)> {
    let mut document = flow_document.clone();
    let nodes = document
        .get_mut("graph")
        .and_then(|graph| graph.get_mut("nodes"))
        .and_then(Value::as_array_mut)
        .ok_or(ControlPlaneError::InvalidInput("flow_document.graph.nodes"))?;
    let mut unresolved_nodes = Vec::new();

    for node in nodes.iter_mut() {
        let candidate = recover_original_node(node).unwrap_or_else(|| node.clone());
        let resolution = resolve_node(&candidate, resources);
        if resolution.status == READY_STATUS {
            if is_unresolved_node(node) {
                *node = candidate;
            }
            continue;
        }

        let unresolved = unresolved_node_from_original(
            &candidate,
            resolution.reason.as_deref().unwrap_or("missing_dependency"),
        );
        unresolved_nodes.push(unresolved_summary(&unresolved));
        *node = unresolved;
    }

    Ok((document, unresolved_nodes))
}

fn recover_original_node(node: &Value) -> Option<Value> {
    if !is_unresolved_node(node) {
        return None;
    }

    let original = node
        .get("config")
        .and_then(|config| config.get("unresolved"))
        .and_then(|unresolved| unresolved.get("original_node"))
        .cloned()?;
    if is_unresolved_node(&original) {
        return None;
    }

    let current_id = node.get("id").and_then(Value::as_str)?;
    let original_id = original.get("id").and_then(Value::as_str)?;
    (current_id == original_id).then_some(original)
}

fn resolve_node(node: &Value, resources: &AgentFlowTemplateResourceSnapshot) -> NodeResolution {
    let Some(node_type) = node.get("type").and_then(Value::as_str) else {
        return unresolved_resolution("missing_node_type");
    };

    if node_type == UNRESOLVED_NODE_TYPE {
        return unresolved_resolution("missing_original_node");
    }

    if node_type == "plugin_node" {
        return resolve_plugin_node(node, resources);
    }

    if !BUILTIN_NODE_TYPES.contains(&node_type) {
        return unresolved_resolution("unsupported_builtin_node");
    }

    NodeResolution {
        status: READY_STATUS.to_string(),
        reason: None,
    }
}

fn resolve_llm_node(node: &Value, resources: &AgentFlowTemplateResourceSnapshot) -> NodeResolution {
    let provider = node
        .get("config")
        .and_then(|config| config.get("model_provider"));
    let provider_code = provider
        .and_then(|value| value.get("provider_code"))
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or_default();
    let model_id = provider
        .and_then(|value| value.get("model_id"))
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or_default();

    if provider_code.is_empty() && model_id.is_empty() {
        return ready_resolution();
    }

    if provider_code.is_empty() {
        return unresolved_resolution("missing_model_provider");
    }

    let Some(provider_resource) = resources
        .model_providers
        .iter()
        .find(|resource| resource.provider_code == provider_code)
    else {
        return unresolved_resolution("missing_model_provider");
    };

    if !provider_resource.ready {
        return unresolved_resolution("model_provider_not_ready");
    }

    if model_id.is_empty()
        || provider_resource.allow_custom_models
        || provider_resource.model_ids.is_empty()
        || provider_resource.model_ids.contains(model_id)
    {
        return ready_resolution();
    }

    unresolved_resolution("missing_model")
}

fn resolve_plugin_node(
    node: &Value,
    resources: &AgentFlowTemplateResourceSnapshot,
) -> NodeResolution {
    let Some(plugin_id) = required_node_string(node, "plugin_id") else {
        return unresolved_resolution("missing_plugin_id");
    };
    let Some(plugin_version) = required_node_string(node, "plugin_version") else {
        return unresolved_resolution("missing_plugin_version");
    };
    let Some(contribution_code) = required_node_string(node, "contribution_code") else {
        return unresolved_resolution("missing_contribution_code");
    };
    let Some(node_shell) = required_node_string(node, "node_shell") else {
        return unresolved_resolution("missing_node_shell");
    };
    let Some(schema_version) = required_node_string(node, "schema_version") else {
        return unresolved_resolution("missing_schema_version");
    };

    let exact = resources.node_contributions.iter().find(|resource| {
        resource.plugin_id == plugin_id
            && resource.plugin_version == plugin_version
            && resource.contribution_code == contribution_code
            && resource.node_shell == node_shell
            && resource.schema_version == schema_version
    });
    let Some(resource) = exact else {
        let same_contribution_exists = resources.node_contributions.iter().any(|resource| {
            resource.plugin_id == plugin_id && resource.contribution_code == contribution_code
        });
        return unresolved_resolution(if same_contribution_exists {
            "version_mismatch"
        } else {
            "missing_plugin"
        });
    };

    if resource.dependency_status != READY_STATUS {
        return unresolved_resolution(&resource.dependency_status);
    }

    let contract_matches = [
        (
            "plugin_unique_identifier",
            &resource.plugin_unique_identifier,
        ),
        ("package_id", &resource.package_id),
        ("contribution_checksum", &resource.contribution_checksum),
        (
            "compiled_contribution_hash",
            &resource.compiled_contribution_hash,
        ),
    ]
    .into_iter()
    .all(|(key, expected)| required_node_string(node, key).as_deref() == Some(expected.as_str()));

    if !contract_matches {
        return unresolved_resolution("contribution_contract_mismatch");
    }

    ready_resolution()
}

fn ready_resolution() -> NodeResolution {
    NodeResolution {
        status: READY_STATUS.to_string(),
        reason: None,
    }
}

fn unresolved_resolution(reason: &str) -> NodeResolution {
    NodeResolution {
        status: MISSING_DEPENDENCY_STATUS.to_string(),
        reason: Some(reason.to_string()),
    }
}

fn required_node_string(node: &Value, key: &str) -> Option<String> {
    node.get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn unresolved_node_from_original(original_node: &Value, reason: &str) -> Value {
    let original_type = original_node
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("unknown");

    json!({
        "id": original_node.get("id").cloned().unwrap_or(Value::String("unresolved-node".to_string())),
        "type": UNRESOLVED_NODE_TYPE,
        "alias": original_node
            .get("alias")
            .cloned()
            .unwrap_or(Value::String("Unresolved node".to_string())),
        "description": original_node
            .get("description")
            .cloned()
            .unwrap_or(Value::String(String::new())),
        "containerId": original_node
            .get("containerId")
            .cloned()
            .unwrap_or(Value::Null),
        "position": original_node
            .get("position")
            .cloned()
            .unwrap_or_else(|| json!({ "x": 0, "y": 0 })),
        "configVersion": 1,
        "config": {
            "unresolved": {
                "dependency_status": MISSING_DEPENDENCY_STATUS,
                "reason": reason,
                "original_type": original_type,
                "original_node": original_node,
            }
        },
        "bindings": original_node
            .get("bindings")
            .cloned()
            .unwrap_or_else(|| json!({})),
        "outputs": original_node
            .get("outputs")
            .cloned()
            .unwrap_or_else(|| json!([])),
    })
}

fn unresolved_summary(unresolved_node: &Value) -> AgentFlowTemplateUnresolvedNode {
    let unresolved = unresolved_node
        .get("config")
        .and_then(|config| config.get("unresolved"))
        .unwrap_or(&Value::Null);

    AgentFlowTemplateUnresolvedNode {
        node_id: unresolved_node
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or("unresolved-node")
            .to_string(),
        alias: unresolved_node
            .get("alias")
            .and_then(Value::as_str)
            .unwrap_or("Unresolved node")
            .to_string(),
        original_type: unresolved
            .get("original_type")
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .to_string(),
        dependency_status: unresolved
            .get("dependency_status")
            .and_then(Value::as_str)
            .unwrap_or(MISSING_DEPENDENCY_STATUS)
            .to_string(),
        reason: unresolved
            .get("reason")
            .and_then(Value::as_str)
            .unwrap_or("missing_dependency")
            .to_string(),
        original_node: unresolved
            .get("original_node")
            .cloned()
            .unwrap_or(Value::Null),
    }
}

fn dependency_statuses(
    flow_document: &Value,
    resources: &AgentFlowTemplateResourceSnapshot,
) -> Vec<AgentFlowTemplateDependencyStatus> {
    collect_template_dependencies(flow_document)
        .into_iter()
        .map(|dependency| {
            let resolution = resolve_dependency(&dependency, resources);
            AgentFlowTemplateDependencyStatus {
                dependency,
                status: resolution.status,
                reason: resolution.reason,
            }
        })
        .collect()
}

fn resolve_dependency(
    dependency: &AgentFlowTemplateDependency,
    resources: &AgentFlowTemplateResourceSnapshot,
) -> NodeResolution {
    match dependency.kind.as_str() {
        "model_provider" => resolve_model_dependency(dependency, resources),
        "plugin_node" => resolve_plugin_dependency(dependency, resources),
        "builtin_node" => {
            if dependency
                .node_type
                .as_deref()
                .is_some_and(|node_type| BUILTIN_NODE_TYPES.contains(&node_type))
            {
                ready_resolution()
            } else {
                unresolved_resolution("unsupported_builtin_node")
            }
        }
        _ => unresolved_resolution("unsupported_dependency"),
    }
}

fn resolve_model_dependency(
    dependency: &AgentFlowTemplateDependency,
    resources: &AgentFlowTemplateResourceSnapshot,
) -> NodeResolution {
    let synthetic_node = json!({
        "type": "llm",
        "config": {
            "model_provider": {
                "provider_code": dependency.provider_code.clone().unwrap_or_default(),
                "model_id": dependency.model_id.clone().unwrap_or_default(),
            }
        }
    });
    resolve_llm_node(&synthetic_node, resources)
}

fn resolve_plugin_dependency(
    dependency: &AgentFlowTemplateDependency,
    resources: &AgentFlowTemplateResourceSnapshot,
) -> NodeResolution {
    let synthetic_node = json!({
        "type": "plugin_node",
        "plugin_id": dependency.plugin_id,
        "plugin_version": dependency.plugin_version,
        "contribution_code": dependency.contribution_code,
        "node_shell": dependency.node_shell,
        "schema_version": dependency.schema_version,
        "plugin_unique_identifier": dependency.plugin_unique_identifier,
        "package_id": dependency.package_id,
        "contribution_checksum": dependency.contribution_checksum,
        "compiled_contribution_hash": dependency.compiled_contribution_hash,
    });
    resolve_plugin_node(&synthetic_node, resources)
}

fn collect_template_dependencies(flow_document: &Value) -> Vec<AgentFlowTemplateDependency> {
    let Some(nodes) = flow_document
        .get("graph")
        .and_then(|graph| graph.get("nodes"))
        .and_then(Value::as_array)
    else {
        return Vec::new();
    };

    let mut dependencies = Vec::new();
    let mut seen = BTreeSet::new();
    for node in nodes {
        let source_node = recover_original_node(node).unwrap_or_else(|| node.clone());
        for dependency in dependencies_for_node(&source_node) {
            let key = serde_json::to_string(&dependency).unwrap_or_default();
            if seen.insert(key) {
                dependencies.push(dependency);
            }
        }
    }
    dependencies
}

fn dependencies_for_node(node: &Value) -> Vec<AgentFlowTemplateDependency> {
    let node_id = node.get("id").and_then(Value::as_str).map(str::to_string);
    let node_type = node
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_string();
    let config_version = node.get("configVersion").and_then(Value::as_i64);
    let mut dependencies = Vec::new();

    if node_type == "plugin_node" {
        dependencies.push(AgentFlowTemplateDependency {
            kind: "plugin_node".to_string(),
            node_id,
            node_type: Some(node_type),
            config_version,
            provider_code: None,
            model_id: None,
            plugin_id: required_node_string(node, "plugin_id"),
            plugin_version: required_node_string(node, "plugin_version"),
            contribution_code: required_node_string(node, "contribution_code"),
            node_shell: required_node_string(node, "node_shell"),
            schema_version: required_node_string(node, "schema_version"),
            plugin_unique_identifier: required_node_string(node, "plugin_unique_identifier"),
            package_id: required_node_string(node, "package_id"),
            contribution_checksum: required_node_string(node, "contribution_checksum"),
            compiled_contribution_hash: required_node_string(node, "compiled_contribution_hash"),
        });
        return dependencies;
    }

    dependencies.push(AgentFlowTemplateDependency {
        kind: "builtin_node".to_string(),
        node_id: node_id.clone(),
        node_type: Some(node_type.clone()),
        config_version,
        provider_code: None,
        model_id: None,
        plugin_id: None,
        plugin_version: None,
        contribution_code: None,
        node_shell: None,
        schema_version: None,
        plugin_unique_identifier: None,
        package_id: None,
        contribution_checksum: None,
        compiled_contribution_hash: None,
    });

    if node_type == "llm" {
        let provider = node
            .get("config")
            .and_then(|config| config.get("model_provider"));
        let provider_code = provider
            .and_then(|value| value.get("provider_code"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
        let model_id = provider
            .and_then(|value| value.get("model_id"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);

        if provider_code.is_some() || model_id.is_some() {
            dependencies.push(AgentFlowTemplateDependency {
                kind: "model_provider".to_string(),
                node_id,
                node_type: Some(node_type),
                config_version,
                provider_code,
                model_id,
                plugin_id: None,
                plugin_version: None,
                contribution_code: None,
                node_shell: None,
                schema_version: None,
                plugin_unique_identifier: None,
                package_id: None,
                contribution_checksum: None,
                compiled_contribution_hash: None,
            });
        }
    }

    dependencies
}

fn sanitize_template_value(value: &Value) -> Value {
    match value {
        Value::Object(object) => Value::Object(
            object
                .iter()
                .map(|(key, value)| {
                    if is_sensitive_key(key) {
                        (key.clone(), Value::String("__omitted_secret__".to_string()))
                    } else {
                        (key.clone(), sanitize_template_value(value))
                    }
                })
                .collect(),
        ),
        Value::Array(items) => Value::Array(items.iter().map(sanitize_template_value).collect()),
        _ => value.clone(),
    }
}

fn is_sensitive_key(key: &str) -> bool {
    let normalized = key.to_ascii_lowercase().replace(['-', '_'], "");
    normalized.contains("apikey")
        || normalized.contains("secret")
        || normalized.contains("password")
        || normalized.contains("credential")
        || normalized.ends_with("token")
}

fn is_unresolved_node(node: &Value) -> bool {
    node.get("type").and_then(Value::as_str) == Some(UNRESOLVED_NODE_TYPE)
}

fn model_ids_from_catalog_json(models_json: &Value) -> BTreeSet<String> {
    let Some(models) = models_json.as_array() else {
        return BTreeSet::new();
    };

    models
        .iter()
        .filter_map(|model| {
            model
                .get("id")
                .or_else(|| model.get("model_id"))
                .or_else(|| model.get("upstream_model_id"))
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
        })
        .collect()
}
