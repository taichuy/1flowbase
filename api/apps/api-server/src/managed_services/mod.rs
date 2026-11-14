//! Generation-frozen managed services. Package declarations become ordinary host-owned console APIs.
mod event_projection;
mod interface;
mod record_acl;
pub(crate) mod record_permissions;
mod registration;
mod routes;
pub(crate) mod stream;
pub(crate) mod stream_ticket;
pub(crate) use interface::registry_contribution;
pub(crate) use registration::{
    append_openapi, load, load_installations, ManagedServiceRegistration,
};
pub(crate) use routes::route_assembly;

pub use api_host_kernel::managed_services::ManagedServiceFailure;
#[cfg_attr(test, api_test_selection::group(core))]
#[cfg(test)]
#[path = "_tests/mod.rs"]
mod tests;
