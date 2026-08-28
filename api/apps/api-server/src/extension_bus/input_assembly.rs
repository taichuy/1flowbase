use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Component, Path, PathBuf},
};

use anyhow::{bail, Context, Result};
use plugin_framework::{
    extension_bus::{
        compile_extension_graph, parse_deployment_plugin_set, Cardinality, ContractDescriptor,
        ContributionDescriptor, ContributionId, ContributionMode, ContributionOrdering,
        DeliverySemantics, DeploymentPluginSet, EffectiveExtensionGraph, ExtensionBusVersion,
        ExtensionPointDescriptor, ExtensionPointId, ExtensionPointKind, FailureSemantics,
        LifecycleSemantics, ModuleActivationDeclaration, ModuleDescriptor, ModuleDisableReason,
        ModuleId, ModuleKind, ModuleVersion, OrderingSemantics, OverridePolicy, PermissionCode,
        ScopeSemantics,
    },
    parse_host_extension_contribution_manifest, parse_plugin_manifest,
    HostExtensionContributionManifest, PluginConsumptionKind, PluginManifestV1,
};

use crate::routes::host_infrastructure::interface_operation::{
    HOST_INFRASTRUCTURE_PROVIDERS_VIEW_CONTRIBUTION_ID,
    HOST_INFRASTRUCTURE_PROVIDERS_VIEW_CONTRIBUTOR_ID,
    HOST_INFRASTRUCTURE_PROVIDERS_VIEW_OPERATION_ID, HOST_INFRASTRUCTURE_PROVIDERS_VIEW_PERMISSION,
    INTERFACE_OPERATION_CONTRACT_ID, INTERFACE_OPERATION_CONTRACT_VERSION,
    INTERFACE_OPERATION_OWNER_MODULE_ID, INTERFACE_OPERATION_POINT_ID,
};
use control_plane::frontend_block_catalog::{
    FRONTEND_BLOCK_CONTRIBUTION_CONTRACT_ID, FRONTEND_BLOCK_CONTRIBUTION_CONTRACT_VERSION,
    FRONTEND_BLOCK_CONTRIBUTION_POINT_ID, FRONTEND_BLOCK_ISOLATED_UI_MOUNT_PERMISSION,
    FRONTEND_BLOCK_TRUSTED_UI_MOUNT_PERMISSION,
};

pub const DEFAULT_PLUGIN_SET_PATH: &str = "plugins/sets/default.yaml";
pub const BOOT_CORE_MODULE_ID: &str = "1flowbase.boot-core";
pub const CACHE_STORE_EXTENSION_POINT_ID: &str = "1flowbase.boot.cache-store";
pub const CACHE_STORE_CONTRACT_ID: &str = "cache-store";
pub const CACHE_STORE_CONTRACT_VERSION: &str = "1";
pub const MODEL_PROVIDER_EXTENSION_POINT_ID: &str = "1flowbase.runtime.model-provider";
pub const MODEL_PROVIDER_CONTRACT_ID: &str = "model-provider";
pub const MODEL_PROVIDER_CONTRACT_VERSION: &str =
    plugin_framework::provider_contract::CURRENT_PROVIDER_CONTRACT;
pub const INTERFACE_COMPLETION_HOOK_POINT_ID: &str = "1flowbase.interface.completion";
pub const INTERFACE_COMPLETION_HOOK_CONTRIBUTION_ID: &str =
    "1flowbase.boot-core.interface.completion.observer";
pub const INTERFACE_LIFECYCLE_HOOK_CONTRACT_ID: &str = "interface-lifecycle-hook";
pub const INTERFACE_LIFECYCLE_HOOK_CONTRACT_VERSION: &str = "1";
pub use control_plane::ports::{
    RUNTIME_EVENT_AFTER_COMMIT_CONTRACT_ID, RUNTIME_EVENT_AFTER_COMMIT_POINT_ID,
    RUNTIME_EVENT_DIAGNOSTIC_CONTRACT_ID, RUNTIME_EVENT_DIAGNOSTIC_POINT_ID,
    RUNTIME_EVENT_LANE_CONTRACT_VERSION, RUNTIME_EVENT_REQUIRED_CONTRACT_ID,
    RUNTIME_EVENT_REQUIRED_POINT_ID,
};
pub use orchestration_runtime::provider_input_pipeline::{
    PROVIDER_INPUT_PIPELINE_CONTRACT_ID, PROVIDER_INPUT_PIPELINE_CONTRACT_VERSION,
    PROVIDER_INPUT_PIPELINE_POINT_ID,
};

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
    interface_operations: Vec<plugin_framework::HostExtensionInterfaceOperationManifest>,
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

    pub fn interface_operations(
        &self,
    ) -> &[plugin_framework::HostExtensionInterfaceOperationManifest] {
        &self.interface_operations
    }

    pub fn compile_graph(&self) -> Result<EffectiveExtensionGraph> {
        compile_extension_graph(self.module_descriptors.clone()).map_err(anyhow::Error::from)
    }

    pub fn compile_lifecycle_subscriber_plan(
        &self,
        graph: &EffectiveExtensionGraph,
    ) -> Result<plugin_framework::extension_bus::EffectiveLifecycleSubscriberPlan> {
        let mut bindings = Vec::new();
        for (_, manifest) in &self.host_extension_manifests {
            for subscription in &manifest.lifecycle_subscriptions {
                bindings.push(
                    plugin_framework::extension_bus::LifecycleSubscriberBinding {
                        contribution_id: lifecycle_subscription_contribution_id(
                            &manifest.extension_id,
                            &subscription.subscription_id,
                        )?,
                        subscription_id: subscription.subscription_id.clone(),
                        point_id: ExtensionPointId::new(subscription.point_id.clone())?,
                        fact_contract_id: subscription.fact.contract_id.clone(),
                        fact_contract_version: subscription.fact.contract_version.clone(),
                        handler_id: subscription.handler.contract_id.clone(),
                        handler_version: subscription.handler.contract_version.clone(),
                    },
                );
            }
        }
        plugin_framework::extension_bus::compile_lifecycle_subscriber_plan(graph, bindings)
            .map_err(anyhow::Error::from)
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
    let mut interface_operations = Vec::new();

    for module_id in plugin_set.host_extension_ids() {
        let manifest = load_package_manifest(
            api_workspace_root,
            "host-extensions",
            module_id,
            PluginConsumptionKind::HostExtension,
        )?;
        let contribution = load_host_contribution(api_workspace_root, module_id, &manifest)?;
        let activation = take_activation(&mut activations, module_id)?;
        module_descriptors.push(derive_host_module_descriptor(
            &manifest,
            &contribution,
            activation,
        )?);
        interface_operations.extend(contribution.interface_operations.iter().cloned());
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
        interface_operations,
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
        extension_points: vec![
            cache_store_extension_point()?,
            model_provider_extension_point()?,
            provider_input_pipeline_extension_point()?,
            runtime_event_required_extension_point()?,
            runtime_event_diagnostic_extension_point()?,
            runtime_event_after_commit_extension_point()?,
            interface_operation_extension_point()?,
            interface_completion_hook_extension_point()?,
            frontend_block_contribution_extension_point()?,
        ],
        contributions: vec![ContributionDescriptor {
            contribution_id: ContributionId::new(INTERFACE_COMPLETION_HOOK_CONTRIBUTION_ID)?,
            contributor_module_id: ModuleId::new(BOOT_CORE_MODULE_ID)?,
            point_id: ExtensionPointId::new(INTERFACE_COMPLETION_HOOK_POINT_ID)?,
            contract_version: plugin_framework::extension_bus::ContractVersion::new(
                INTERFACE_LIFECYCLE_HOOK_CONTRACT_VERSION,
            )?,
            required_permissions: BTreeSet::new(),
            mode: ContributionMode::Append,
            ordering: ContributionOrdering::default(),
        }],
    })
}

fn interface_completion_hook_extension_point() -> Result<ExtensionPointDescriptor> {
    Ok(ExtensionPointDescriptor {
        point_id: ExtensionPointId::new(INTERFACE_COMPLETION_HOOK_POINT_ID)?,
        owner_module_id: ModuleId::new(BOOT_CORE_MODULE_ID)?,
        point_kind: ExtensionPointKind::Pipeline,
        contract: ContractDescriptor::new(
            INTERFACE_LIFECYCLE_HOOK_CONTRACT_ID,
            INTERFACE_LIFECYCLE_HOOK_CONTRACT_VERSION,
        )?,
        scope: ScopeSemantics::System,
        cardinality: Cardinality::OneOrMore,
        ordering: OrderingSemantics::Dependency,
        failure: FailureSemantics::BestEffort,
        delivery: DeliverySemantics::Synchronous,
        lifecycle: LifecycleSemantics::Invocation,
        allowed_permissions: BTreeSet::new(),
        override_policy: OverridePolicy::Sealed,
    })
}

fn frontend_block_contribution_extension_point() -> Result<ExtensionPointDescriptor> {
    Ok(ExtensionPointDescriptor {
        point_id: ExtensionPointId::new(FRONTEND_BLOCK_CONTRIBUTION_POINT_ID)?,
        owner_module_id: ModuleId::new(BOOT_CORE_MODULE_ID)?,
        point_kind: ExtensionPointKind::Contribution,
        contract: ContractDescriptor::new(
            FRONTEND_BLOCK_CONTRIBUTION_CONTRACT_ID,
            FRONTEND_BLOCK_CONTRIBUTION_CONTRACT_VERSION,
        )?,
        scope: ScopeSemantics::Workspace,
        cardinality: Cardinality::Many,
        ordering: OrderingSemantics::Lexicographic,
        failure: FailureSemantics::FailClosed,
        delivery: DeliverySemantics::Synchronous,
        lifecycle: LifecycleSemantics::WorkspaceAssignment,
        allowed_permissions: [
            FRONTEND_BLOCK_TRUSTED_UI_MOUNT_PERMISSION,
            FRONTEND_BLOCK_ISOLATED_UI_MOUNT_PERMISSION,
        ]
        .into_iter()
        .map(PermissionCode::new)
        .collect::<std::result::Result<_, _>>()?,
        override_policy: OverridePolicy::Sealed,
    })
}

fn interface_operation_extension_point() -> Result<ExtensionPointDescriptor> {
    Ok(ExtensionPointDescriptor {
        point_id: ExtensionPointId::new(INTERFACE_OPERATION_POINT_ID)?,
        owner_module_id: ModuleId::new(INTERFACE_OPERATION_OWNER_MODULE_ID)?,
        point_kind: ExtensionPointKind::Contribution,
        contract: ContractDescriptor::new(
            INTERFACE_OPERATION_CONTRACT_ID,
            INTERFACE_OPERATION_CONTRACT_VERSION,
        )?,
        scope: ScopeSemantics::System,
        cardinality: Cardinality::Many,
        ordering: OrderingSemantics::Lexicographic,
        failure: FailureSemantics::FailClosed,
        delivery: DeliverySemantics::Synchronous,
        lifecycle: LifecycleSemantics::BootSnapshot,
        allowed_permissions: BTreeSet::from([PermissionCode::new(
            HOST_INFRASTRUCTURE_PROVIDERS_VIEW_PERMISSION,
        )?]),
        override_policy: OverridePolicy::Sealed,
    })
}

fn runtime_event_required_extension_point() -> Result<ExtensionPointDescriptor> {
    Ok(ExtensionPointDescriptor {
        point_id: ExtensionPointId::new(RUNTIME_EVENT_REQUIRED_POINT_ID)?,
        owner_module_id: ModuleId::new(BOOT_CORE_MODULE_ID)?,
        point_kind: ExtensionPointKind::EventStream,
        contract: ContractDescriptor::new(
            RUNTIME_EVENT_REQUIRED_CONTRACT_ID,
            RUNTIME_EVENT_LANE_CONTRACT_VERSION,
        )?,
        scope: ScopeSemantics::Global,
        cardinality: Cardinality::Many,
        ordering: OrderingSemantics::Dependency,
        failure: FailureSemantics::FailClosed,
        delivery: DeliverySemantics::RequiredStream,
        lifecycle: LifecycleSemantics::Invocation,
        allowed_permissions: BTreeSet::new(),
        override_policy: OverridePolicy::Sealed,
    })
}

fn runtime_event_diagnostic_extension_point() -> Result<ExtensionPointDescriptor> {
    Ok(ExtensionPointDescriptor {
        point_id: ExtensionPointId::new(RUNTIME_EVENT_DIAGNOSTIC_POINT_ID)?,
        owner_module_id: ModuleId::new(BOOT_CORE_MODULE_ID)?,
        point_kind: ExtensionPointKind::EventStream,
        contract: ContractDescriptor::new(
            RUNTIME_EVENT_DIAGNOSTIC_CONTRACT_ID,
            RUNTIME_EVENT_LANE_CONTRACT_VERSION,
        )?,
        scope: ScopeSemantics::Global,
        cardinality: Cardinality::Many,
        ordering: OrderingSemantics::Dependency,
        failure: FailureSemantics::BestEffort,
        delivery: DeliverySemantics::DiagnosticBestEffort,
        lifecycle: LifecycleSemantics::Invocation,
        allowed_permissions: BTreeSet::new(),
        override_policy: OverridePolicy::Sealed,
    })
}

fn runtime_event_after_commit_extension_point() -> Result<ExtensionPointDescriptor> {
    Ok(ExtensionPointDescriptor {
        point_id: ExtensionPointId::new(RUNTIME_EVENT_AFTER_COMMIT_POINT_ID)?,
        owner_module_id: ModuleId::new(BOOT_CORE_MODULE_ID)?,
        point_kind: ExtensionPointKind::EventStream,
        contract: ContractDescriptor::new(
            RUNTIME_EVENT_AFTER_COMMIT_CONTRACT_ID,
            RUNTIME_EVENT_LANE_CONTRACT_VERSION,
        )?,
        scope: ScopeSemantics::Global,
        cardinality: Cardinality::Many,
        ordering: OrderingSemantics::Dependency,
        failure: FailureSemantics::IsolateContribution,
        delivery: DeliverySemantics::AfterCommitDurable,
        lifecycle: LifecycleSemantics::Invocation,
        allowed_permissions: BTreeSet::new(),
        override_policy: OverridePolicy::Sealed,
    })
}

fn provider_input_pipeline_extension_point() -> Result<ExtensionPointDescriptor> {
    use orchestration_runtime::provider_input_pipeline::{
        REWRITE_MESSAGES_PERMISSION, REWRITE_MODEL_PARAMETERS_PERMISSION,
        REWRITE_RESPONSE_FORMAT_PERMISSION, REWRITE_SYSTEM_PERMISSION, REWRITE_TOOLS_PERMISSION,
    };

    Ok(ExtensionPointDescriptor {
        point_id: ExtensionPointId::new(PROVIDER_INPUT_PIPELINE_POINT_ID)?,
        owner_module_id: ModuleId::new(BOOT_CORE_MODULE_ID)?,
        point_kind: ExtensionPointKind::Pipeline,
        contract: ContractDescriptor::new(
            PROVIDER_INPUT_PIPELINE_CONTRACT_ID,
            PROVIDER_INPUT_PIPELINE_CONTRACT_VERSION,
        )?,
        scope: ScopeSemantics::Global,
        cardinality: Cardinality::Many,
        ordering: OrderingSemantics::Dependency,
        failure: FailureSemantics::FailClosed,
        delivery: DeliverySemantics::Synchronous,
        lifecycle: LifecycleSemantics::Invocation,
        allowed_permissions: [
            REWRITE_MESSAGES_PERMISSION,
            REWRITE_SYSTEM_PERMISSION,
            REWRITE_TOOLS_PERMISSION,
            REWRITE_RESPONSE_FORMAT_PERMISSION,
            REWRITE_MODEL_PARAMETERS_PERMISSION,
        ]
        .into_iter()
        .map(PermissionCode::new)
        .collect::<std::result::Result<_, _>>()?,
        override_policy: OverridePolicy::Sealed,
    })
}

fn model_provider_extension_point() -> Result<ExtensionPointDescriptor> {
    Ok(ExtensionPointDescriptor {
        point_id: ExtensionPointId::new(MODEL_PROVIDER_EXTENSION_POINT_ID)?,
        owner_module_id: ModuleId::new(BOOT_CORE_MODULE_ID)?,
        point_kind: ExtensionPointKind::Slot,
        contract: ContractDescriptor::new(
            MODEL_PROVIDER_CONTRACT_ID,
            MODEL_PROVIDER_CONTRACT_VERSION,
        )?,
        scope: ScopeSemantics::Global,
        cardinality: Cardinality::Many,
        ordering: OrderingSemantics::Lexicographic,
        failure: FailureSemantics::FailClosed,
        delivery: DeliverySemantics::Synchronous,
        lifecycle: LifecycleSemantics::RuntimeWorker,
        allowed_permissions: BTreeSet::new(),
        override_policy: OverridePolicy::Sealed,
    })
}

fn cache_store_extension_point() -> Result<ExtensionPointDescriptor> {
    Ok(ExtensionPointDescriptor {
        point_id: ExtensionPointId::new(CACHE_STORE_EXTENSION_POINT_ID)?,
        owner_module_id: ModuleId::new(BOOT_CORE_MODULE_ID)?,
        point_kind: ExtensionPointKind::Slot,
        contract: ContractDescriptor::new(CACHE_STORE_CONTRACT_ID, CACHE_STORE_CONTRACT_VERSION)?,
        scope: ScopeSemantics::System,
        cardinality: Cardinality::ExactlyOne,
        ordering: OrderingSemantics::Lexicographic,
        failure: FailureSemantics::FailClosed,
        delivery: DeliverySemantics::Synchronous,
        lifecycle: LifecycleSemantics::BootSnapshot,
        allowed_permissions: BTreeSet::new(),
        override_policy: OverridePolicy::Sealed,
    })
}

fn derive_host_module_descriptor(
    manifest: &PluginManifestV1,
    contribution: &HostExtensionContributionManifest,
    activation: ModuleActivationDeclaration,
) -> Result<ModuleDescriptor> {
    let mut descriptor = derive_module_descriptor(manifest, ModuleKind::TrustedHost, activation)?;
    descriptor.contributions = contribution
        .infrastructure_providers
        .iter()
        .filter(|provider| provider.contract == CACHE_STORE_CONTRACT_ID)
        .map(|provider| {
            Ok(ContributionDescriptor {
                contribution_id: infrastructure_provider_contribution_id(
                    &contribution.extension_id,
                    &provider.contract,
                    &provider.provider_code,
                )?,
                contributor_module_id: ModuleId::new(contribution.extension_id.as_str())?,
                point_id: ExtensionPointId::new(CACHE_STORE_EXTENSION_POINT_ID)?,
                contract_version: plugin_framework::extension_bus::ContractVersion::new(
                    CACHE_STORE_CONTRACT_VERSION,
                )?,
                required_permissions: BTreeSet::new(),
                mode: ContributionMode::Append,
                ordering: ContributionOrdering::default(),
            })
        })
        .collect::<Result<Vec<_>>>()?;
    for operation in &contribution.interface_operations {
        descriptor.contributions.push(ContributionDescriptor {
            contribution_id: ContributionId::new(format!(
                "{}.interface-operation.{}",
                contribution.extension_id, operation.operation_id
            ))?,
            contributor_module_id: ModuleId::new(contribution.extension_id.as_str())?,
            point_id: ExtensionPointId::new(INTERFACE_OPERATION_POINT_ID)?,
            contract_version: plugin_framework::extension_bus::ContractVersion::new(
                INTERFACE_OPERATION_CONTRACT_VERSION,
            )?,
            required_permissions: BTreeSet::from([PermissionCode::new(
                operation.required_core_permission.clone(),
            )?]),
            mode: ContributionMode::Append,
            ordering: ContributionOrdering::default(),
        });
    }
    for subscription in &contribution.lifecycle_subscriptions {
        descriptor.contributions.push(ContributionDescriptor {
            contribution_id: lifecycle_subscription_contribution_id(
                &contribution.extension_id,
                &subscription.subscription_id,
            )?,
            contributor_module_id: ModuleId::new(contribution.extension_id.as_str())?,
            point_id: ExtensionPointId::new(subscription.point_id.clone())?,
            contract_version: plugin_framework::extension_bus::ContractVersion::new(
                RUNTIME_EVENT_LANE_CONTRACT_VERSION,
            )?,
            required_permissions: BTreeSet::new(),
            mode: ContributionMode::Append,
            ordering: ContributionOrdering::default(),
        });
    }
    if contribution.extension_id == HOST_INFRASTRUCTURE_PROVIDERS_VIEW_CONTRIBUTOR_ID {
        descriptor.granted_permissions.insert(PermissionCode::new(
            HOST_INFRASTRUCTURE_PROVIDERS_VIEW_PERMISSION,
        )?);
    }
    if contribution.extension_id == HOST_INFRASTRUCTURE_PROVIDERS_VIEW_CONTRIBUTOR_ID
        && contribution.interface_operations.iter().any(|operation| {
            operation.operation_id == HOST_INFRASTRUCTURE_PROVIDERS_VIEW_OPERATION_ID
        })
        && !descriptor.contributions.iter().any(|candidate| {
            candidate.contribution_id.as_str() == HOST_INFRASTRUCTURE_PROVIDERS_VIEW_CONTRIBUTION_ID
        })
    {
        bail!("official providers view interface operation contribution id mismatch");
    }
    Ok(descriptor)
}

fn lifecycle_subscription_contribution_id(
    extension_id: &str,
    subscription_id: &str,
) -> Result<ContributionId> {
    ContributionId::new(format!("{extension_id}.lifecycle.{subscription_id}"))
        .map_err(anyhow::Error::from)
}

pub(crate) fn infrastructure_provider_contribution_id(
    extension_id: &str,
    contract: &str,
    provider_code: &str,
) -> Result<ContributionId> {
    Ok(ContributionId::new(format!(
        "{extension_id}.infrastructure.{contract}.{provider_code}"
    ))?)
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
