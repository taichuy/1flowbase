#![recursion_limit = "256"]
//! Plugin, model, data source and managed service adapters.
#[macro_use]
mod test_support_visibility;
pub use api_host_kernel::*;
pub mod managed_services;
pub mod plugin_data_model_mounts;
pub mod routes {
    pub use api_host_kernel::routes::*;
    #[cfg(test)]
    pub(crate) mod _tests;
    pub mod console_interface;
    #[path = "plugins_and_models/mod.rs"]
    pub mod plugins_and_models_group;
    pub use plugins_and_models_group::*;
}
