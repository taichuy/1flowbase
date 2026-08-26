extern crate self as storage_durable;

mod backend_kind;

pub use backend_kind::DurableBackendKind;

pub fn crate_name() -> &'static str {
    "storage-durable"
}

#[cfg(test)]
mod _tests;
