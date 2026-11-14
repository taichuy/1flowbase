#[path = "app_state.rs"]
pub mod app_state;
#[path = "../slices/console_contracts/console_operation_compilation.rs"]
pub mod console_operation_compilation;
#[path = "../slices/console_contracts/console_surface_registry.rs"]
pub mod console_surface_registry;
#[path = "error_response.rs"]
pub mod error_response;
#[path = "extension_bus/mod.rs"]
pub mod extension_bus;
#[path = "../slices/console_contracts/external_endpoint_catalog.rs"]
pub mod external_endpoint_catalog;
#[path = "../slices/console_contracts/external_route_assembly.rs"]
pub mod external_route_assembly;
#[path = "boot_composition/generation_dispatch.rs"]
pub mod generation_dispatch;
#[path = "openapi_docs.rs"]
pub mod openapi_docs;
#[path = "openapi_interface/mod.rs"]
pub mod openapi_interface;
#[path = "response.rs"]
pub mod response;

mod application_runtime_shared;
pub use application_runtime_shared::*;

pub mod builtin_mcp;
pub mod mcp_catalog;
pub mod resource_import_context;
pub mod resource_import_ports;

pub mod console_settings_ports;
pub mod frontend_block_dependencies;
