//! Native bundle loading and plugin ownership validation; no catalog-derived rewriting.
use super::*;
use crate::errors::ControlPlaneError;
use crate::plugin_management::contribution_authority::read_verified_package_asset;
use std::{collections::BTreeSet, path::Path};

pub(in crate::plugin_management::managed_activation) async fn load_managed_mcp_bundle(
    package_root: &Path,
    reference: &plugin_framework::ManagedServiceMcpTemplate,
) -> Result<domain::McpBundlePackage> {
    let raw =
        read_verified_package_asset(package_root, &reference.path, Some(&reference.sha256)).await?;
    let manifest: domain::McpBundleManifest = serde_json::from_slice(&raw)?;
    let mut package = domain::McpBundlePackage {
        manifest,
        tools: vec![],
        instances: vec![],
        connections: vec![],
    };
    let directory = reference
        .path
        .rsplit_once('/')
        .map_or("", |(directory, _)| directory);
    let mut paths = BTreeSet::new();
    for file in &package.manifest.files {
        if !plugin_framework::valid_managed_mcp_asset_path(&file.path)
            || !paths.insert(&file.path)
            || file.kind == domain::McpBundleFileKind::Connection
        {
            return Err(ControlPlaneError::InvalidInput("managed_mcp_template_file").into());
        }
        let path = if directory.is_empty() {
            file.path.clone()
        } else {
            format!("{directory}/{}", file.path)
        };
        let raw = read_verified_package_asset(package_root, &path, Some(&file.sha256)).await?;
        match file.kind {
            domain::McpBundleFileKind::Tool => package.tools.push(serde_json::from_slice(&raw)?),
            domain::McpBundleFileKind::Instance => {
                package.instances.push(serde_json::from_slice(&raw)?)
            }
            domain::McpBundleFileKind::Connection => unreachable!(),
        }
    }
    Ok(package)
}

/// The caller supplies a hash-verified installed bundle. Validate ownership before any write.
pub fn prepare_managed_mcp_bundle_template(
    actor: &domain::ActorContext,
    installation_id: Uuid,
    manifest: &plugin_framework::PluginManifestV1,
    package: domain::McpBundlePackage,
) -> Result<SeedMcpBundleGraphInput> {
    let instance = manifest
        .managed_service
        .as_ref()
        .and_then(|service| service.mcp_instance.as_ref())
        .filter(|instance| instance.template.is_some())
        .ok_or(ControlPlaneError::InvalidInput(
            "managed_mcp_template_declaration",
        ))?;
    crate::mcp_bundle::validate_package(&package)?;
    let code = manifest.plugin_code()?;
    if package.manifest.organization != manifest.publisher_namespace
        || package.manifest.bundle_id != code
        || package.manifest.bundle_version != manifest.version
        || package.instances.len() != 1
        || package.instances[0].instance_id != instance.instance_id
        || !package.connections.is_empty()
        || package.tools.is_empty()
    {
        return Err(ControlPlaneError::InvalidInput("managed_mcp_template_owner").into());
    }
    let allowed_interfaces = manifest
        .managed_service
        .as_ref()
        .unwrap()
        .operations
        .iter()
        .map(|operation| operation.interface_id.as_str())
        .collect::<BTreeSet<_>>();
    let namespace = format!("plugin.{code}.");
    let mut tool_ids = BTreeSet::new();
    for tool in &package.tools {
        if !tool.tool_id.starts_with(&namespace)
            || tool.tool_id.len() == namespace.len()
            || !tool
                .tool_id
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte))
            || !matches!(&tool.execution_target, domain::McpToolExecutionTarget::InterfaceWrapper { interface_id }
                if allowed_interfaces.contains(interface_id.as_str()))
        {
            return Err(ControlPlaneError::InvalidInput("managed_mcp_template_tool_owner").into());
        }
        tool_ids.insert(tool.tool_id.as_str());
    }
    let graph = &package.instances[0];
    let mut groups = BTreeSet::new();
    for group in &graph.groups {
        if !logical_path(&group.path) || !groups.insert(group.path.as_str()) {
            return Err(ControlPlaneError::InvalidInput("managed_mcp_template_group").into());
        }
    }
    if !groups.contains(graph.default_entry_path.as_str())
        || groups.iter().any(|path| {
            let parent = path.rsplit_once('/').map_or("", |(parent, _)| parent);
            !parent.is_empty() && !groups.contains(parent)
        })
    {
        return Err(ControlPlaneError::InvalidInput("managed_mcp_template_group").into());
    }
    let mut bindings = BTreeSet::new();
    let mut bound_tools = BTreeSet::new();
    for binding in &graph.bindings {
        if !groups.contains(binding.group_path.as_str())
            || !tool_ids.contains(binding.tool_id.as_str())
            || !bindings.insert((binding.group_path.as_str(), binding.tool_id.as_str()))
        {
            return Err(ControlPlaneError::InvalidInput("managed_mcp_template_binding").into());
        }
        bound_tools.insert(binding.tool_id.as_str());
    }
    if bound_tools != tool_ids {
        return Err(ControlPlaneError::InvalidInput("managed_mcp_template_binding").into());
    }
    let tools = package
        .tools
        .into_iter()
        .map(|tool| CreateMcpToolInput {
            id: Uuid::now_v7(),
            actor_user_id: actor.user_id,
            workspace_id: actor.current_workspace_id,
            tool_id: tool.tool_id,
            name: tool.name,
            short_description: tool.short_description,
            full_description: tool.full_description,
            execution_target: tool.execution_target,
            parameter_schema: tool.parameter_schema_snapshot,
            result_schema: tool.result_schema_snapshot,
            input_mapping: tool.input_mapping,
            output_mapping: tool.output_mapping,
            max_inline_chars: tool.max_inline_chars,
            response_fields: tool.response_fields,
            permission_code: tool.permission_code_snapshot,
            risk_level: tool.risk_level_snapshot,
            des_id: Uuid::now_v7().simple().to_string()[..8].to_string(),
            des_id_required: false,
            status: tool.status,
        })
        .collect();
    Ok(SeedMcpBundleGraphInput {
        kind: if instance.reset_on_activation {
            McpBundleSeedKind::PluginTemplateReset { installation_id }
        } else {
            McpBundleSeedKind::PluginTemplate { installation_id }
        },
        actor_user_id: actor.user_id,
        workspace_id: actor.current_workspace_id,
        source: domain::McpManagedBundleSource {
            organization: manifest.publisher_namespace.clone(),
            bundle_id: code.to_string(),
            bundle_version: manifest.version.clone(),
        },
        tools,
        instances: package.instances,
    })
}

fn logical_path(path: &str) -> bool {
    path == "/"
        || path.starts_with('/')
            && path.split('/').skip(1).all(|part| {
                !part.is_empty()
                    && part
                        .bytes()
                        .all(|byte| byte.is_ascii_alphanumeric() || b"_-".contains(&byte))
            })
}
