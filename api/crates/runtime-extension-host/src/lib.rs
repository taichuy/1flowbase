//! In-process RuntimeExtension host.
//!
//! The public boundary is [`RuntimeExtensionHost`] plus the stable ports owned by `runtime-core`.
//! Worker protocol, registry, process and package details remain implementation concerns.

pub mod capability_host;
pub mod capability_stdio;
pub mod data_source_host;
pub mod data_source_stdio;
pub mod network_egress_host;
pub mod package_loader;
mod plugin_scope;
pub mod provider_host;
pub mod stdio_runtime;

mod runtime_host;

pub use runtime_host::{RuntimeArtifactResolver, RuntimeExtensionHost};
