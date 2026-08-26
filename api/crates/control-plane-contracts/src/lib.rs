//! Stable control-plane contracts implemented by host infrastructure adapters.

extern crate self as control_plane_contracts;

pub mod application_public_api;
pub mod console_policy_migration;
pub mod error;
pub mod i18n_catalog;
pub mod ports;
pub mod system_backup;
pub mod system_recovery;

pub use application_public_api::*;
pub use console_policy_migration::*;
pub use error::*;
pub use i18n_catalog::*;
pub use ports::*;
pub use system_backup::*;
pub use system_recovery::*;

pub fn crate_name() -> &'static str {
    "control-plane-contracts"
}

#[cfg(test)]
mod _tests;
