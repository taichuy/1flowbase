//! Explicit managed declarations. Descriptor grants and activation are declarations only;
//! the installing host replaces them from durable authorization and desired state.
use std::{
    collections::BTreeSet,
    path::{Component, Path},
};

use extension_contracts::extension_bus::{
    ContributionId, ModuleActivationDeclaration, ModuleDescriptor, ModuleKind,
};
use serde::{Deserialize, Serialize};

use crate::{
    FrameworkResult, PluginConsumptionKind, PluginExecutionMode, PluginFrameworkError,
    PluginManifestV1, PluginRuntimeManifest,
};

/// References existing typed manifest payloads; execution does not infer a payload from a slot.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ManagedContributionPayload {
    NodeContribution { contribution_code: String },
    FrontendBlock { contribution_code: String },
    JsDependency { alias: String },
    ProviderDistributionRule { rule_id: String },
    DataModels,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedContributionExecutionBinding {
    pub contribution_id: ContributionId,
    pub execution_mode: PluginExecutionMode,
    pub runtime: PluginRuntimeManifest,
    pub handler: String,
    #[serde(default)]
    pub payload: Option<ManagedContributionPayload>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedManifest {
    pub module: ModuleDescriptor,
    pub execution_bindings: Vec<ManagedContributionExecutionBinding>,
}

impl ManagedManifest {
    /// Pins the declaration (including contract and required permissions) and execution together.
    pub fn execution_binding_fingerprint(
        &self,
        contribution_id: &ContributionId,
    ) -> FrameworkResult<extension_contracts::extension_bus::ManagedBindingFingerprint> {
        let contribution = self
            .module
            .contributions
            .iter()
            .find(|contribution| &contribution.contribution_id == contribution_id)
            .ok_or_else(|| invalid("managed contribution does not exist"))?;
        let binding = self
            .execution_bindings
            .iter()
            .find(|binding| &binding.contribution_id == contribution_id)
            .ok_or_else(|| invalid("managed execution binding does not exist"))?;
        let bytes = serde_json::to_vec(&(contribution, binding))
            .map_err(|error| invalid(&error.to_string()))?;
        Ok(extension_contracts::extension_bus::ManagedBindingFingerprint::from_bytes(&bytes))
    }
}

impl PluginManifestV1 {
    /// Governance derives from the package classification, never a contribution's point kind.
    pub fn module_kind(&self) -> ModuleKind {
        match self.consumption_kind {
            PluginConsumptionKind::HostExtension => ModuleKind::TrustedHost,
            PluginConsumptionKind::RuntimeExtension => ModuleKind::Runtime,
            PluginConsumptionKind::CapabilityPlugin => ModuleKind::Capability,
        }
    }
}

pub(crate) fn validate_managed_manifest(manifest: &PluginManifestV1) -> FrameworkResult<()> {
    let managed = manifest
        .managed
        .as_ref()
        .ok_or_else(|| invalid("manifest_version=2 requires managed"))?;
    let module = &managed.module;
    if manifest.contract_version != "1flowbase.extension-bus/v1" {
        return Err(invalid(
            "managed package contract_version must be 1flowbase.extension-bus/v1",
        ));
    }
    if module.module_id.as_str() != manifest.plugin_code()?
        || module.module_version.as_str() != manifest.version
        || module.module_kind != manifest.module_kind()
    {
        return Err(invalid(
            "managed.module identity and governance must match the package",
        ));
    }
    // Disabled state is host-owned, and a package may not self-award authorization.
    if module.activation != ModuleActivationDeclaration::Active
        || !module.granted_permissions.is_empty()
    {
        return Err(invalid(
            "managed.module activation and granted_permissions are host-owned",
        ));
    }
    if module.module_kind == ModuleKind::TrustedHost
        && manifest
            .binding_targets
            .iter()
            .any(|target| target == "workspace")
    {
        return Err(invalid(
            "host_extension cannot declare workspace binding_targets",
        ));
    }
    if !module.module_kind.may_define_points()
        && module
            .extension_points
            .iter()
            .any(|point| !point.is_managed_composition_event(&module.module_id))
    {
        return Err(invalid(
            "managed packages may only define their schema-registered namespaced event",
        ));
    }
    if module.contributions.is_empty() {
        return Err(invalid("managed.module requires contributions"));
    }
    if !manifest.slot_codes.is_empty() {
        return Err(invalid(
            "manifest_version=2 uses managed contributions, not slot_codes",
        ));
    }
    if !manifest.node_contributions.is_empty() {
        crate::manifest_v1::validate_node_contributions(&manifest.node_contributions)?;
    }
    if !manifest.block_contributions.is_empty() {
        crate::manifest_v1::validate_frontend_block_contributions(&manifest.block_contributions)?;
    }
    if !manifest.js_dependencies.is_empty() {
        crate::manifest_v1::validate_js_dependencies(&manifest.js_dependencies)?;
    }
    if !manifest.data_models.is_empty() && manifest.permissions.storage != "host_managed" {
        return Err(invalid(
            "data_models requires permissions.storage=host_managed",
        ));
    }
    for model in &manifest.data_models {
        model
            .validate_additive_v1()
            .map_err(|error| invalid(&error.to_string()))?;
    }
    for rule in &manifest.provider_distribution_rules {
        rule.validate()
            .map_err(|error| invalid(&error.to_string()))?;
    }
    let mut contributions = BTreeSet::new();
    for contribution in &module.contributions {
        if contribution.contributor_module_id != module.module_id
            || !contributions.insert(&contribution.contribution_id)
        {
            return Err(invalid(
                "managed contributions require matching module identity and unique contribution_id",
            ));
        }
        if contribution.mode == extension_contracts::extension_bus::ContributionMode::Override
            && !module.module_kind.may_override()
        {
            return Err(invalid(
                "only trusted host packages may override contributions",
            ));
        }
    }
    let mut points = BTreeSet::new();
    for point in &module.extension_points {
        if point.owner_module_id != module.module_id || !points.insert(&point.point_id) {
            return Err(invalid(
                "managed extension points require matching owner and unique point_id",
            ));
        }
    }
    let mut payloads = BTreeSet::new();
    for node in &manifest.node_contributions {
        if !payloads.insert(ManagedContributionPayload::NodeContribution {
            contribution_code: node.contribution_code.clone(),
        }) {
            return Err(invalid("duplicate managed node payload"));
        }
    }
    for block in &manifest.block_contributions {
        if !payloads.insert(ManagedContributionPayload::FrontendBlock {
            contribution_code: block.contribution_code.clone(),
        }) {
            return Err(invalid("duplicate managed frontend block payload"));
        }
    }
    for dependency in &manifest.js_dependencies {
        payloads.insert(ManagedContributionPayload::JsDependency {
            alias: dependency.alias.clone(),
        });
    }
    for rule in &manifest.provider_distribution_rules {
        if !payloads.insert(ManagedContributionPayload::ProviderDistributionRule {
            rule_id: rule.rule_id.clone(),
        }) {
            return Err(invalid(
                "duplicate managed provider distribution rule payload",
            ));
        }
    }
    if !manifest.data_models.is_empty() {
        payloads.insert(ManagedContributionPayload::DataModels);
    }
    let mut bound_payloads = BTreeSet::new();
    let mut bindings = BTreeSet::new();
    for binding in &managed.execution_bindings {
        if !contributions.contains(&binding.contribution_id)
            || !bindings.insert(&binding.contribution_id)
        {
            return Err(invalid(
                "managed execution binding must reference a unique declared contribution_id",
            ));
        }
        if let Some(payload) = &binding.payload {
            if !payloads.contains(payload) || !bound_payloads.insert(payload.clone()) {
                return Err(invalid(
                    "managed payload binding must reference a unique existing payload",
                ));
            }
            if let ManagedContributionPayload::ProviderDistributionRule { rule_id } = payload {
                let rule = manifest
                    .provider_distribution_rules
                    .iter()
                    .find(|rule| &rule.rule_id == rule_id)
                    .ok_or_else(|| invalid("missing provider distribution payload"))?;
                if rule.handler != binding.handler {
                    return Err(invalid(
                        "provider distribution payload handler must match execution binding",
                    ));
                }
            }
        }
        if binding.handler.trim().is_empty() {
            return Err(invalid(
                "managed execution binding handler must not be empty",
            ));
        }
        if !manifest.permissions.credit.is_empty()
            && (manifest.trust_level != "verified_official"
                || binding.execution_mode != PluginExecutionMode::ProcessPerCall)
        {
            return Err(invalid(
                "managed credit permissions require verified_official process_per_call bindings",
            ));
        }
        let native = binding.execution_mode == PluginExecutionMode::InProcess;
        let worker = matches!(
            binding.execution_mode,
            PluginExecutionMode::StatefulProviderWorker
                | PluginExecutionMode::StatefulRuntimeWorker
        );
        let expected_protocol = if native {
            "native_host"
        } else if worker {
            "stdio_json_worker"
        } else {
            "stdio_json"
        };
        if binding.runtime.protocol != expected_protocol
            || (native && module.module_kind != ModuleKind::TrustedHost)
        {
            return Err(invalid("managed execution binding protocol does not match its execution mode or governance"));
        }
        let path = Path::new(&binding.runtime.entry);
        if binding.runtime.entry.trim().is_empty()
            || path.is_absolute()
            || path
                .components()
                .any(|component| !matches!(component, Component::Normal(_)))
        {
            return Err(invalid(
                "managed execution binding entry must be a relative package path",
            ));
        }
        let mut capabilities = BTreeSet::new();
        if binding
            .runtime
            .capabilities
            .iter()
            .any(|value| value.trim().is_empty() || !capabilities.insert(value))
        {
            return Err(invalid(
                "managed execution capabilities must be non-empty and unique",
            ));
        }
    }
    if bound_payloads != payloads {
        return Err(invalid(
            "every managed payload requires exactly one explicit binding",
        ));
    }
    if bindings != contributions {
        return Err(invalid(
            "every managed contribution requires exactly one execution binding",
        ));
    }
    Ok(())
}

fn invalid(message: &str) -> PluginFrameworkError {
    PluginFrameworkError::invalid_provider_package(message)
}
