use crate::{
    app_state::ApiState,
    routes::{
        console_route_assembly::{console_get, ConsoleRouteAssembly},
        docs,
    },
};
use access_control::{
    ConsoleRouteOwnership::ConsoleOperation, MODEL_DEFINITIONS_OPENAPI_VIEW_OPERATION_ID,
};
use std::sync::Arc;
pub fn router() -> axum::Router<Arc<ApiState>> {
    route_assembly().into_router()
}
pub fn route_assembly() -> ConsoleRouteAssembly<Arc<ApiState>> {
    ConsoleRouteAssembly::new().route(
        "/settings/data-models/model-definitions/:model_id/openapi.json",
        console_get(
            docs::get_data_model_openapi,
            ConsoleOperation(MODEL_DEFINITIONS_OPENAPI_VIEW_OPERATION_ID.to_string()),
        ),
    )
}
