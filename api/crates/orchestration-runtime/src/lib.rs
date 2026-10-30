extern crate self as orchestration_runtime;

pub mod answer_presentation;
pub mod answer_projection;
pub mod binding_runtime;
#[path = "code_execution/code_executor_capability.rs"]
pub mod code_executor_capability;
#[path = "code_execution/code_runtime.rs"]
mod code_runtime;
pub mod compiled_plan;
pub mod compiler;
pub mod container_contract;
pub mod execution_engine;
pub mod execution_state;
pub mod node_error_policy;
pub mod node_errors;
pub mod output_schema;
pub mod payload_builder;
#[path = "code_execution/preview_executor.rs"]
pub mod preview_executor;
pub mod provider_input_pipeline;
pub mod runtime_backend;
pub mod transport_session;

pub fn crate_name() -> &'static str {
    "orchestration-runtime"
}

#[cfg(test)]
pub mod _tests;
