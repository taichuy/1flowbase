//! Stable control-plane contracts implemented by host infrastructure adapters.

extern crate self as control_plane_contracts;

pub mod error;
pub mod ports;

pub use error::*;
pub use ports::*;

pub fn crate_name() -> &'static str {
    "control-plane-contracts"
}
