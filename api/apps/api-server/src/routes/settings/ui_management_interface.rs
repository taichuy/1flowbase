pub(crate) use api_console_settings::routes::ui_management_interface::*;
#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(all(test, not(feature = "root-1805-consumer-fixture")))]
#[path = "ui_management_interface/_tests/settings_module_assets.rs"]
mod settings_module_asset_tests;

#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
use interface_runtime::UserPrincipal;
