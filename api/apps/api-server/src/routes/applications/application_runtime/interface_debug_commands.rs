pub use api_applications::routes::application_runtime::interface_debug_commands::*;
#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
use std::sync::Arc;
#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
use uuid::Uuid;
#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
#[path = "_tests/generation_factory.rs"]
mod generation_factory_tests;
