#![recursion_limit = "256"]
//! Application management, orchestration, debug runtime, archives and log exchange.
#[macro_use]
mod test_support_visibility;
pub use api_host_kernel::*;
pub mod application_log_exchange;
pub mod routes {
    pub use api_host_kernel::routes::*;
    #[cfg(test)]
    pub(crate) mod _tests;
    #[path = "applications/mod.rs"]
    pub mod applications_group;
    pub use applications_group::{
        application_api, application_orchestration, application_runtime, applications,
        debug_run_stream,
    };
}
