pub use api_console_interaction::routes::local_commands::*;
#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
use interface_runtime::InterfaceContract;
#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
#[path = "_tests/mod.rs"]
mod tests;
