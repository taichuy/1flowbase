pub(crate) use api_plugins_models::managed_services::*;
#[cfg(test)]
pub(crate) mod record_permissions;
#[cfg(test)]
pub(crate) mod stream;
#[cfg_attr(test, api_test_selection::group(core))]
#[cfg(test)]
#[path = "_tests/mod.rs"]
mod tests;
