//! In-process RuntimeExtension host.
//!
//! The public boundary is [`RuntimeExtensionHost`] plus the stable ports owned by `runtime-core`.
//! Worker protocol, registry, process and package details remain implementation concerns.
//!
//! Internal Host types cannot be imported by consumers:
//! ```compile_fail
//! use runtime_extension_host::provider_host::ProviderHost;
//! ```

mod capability_host;
mod capability_stdio;
mod data_source_host;
mod data_source_stdio;
mod network_egress_host;
mod package_loader;
mod plugin_scope;
mod provider_host;
mod stdio_runtime;

mod runtime_host;

#[cfg(test)]
mod _tests;

pub use runtime_host::{RuntimeArtifactResolver, RuntimeExtensionHost};
