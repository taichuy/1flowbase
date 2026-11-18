use std::sync::Arc;

use crate::{
    app_state::ApiState,
    routes::{
        console_route_assembly::{console_get, console_patch, ConsoleRouteAssembly},
        network_center::{
            create_network_egress_route, delete_network_egress_route, list_network_egress_routes,
            update_network_egress_route,
        },
    },
};

pub fn route_assembly() -> ConsoleRouteAssembly<Arc<ApiState>> {
    use access_control::ConsoleRouteOwnership::ConsoleOperation;

    ConsoleRouteAssembly::new()
        .route(
            "/network-center/routes",
            console_get(
                list_network_egress_routes,
                ConsoleOperation("network_egress_routes.list".to_string()),
            )
            .post(
                create_network_egress_route,
                ConsoleOperation("network_egress_routes.create".to_string()),
            ),
        )
        .route(
            "/network-center/routes/:route_id",
            console_patch(
                update_network_egress_route,
                ConsoleOperation("network_egress_routes.update".to_string()),
            )
            .delete(
                delete_network_egress_route,
                ConsoleOperation("network_egress_routes.delete".to_string()),
            ),
        )
}
