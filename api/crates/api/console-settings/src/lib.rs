#![recursion_limit = "256"]
//! System Settings, documentation, i18n and backup/recovery adapters.
#[macro_use]
mod test_support_visibility;
pub use api_host_kernel::*;
mod route_prefix;
pub mod runtime_data_model_docs;
pub mod system_backup;
pub mod system_recovery;
pub mod routes {
    pub use api_host_kernel::routes::*;
    pub mod core_console_i18n;
    pub mod runtime_i18n_catalog;
    #[path = "settings/mod.rs"]
    pub mod settings_group;
    pub use crate::route_prefix::PUBLIC_API_PATH_PREFIX;
    pub use settings_group::*;
    pub use settings_group::{billing, host_infrastructure, system};
}

#[doc(hidden)]
pub mod openapi;
