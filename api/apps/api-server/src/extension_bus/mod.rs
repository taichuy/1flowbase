pub use api_host_kernel::extension_bus::*;
pub mod authentication_activation;
pub mod boot_snapshot;
pub use self::boot_snapshot::compile_extension_boot_snapshot;
pub(crate) use self::boot_snapshot::SnapshotAssembly;
pub mod managed_activation;
#[cfg_attr(test, api_test_selection::group(contracts))]
#[cfg(test)]
pub(crate) use self::managed_activation::validate_interface_protocol;
pub(crate) use self::managed_activation::{
    ManagedCommunicationCommandProvider, ManagedExtensionComposition,
    ManagedWorkspacePublicationSource,
};
pub mod interface_contributions;
pub(crate) use self::interface_contributions::{
    production_interface_contributions, InterfaceContributionCollectorAssembly,
};
