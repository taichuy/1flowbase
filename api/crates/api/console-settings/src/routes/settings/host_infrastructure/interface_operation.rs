use crate::app_state::ApiState;
use access_control::ConsoleOperationRegistry;
use control_plane::ports::RoleConsolePolicyReader;
use interface_runtime::{
    AuthorizationAdapterReference, InterfaceAuthorizationError, InterfaceAuthorizationFuture,
    InterfaceAuthorizationPort, InterfaceAuthorizationRequest, InterfaceInvocationKernel,
};
use std::sync::Arc;

pub const INTERFACE_OPERATION_POINT_ID: &str = "1flowbase.application.interface-operation";
pub const INTERFACE_OPERATION_CONTRACT_ID: &str = "interface-operation";
pub const INTERFACE_OPERATION_CONTRACT_VERSION: &str = "1";
pub const INTERFACE_OPERATION_OWNER_MODULE_ID: &str = "1flowbase.boot-core";

pub use api_host_kernel::console_invocation::{invocation_kernel, invocation_kernel_with_admission};

pub(crate) fn is_active_interface_route(state: &ApiState, method: &str, path: &str) -> bool {
    state
        .extension_boot_snapshot
        .as_ref()
        .and_then(|snapshot| snapshot.interface_registry())
        .map(|registry| registry.snapshot())
        .is_some_and(|snapshot| {
            snapshot.bindings().any(|binding| {
                binding.projection().http_route().is_some_and(|route| {
                    route.method().eq_ignore_ascii_case(method)
                        && crate::routes::console_route_assembly::route_templates_match(
                            route.path(),
                            path,
                        )
                })
            })
        })
}
