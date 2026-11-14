pub use api_plugins_models::routes::plugins::*;
#[cfg(not(test))]
pub use api_plugins_models::routes::plugins::extension_center;
#[cfg(test)]
#[path = "plugins/extension_center.rs"]
pub mod extension_center;
