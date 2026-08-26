extern crate self as storage_durable;

mod backend_kind;
pub mod model_metadata;
pub mod resource_descriptor;
pub mod runtime_model_availability;
pub mod runtime_record_repository;

pub use backend_kind::DurableBackendKind;

pub fn crate_name() -> &'static str {
    "storage-durable"
}

#[cfg(test)]
mod _tests;
