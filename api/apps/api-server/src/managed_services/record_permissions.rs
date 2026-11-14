use serde_json::json;
pub(crate) use api_plugins_models::managed_services::record_permissions::*;
#[cfg_attr(test, api_test_selection::group(core))]
#[path = "_tests/record_permissions.rs"]
mod tests;
