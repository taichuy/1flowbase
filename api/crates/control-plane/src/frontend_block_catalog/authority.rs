//! Joins host-owned catalog entries to exact managed payload subjects and durable grants.
use super::{
    FrontendContributionCandidate, FRONTEND_BLOCK_CONTRIBUTION_CONTRACT_VERSION,
    FRONTEND_BLOCK_CONTRIBUTION_POINT_ID,
};
use crate::plugin_management::HostContributionGrantPolicy;
use plugin_framework::extension_bus::{ContributionMode, ModuleKind};
use plugin_framework::{ManagedContributionPayload, ManagedManifest, PluginExecutionMode};

pub(super) fn requires_authority(installation: &domain::PluginInstallationRecord) -> bool {
    !(installation.is_system_reserved && installation.source_kind == "builtin")
        && installation.contract_version == "1flowbase.extension-bus/v1"
}

pub(super) fn granted_permissions(
    candidate: &FrontendContributionCandidate,
    requested: &str,
) -> Option<Vec<String>> {
    if !requires_authority(&candidate.installation) {
        return Some(vec![requested.to_string()]);
    }
    let installation = &candidate.installation;
    let managed: ManagedManifest =
        serde_json::from_value(installation.metadata_json.get("managed")?.clone()).ok()?;
    let expected_kind = match installation.category {
        domain::ExtensionCategory::RuntimeExtensions => ModuleKind::Runtime,
        domain::ExtensionCategory::CapabilityPlugins => ModuleKind::Capability,
        _ => return None,
    };
    if managed.module.module_id.as_str() != installation.provider_code
        || managed.module.module_version.as_str() != installation.plugin_version
        || managed.module.module_kind != expected_kind
        || domain::managed_installation_scope(installation, candidate.workspace_id)
            != candidate.workspace_id
    {
        return None;
    }
    // No contribution-name guessing: the verified v2 declaration explicitly owns this payload.
    let mut bindings = managed.execution_bindings.iter().filter(|binding| matches!(
        binding.payload.as_ref(), Some(ManagedContributionPayload::FrontendBlock { contribution_code })
            if contribution_code == &candidate.catalog_entry.contribution_code
    ));
    let binding = bindings.next()?;
    if bindings.next().is_some() || binding.execution_mode != PluginExecutionMode::DeclarativeOnly {
        return None;
    }
    let mut subjects = managed
        .module
        .contributions
        .iter()
        .filter(|subject| subject.contribution_id == binding.contribution_id);
    let subject = subjects.next()?;
    if subjects.next().is_some()
        || subject.contributor_module_id != managed.module.module_id
        || subject.point_id.as_str() != FRONTEND_BLOCK_CONTRIBUTION_POINT_ID
        || subject.contract_version.as_str() != FRONTEND_BLOCK_CONTRIBUTION_CONTRACT_VERSION
        || subject.mode != ContributionMode::Append
        || !subject
            .required_permissions
            .iter()
            .any(|permission| permission.as_str() == requested)
    {
        return None;
    }
    let permissions = HostContributionGrantPolicy::root_composition()
        .effective_permissions(
            installation,
            candidate.workspace_id,
            subject,
            candidate.authority.as_ref()?,
        )
        .ok()?;
    permissions
        .iter()
        .any(|permission| permission.as_str() == requested)
        .then(|| {
            permissions
                .into_iter()
                .map(|permission| permission.as_str().to_string())
                .collect()
        })
}
