use crate::{app_state::ApiState, routes::console_route_assembly::ConsoleRouteAssembly};
pub use api_console_settings::routes::roles::*;
use std::sync::Arc;
pub fn router() -> axum::Router<Arc<ApiState>> {
    route_assembly().into_router()
}
pub fn route_assembly() -> ConsoleRouteAssembly<Arc<ApiState>> {
    ConsoleRouteAssembly::new()
        .merge(crate::managed_services::record_permissions::route_assembly())
        .merge(api_console_settings::routes::roles::route_assembly())
}
