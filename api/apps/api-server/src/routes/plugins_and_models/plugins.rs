#[cfg(not(test))]
pub use api_plugins_models::routes::plugins::extension_center;
pub use api_plugins_models::routes::plugins::*;
#[cfg(test)]
#[path = "plugins/extension_center.rs"]
pub mod extension_center;
