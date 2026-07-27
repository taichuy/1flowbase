extern crate self as plugin_framework;

pub mod artifact_reconcile;
pub mod assignment;
pub mod capability_kind;
pub mod data_source_contract;
pub mod data_source_package;
pub mod error;
pub mod frontend_module_asset;
pub mod host_contract;
pub mod host_extension_contribution;
pub mod host_extension_dropin;
pub mod host_extension_manifest;
pub mod host_extension_registry;
pub mod installation;
pub mod manifest_v1;
pub mod package_intake;
pub mod provider_contract;
pub mod provider_package;
pub mod runtime_target;
pub mod scope_provider_contract;

pub use artifact_reconcile::*;
pub use assignment::*;
pub use capability_kind::*;
pub use data_source_contract::*;
pub use data_source_package::*;
pub use error::*;
pub use frontend_module_asset::*;
pub use host_contract::{HostContractCode, RuntimeSlotCode, StorageImplementationKind};
pub use host_extension_contribution::{
    parse_host_extension_contribution_manifest, AuthProviderContributionManifest,
    HostExtensionBootstrapPhase, HostExtensionContributionManifest, HostExtensionMigrationManifest,
    HostExtensionNativeEntrypointManifest, HostExtensionRouteActionManifest,
    HostExtensionRouteManifest, HostExtensionWorkerManifest, HostInfrastructureProviderManifest,
};
pub use host_extension_dropin::*;
pub use host_extension_manifest::{
    parse_host_extension_manifest, HostExtensionActivationPhase, HostExtensionDependencyManifest,
    HostExtensionInterfaceManifest, HostExtensionLoadOrderManifest, HostExtensionManifestV1,
    HostExtensionSourceKind, HostExtensionStorageManifest,
};
pub use host_extension_registry::{HostExtensionRegistry, RegisteredHostExtension};
pub use installation::*;
pub use manifest_v1::{
    parse_plugin_manifest, FrontendBlockContextContractManifest, FrontendBlockContributionManifest,
    FrontendBlockPermissionsManifest, FrontendComponentContractManifest,
    FrontendComponentExampleManifest, FrontendComponentPropManifest,
    FrontendComponentUpstreamManifest, FrontendModuleAssetManifest,
    FrontendModuleAssetRoleManifest, FrontendModuleBindingManifest,
    NodeContributionDependencyManifest, NodeContributionManifest, PluginExecutionMode,
    PluginManifestV1, PluginPermissionManifest, PluginRuntimeLimits, PluginRuntimeManifest,
};
pub use package_intake::*;
pub use provider_contract::*;
pub use provider_package::*;
pub use runtime_target::*;
pub use scope_provider_contract::*;

pub fn crate_name() -> &'static str {
    "plugin-framework"
}

#[cfg(test)]
pub mod _tests;
