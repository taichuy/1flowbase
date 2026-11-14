use crate::{app_state::ApiState, routes::console_route_assembly::ConsoleRouteAssembly};
use std::sync::Arc;
pub fn router() -> axum::Router<Arc<ApiState>> {
    route_assembly().into_router()
}
pub fn route_assembly() -> ConsoleRouteAssembly<Arc<ApiState>> {
    api_plugins_models::plugin_data_model_mounts::route_assembly()
        .merge(api_console_settings::routes::data_models::route_assembly())
}
