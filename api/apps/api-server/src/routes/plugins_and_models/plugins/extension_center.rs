pub(crate) use api_plugins_models::routes::plugins::extension_center::*;
#[cfg_attr(test, api_test_selection::group(routes))]
#[path = "extension_center/_tests/mod.rs"]
mod _tests;
#[cfg_attr(test, api_test_selection::group(routes))]
#[path = "extension_center/managed_schema.rs"]
mod managed_schema;
