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
mod condition;
use condition::strict_condition;
#[cfg(test)]
#[path = "_tests/mod.rs"]
mod tests;
