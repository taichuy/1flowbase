//! Stable control-plane contracts implemented by host infrastructure adapters.

extern crate self as control_plane_contracts;

pub mod application_public_api;
pub mod console_policy_migration;
pub mod error;
pub mod ports;

pub use application_public_api::*;
pub use console_policy_migration::*;
pub use error::*;
pub use ports::*;

pub fn crate_name() -> &'static str {
    "control-plane-contracts"
}

#[cfg(test)]
mod _tests;
