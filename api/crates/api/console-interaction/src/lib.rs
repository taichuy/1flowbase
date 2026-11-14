#![recursion_limit = "256"]
//! Console interaction, identity and host operation adapters.
#[macro_use]
mod test_support_visibility;
pub use api_host_kernel::*;
pub mod communication_runtime;
pub mod event_center;
pub mod local_command;
pub mod local_terminal;
pub mod routes {
    pub use api_host_kernel::routes::*;
    pub mod console_interface;
    pub mod assistant;
    pub mod debug_run_stream { pub use api_host_kernel::debug_stream_contract::*; }
    pub mod client_collectors;
    pub mod communications;
    pub mod events;
    pub mod files;
    pub mod frontstage;
    #[path = "identity/mod.rs"]
    pub mod identity_group;
    pub use identity_group::*;
    pub mod local_commands;
    pub mod network_center;
    pub mod notifications;
    pub mod terminal;
    pub mod plugins { pub use api_host_kernel::plugin_transport::*; }
}

#[doc(hidden)]
pub mod openapi;
