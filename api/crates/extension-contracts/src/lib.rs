//! Stable contracts shared by extension packaging and runtime consumers.
//!
//! Package intake, installation, registry, and compilation remain owned by
//! `plugin-framework`; this crate owns only canonical wire and runtime types.

extern crate self as extension_contracts;
pub mod authentication_contract;
pub use authentication_contract::*;
pub mod application_logs_contract;
pub use application_logs_contract::*;
#[path = "host_services/http_egress_contract.rs"]
pub mod http_egress_contract;
pub use http_egress_contract::*;

pub mod data_model_template_contract;
pub mod data_source_contract;
pub mod error;
pub mod extension_bus;
#[path = "managed_contributions/lifecycle_contract.rs"]
pub mod lifecycle_contract;
#[path = "managed_contributions/managed_event_contract.rs"]
pub mod managed_event_contract;
#[path = "managed_contributions/managed_hook_contract.rs"]
pub mod managed_hook_contract;
#[path = "managed_contributions/managed_interface_contract.rs"]
pub mod managed_interface_contract;
#[path = "managed_contributions/managed_interface_reference.rs"]
pub mod managed_interface_reference;
pub mod network_egress_contract;
pub mod network_egress_provider_contract;
pub mod notification_contract;
pub mod package_intake_contract;
#[path = "host_services/plugin_data_contract.rs"]
pub mod plugin_data_contract;
#[path = "host_services/plugin_data_model_contract.rs"]
pub mod plugin_data_model_contract;
pub mod provider_contract;
pub mod provider_distribution_contract;
pub mod runtime_target;
pub mod semantic_terminal;
pub mod stdio_multiplex;

pub use data_model_template_contract::*;
pub use data_source_contract::*;
pub use error::*;
pub use extension_bus::*;
pub use lifecycle_contract::*;
pub use managed_event_contract::*;
pub use managed_hook_contract::*;
pub use managed_interface_contract::*;
pub use managed_interface_reference::*;
pub use network_egress_contract::*;
pub use network_egress_provider_contract::*;
pub use notification_contract::*;
pub use package_intake_contract::*;
pub use plugin_data_contract::*;
pub use plugin_data_model_contract::*;
pub use provider_contract::*;
pub use provider_distribution_contract::*;
pub use runtime_target::*;
pub use semantic_terminal::*;
pub use stdio_multiplex::*;

pub fn crate_name() -> &'static str {
    "extension-contracts"
}

#[cfg(test)]
mod _tests;

pub mod capability_contract;
pub use capability_contract::*;

#[path = "host_services/plugin_credential_contract.rs"]
pub mod plugin_credential_contract;
pub use plugin_credential_contract::*;

pub mod file_import_contract;
pub use file_import_contract::*;

pub mod file_read_contract;
pub use file_read_contract::*;

#[path = "managed_contributions/managed_service_contract.rs"]
pub mod managed_service_contract;
pub use managed_service_contract::*;
