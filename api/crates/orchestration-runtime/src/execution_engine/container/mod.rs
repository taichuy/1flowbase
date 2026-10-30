//! Bounded container execution and durable transition contracts.
mod state;
pub use state::*;
mod interpreter;
mod recovery;
mod runtime;
pub(crate) use interpreter::*;
pub(crate) use recovery::{execute_root, recover_native_container, waiting_execution_id};
pub use recovery::{
    recover_container_with_runtime_context_and_lifecycle,
    resume_scoped_container_with_runtime_context_and_lifecycle,
};
pub(crate) use runtime::*;
pub use runtime::{deadline_expired, scoped_execution_identity};
mod root;
pub(crate) use root::*;
mod condition;
use condition::strict_condition;
#[cfg(test)]
#[path = "_tests/mod.rs"]
mod tests;

mod frame_terminal;
use frame_terminal::commit_frame_terminal;

mod round_terminal;
use round_terminal::commit_round_terminal;

/// The borrowed checkpoint cursor at entry to a container node.
pub(crate) struct ContainerEntryPosition<'a> {
    pub(super) pool: &'a serde_json::Map<String, serde_json::Value>,
    pub(super) index: usize,
    pub(super) active: &'a std::collections::BTreeSet<String>,
}

/// Fields staged together by the round terminal transaction owner.
struct RoundTerminalResult {
    output: serde_json::Value,
    variables: serde_json::Map<String, serde_json::Value>,
    error_payload: Option<serde_json::Value>,
}
