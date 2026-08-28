//! Versioned, deterministic contract compilation for the Extension Bus.
//!
//! This module only derives an immutable effective graph from discovered descriptors. It owns no
//! discovery, persistence, activation, or runtime dispatch behavior.

mod compiler;
mod deployment_set;
mod hook_plan;

pub use compiler::{compile_extension_graph, CompilationError};
pub use deployment_set::*;
pub use extension_contracts::extension_bus::*;
pub use hook_plan::*;
