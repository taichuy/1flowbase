#![recursion_limit = "256"]
//! Application public HTTP protocol domain. Host assembly remains in api-server.
#[macro_use]
mod test_support_visibility;
pub use api_host_kernel::*;
pub mod agent_logs_protocol;
pub mod application_public_docs;
pub mod routes {
    pub use api_host_kernel::routes::*;
    pub mod application_public_api;
}

#[doc(hidden)]
pub mod openapi;
