use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Component, Path, PathBuf},
};

use anyhow::{bail, Context, Result};
use plugin_framework::{
    extension_bus::{
        compile_extension_graph, EffectiveExtensionGraph, ExtensionBusVersion,
        ModuleActivationDeclaration, ModuleDescriptor, ModuleDisableReason, ModuleId, ModuleKind,
        ModuleVersion,
    },
    parse_host_extension_contribution_manifest, parse_plugin_manifest,
    HostExtensionContributionManifest, PluginConsumptionKind, PluginManifestV1,
};

use super::{parse_deployment_plugin_set, DeploymentPluginSet};

pub const DEFAULT_PLUGIN_SET_PATH: &str = "plugins/sets/default.yaml";
pub const BOOT_CORE_MODULE_ID: &str = "1flowbase.boot-core";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleActivationFact {
    module_id: ModuleId,
    activation: ModuleActivationDeclaration,
}

impl ModuleActivationFact {
    pub fn new(
        module_id: impl Into<String>,
        activation: ModuleActivationDeclaration,
    ) -> Result<Self> {
        Ok(Self {
            module_id: ModuleId::new(module_id)?,
            activation,
        })
    }

    pub fn disabled(module_id: impl Into<String>, reason: ModuleDisableReason) -> Result<Self> {
        Self::new(module_id, ModuleActivationDeclaration::Disabled { reason })
    }
}

#[derive(Debug, Clone)]
pub struct ExtensionGraphInputAssembly {
    plugin_set: DeploymentPluginSet,
    module_descriptors: Vec<ModuleDescriptor>,
    host_extension_manifests: Vec<(PluginManifestV1, HostExtensionContributionManifest)>,
}

impl ExtensionGraphInputAssembly {
    pub fn plugin_set(&self) -> &DeploymentPluginSet {
        &self.plugin_set
    }

    pub fn module_descriptors(&self) -> &[ModuleDescriptor] {
        &self.module_descriptors
    }

    pub fn host_extension_manifests(
        &self,
    ) -> &[(PluginManifestV1, HostExtensionContributionManifest)] {
        &self.host_extension_manifests
    }

    pub fn into_host_extension_manifests(
        self,
    ) -> Vec<(PluginManifestV1, HostExtensionContributionManifest)> {
        self.host_extension_manifests
    }

    pub fn compile_graph(&self) -> Result<EffectiveExtensionGraph> {
        compile_extension_graph(self.module_descriptors.clone()).map_err(anyhow::Error::from)
    }
}

pub fn assemble_extension_graph_input(
    api_workspace_root: impl AsRef<Path>,
    plugin_set_path: impl AsRef<Path>,
    activation_facts: Vec<ModuleActivationFact>,
) -> Result<ExtensionGraphInputAssembly> {
    let api_workspace_root = api_workspace_root.as_ref();
    let plugin_set_path = resolve_workspace_path(api_workspace_root, plugin_set_path.as_ref())?;
    let set_raw = fs::read_to_string(&plugin_set_path)
        .with_context(|| format!("failed to read plugin set {}", plugin_set_path.display()))?;
    let plugin_set = parse_deployment_plugin_set(&set_raw)
        .with_context(|| format!("failed to parse plugin set {}", plugin_set_path.display()))?;
    let mut activations = index_activation_facts(activation_facts)?;
    if activations.contains_key(&ModuleId::new(BOOT_CORE_MODULE_ID)?) {
        bail!("Boot Core activation cannot be overridden by deployment facts");
    }

    let mut module_descriptors = vec![boot_core_descriptor()?];
    let mut host_extension_manifests = Vec::new();

    for module_id in plugin_set.host_extension_ids() {
        let manifest = load_package_manifest(
            api_workspace_root,
            "host-extensions",
            module_id,
            PluginConsumptionKind::HostExtension,
        )?;
        let contribution = load_host_contribution(api_workspace_root, module_id, &manifest)?;
        let activation = take_activation(&mut activations, module_id)?;
        module_descriptors.push(derive_module_descriptor(
            &manifest,
            ModuleKind::TrustedHost,
            activation,
        )?);
        host_extension_manifests.push((manifest, contribution));
    }
    for module_id in plugin_set.runtime_extension_ids() {
        let manifest = load_package_manifest(
            api_workspace_root,
            "runtime-extensions",
            module_id,
            PluginConsumptionKind::RuntimeExtension,
        )?;
        let activation = take_activation(&mut activations, module_id)?;
        module_descriptors.push(derive_module_descriptor(
            &manifest,
            ModuleKind::Runtime,
            activation,
        )?);
    }
    for module_id in plugin_set.capability_plugin_ids() {
        let manifest = load_package_manifest(
            api_workspace_root,
            "capability-plugins",
            module_id,
            PluginConsumptionKind::CapabilityPlugin,
        )?;
        let activation = take_activation(&mut activations, module_id)?;
        module_descriptors.push(derive_module_descriptor(
            &manifest,
            ModuleKind::Capability,
            activation,
        )?);
    }

    if let Some((module_id, _)) = activations.pop_first() {
        bail!("activation fact references module not listed in plugin set: {module_id:?}");
    }
    module_descriptors.sort_by(|left, right| left.module_id.cmp(&right.module_id));

    Ok(ExtensionGraphInputAssembly {
        plugin_set,
        module_descriptors,
        host_extension_manifests,
    })
}

fn index_activation_facts(
    activation_facts: Vec<ModuleActivationFact>,
) -> Result<BTreeMap<ModuleId, ModuleActivationDeclaration>> {
    let mut activations = BTreeMap::new();
    for fact in activation_facts {
        if activations
            .insert(fact.module_id.clone(), fact.activation)
            .is_some()
        {
            bail!("duplicate activation fact for module {:?}", fact.module_id);
        }
    }
    Ok(activations)
}

fn take_activation(
    activations: &mut BTreeMap<ModuleId, ModuleActivationDeclaration>,
    module_id: &str,
) -> Result<ModuleActivationDeclaration> {
    Ok(activations
        .remove(&ModuleId::new(module_id)?)
        .unwrap_or(ModuleActivationDeclaration::Active))
}

fn boot_core_descriptor() -> Result<ModuleDescriptor> {
    Ok(ModuleDescriptor {
        bus_version: ExtensionBusVersion::V1,
        module_id: ModuleId::new(BOOT_CORE_MODULE_ID)?,
        module_version: ModuleVersion::new(env!("CARGO_PKG_VERSION"))?,
        module_kind: ModuleKind::BootCore,
        activation: ModuleActivationDeclaration::Active,
        dependencies: BTreeSet::new(),
        granted_permissions: BTreeSet::new(),
        extension_points: Vec::new(),
        contributions: Vec::new(),
    })
}

fn derive_module_descriptor(
    manifest: &PluginManifestV1,
    module_kind: ModuleKind,
    activation: ModuleActivationDeclaration,
) -> Result<ModuleDescriptor> {
    Ok(ModuleDescriptor {
        bus_version: ExtensionBusVersion::V1,
        module_id: ModuleId::new(manifest.plugin_code()?)?,
        module_version: ModuleVersion::new(manifest.version.clone())?,
        module_kind,
        activation,
        dependencies: BTreeSet::new(),
        granted_permissions: BTreeSet::new(),
        extension_points: Vec::new(),
        contributions: Vec::new(),
    })
}

fn load_package_manifest(
    api_workspace_root: &Path,
    package_kind: &str,
    listed_module_id: &str,
    expected_kind: PluginConsumptionKind,
) -> Result<PluginManifestV1> {
    let manifest_path = api_workspace_root
        .join("plugins")
        .join(package_kind)
        .join(listed_module_id)
        .join("manifest.yaml");
    let raw = fs::read_to_string(&manifest_path).with_context(|| {
        format!(
            "failed to read listed plugin package {listed_module_id} at {}",
            manifest_path.display()
        )
    })?;
    let manifest = parse_plugin_manifest(&raw)
        .with_context(|| format!("failed to parse {}", manifest_path.display()))?;
    let manifest_id = manifest.plugin_code()?;
    if manifest_id != listed_module_id {
        bail!(
            "plugin set identity mismatch: listed {listed_module_id} package declares {manifest_id}"
        );
    }
    if manifest.consumption_kind != expected_kind {
        bail!(
            "plugin set category mismatch for {listed_module_id}: expected {}, found {}",
            expected_kind.as_str(),
            manifest.consumption_kind.as_str()
        );
    }
    Ok(manifest)
}

fn load_host_contribution(
    api_workspace_root: &Path,
    listed_module_id: &str,
    manifest: &PluginManifestV1,
) -> Result<HostExtensionContributionManifest> {
    let contribution_path = api_workspace_root
        .join("plugins/host-extensions")
        .join(listed_module_id)
        .join(&manifest.runtime.entry);
    let raw = fs::read_to_string(&contribution_path)
        .with_context(|| format!("failed to read {}", contribution_path.display()))?;
    let contribution = parse_host_extension_contribution_manifest(&raw)
        .with_context(|| format!("failed to parse {}", contribution_path.display()))?;
    if contribution.extension_id != listed_module_id {
        bail!(
            "host extension identity mismatch: package {listed_module_id} contribution {}",
            contribution.extension_id
        );
    }
    if contribution.version != manifest.version {
        bail!(
            "host extension version mismatch: package {} contribution {}",
            manifest.version,
            contribution.version
        );
    }
    if !contribution.native.library.starts_with("builtin://") {
        bail!(
            "builtin host extension native library must use builtin://: {}",
            contribution.native.library
        );
    }
    Ok(contribution)
}

fn resolve_workspace_path(api_workspace_root: &Path, relative_path: &Path) -> Result<PathBuf> {
    if relative_path.is_absolute()
        || relative_path
            .components()
            .any(|component| matches!(component, Component::ParentDir))
    {
        bail!(
            "plugin set path must stay under API workspace: {}",
            relative_path.display()
        );
    }
    Ok(api_workspace_root.join(relative_path))
}
