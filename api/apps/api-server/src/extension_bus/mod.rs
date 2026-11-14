pub use api_host_kernel::extension_bus::*;
pub(crate) use api_host_kernel::extension_bus::{
    input_assembly, managed_interface, managed_projection,
};
pub(crate) mod authentication_activation;
mod boot_snapshot;
pub use self::boot_snapshot::compile_extension_boot_snapshot;
pub(crate) use self::boot_snapshot::SnapshotAssembly;
mod managed_activation;
#[cfg(test)]
pub(crate) use self::managed_activation::validate_interface_protocol;
pub(crate) use self::managed_activation::{
    ManagedCommunicationCommandProvider, ManagedExtensionComposition,
    ManagedWorkspacePublicationSource,
};
mod interface_contributions;
pub(crate) use self::interface_contributions::{
    production_interface_contributions, InterfaceContributionCollectorAssembly,
};
