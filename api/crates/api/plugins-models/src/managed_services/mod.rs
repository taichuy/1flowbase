//! Generation-frozen managed services. Package declarations become ordinary host-owned console APIs.
test_support_item! { , []
mod event_projection;
}
test_support_item! { , []
mod interface;
}
mod record_acl;
pub mod record_permissions;
mod registration;
mod routes;
pub mod stream;
pub mod stream_ticket;
pub use interface::registry_contribution;
pub use registration::{
    append_openapi, load, load_installations, ManagedServiceRegistration,
};
pub use routes::route_assembly;

pub use api_host_kernel::managed_services::ManagedServiceFailure;

#[doc(hidden)]
#[cfg(any(test, feature = "test-support"))]
#[path = "_tests/support/mod.rs"]
pub mod support;

#[cfg(test)]
#[path = "_tests/mod.rs"]
mod tests;
