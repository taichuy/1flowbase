extern crate self as storage_durable;

mod backend_kind;
mod runtime;

pub use backend_kind::DurableBackendKind;
pub use runtime::{
    build_main_durable_postgres, build_main_durable_postgres_with_max_connections,
    build_main_durable_postgres_with_pool_settings, MainDurableRuntime, MainDurableStore,
};
pub use storage_postgres::{execute_native_sql, PgPoolSettings};

pub fn crate_name() -> &'static str {
    "storage-durable"
}

#[cfg(test)]
mod _tests;
