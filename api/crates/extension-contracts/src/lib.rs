//! Stable contracts shared by extension packaging and runtime consumers.
//!
//! Package intake, installation, registry, and compilation remain owned by
//! `plugin-framework`; this crate owns only canonical wire and runtime types.

extern crate self as extension_contracts;

pub mod data_model_template_contract;
pub mod data_source_contract;
pub mod error;
pub mod extension_bus;
pub mod lifecycle_contract;
pub mod network_egress_contract;
pub mod network_egress_provider_contract;
pub mod package_intake_contract;
pub mod plugin_data_model_contract;
pub mod provider_contract;
pub mod runtime_target;
pub mod semantic_terminal;

pub use data_model_template_contract::*;
pub use data_source_contract::*;
pub use error::*;
pub use extension_bus::*;
pub use lifecycle_contract::*;
pub use network_egress_contract::*;
pub use network_egress_provider_contract::*;
pub use package_intake_contract::*;
pub use plugin_data_model_contract::*;
pub use provider_contract::*;
pub use runtime_target::*;
pub use semantic_terminal::*;

pub fn crate_name() -> &'static str {
    "extension-contracts"
}

#[cfg(test)]
mod _tests;
