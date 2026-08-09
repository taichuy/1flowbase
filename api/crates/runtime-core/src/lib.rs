extern crate self as runtime_core;

pub mod capability_slots;
pub mod data_model_template_registry;
pub mod general_data_model_template;
pub mod model_metadata;
pub mod resource_descriptor;
pub mod resource_registry;
pub mod runtime_acl;
pub mod runtime_engine;
pub mod runtime_model_registry;
pub mod runtime_record_repository;

pub fn crate_name() -> &'static str {
    "runtime-core"
}

#[cfg(test)]
pub mod _tests;
