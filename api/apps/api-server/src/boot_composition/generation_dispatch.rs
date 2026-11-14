use crate::{app_state::ApiState, error_response::ApiError};
pub use api_host_kernel::generation_dispatch::*;
use std::sync::Arc;
pub(crate) trait GenerationDispatchAssembly {
    fn compile_router(
        &self,
        state: &Arc<ApiState>,
        registry: &interface_runtime::CompiledInterfaceRegistry,
        assembly: Option<
            crate::routes::console_route_assembly::ConsoleRouteAssembly<Arc<ApiState>>,
        >,
    ) -> Result<(), ApiError>;
    #[cfg_attr(test, api_test_selection::group(contracts))]
    #[cfg(test)]
    fn compile(state: &Arc<ApiState>) -> Result<Arc<GenerationDispatch>, ApiError>;
}
impl GenerationDispatchAssembly for GenerationDispatch {
    fn compile_router(
        &self,
        state: &Arc<ApiState>,
        registry: &interface_runtime::CompiledInterfaceRegistry,
        assembly: Option<
            crate::routes::console_route_assembly::ConsoleRouteAssembly<Arc<ApiState>>,
        >,
    ) -> Result<(), ApiError> {
        if self.is_ready()
            || state
                .extension_boot_snapshot
                .as_ref()
                .map(|boot| boot.generation_id())
                != Some(self.generation_id())
        {
            return Err(unavailable());
        }
        // Never wrap this internal router with serving_router: that would create O -> R -> O.
        let router = match assembly {
            Some(assembly) => {
                #[cfg(test)]
                if let Some(boot) = &state.extension_boot_snapshot {
                    boot.record_dispatch_assembly();
                }
                crate::console_router_with_assembly(state.clone(), true, assembly).0
            }
            None => crate::console_router_for_registry(state.clone(), true, Some(registry)),
        };
        self.publish_ready(
            crate::openapi_interface::console_router_callable_dispatch_port(router.clone()),
            Arc::new(
                crate::routes::mcp_protocol::virtual_ui::ConsoleRouterMcpInterfaceDispatchPort::new(
                    router,
                ),
            ),
            Arc::new(crate::public_gateway_assembly::PublicRuntimeInvokerAdapter),
        )
    }
    #[cfg_attr(test, api_test_selection::group(contracts))]
    #[cfg(test)]
    fn compile(state: &Arc<ApiState>) -> Result<Arc<Self>, ApiError> {
        let boot = state
            .extension_boot_snapshot
            .as_ref()
            .ok_or_else(unavailable)?;
        let registry = boot
            .interface_registry()
            .ok_or_else(unavailable)?
            .snapshot();
        let owner = Self::new(boot.generation_id());
        owner.compile_router(state, &registry, None)?;
        Ok(owner)
    }
}
