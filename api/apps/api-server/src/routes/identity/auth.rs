#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
use super::login_entries_interface;
pub use api_console_interaction::routes::auth::*;
#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
use control_plane::auth::AuthenticatorRegistry;
#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
use uuid::Uuid;
#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
#[path = "../../_tests/identity_routes/entry_action.rs"]
mod entry_action_tests;
