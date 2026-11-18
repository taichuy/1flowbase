use super::*;
#[cfg(any(test, feature = "test-support"))]
struct UnavailableRuntimeModelOperationPort;

#[cfg(any(test, feature = "test-support"))]
impl RuntimeModelOperationPort for UnavailableRuntimeModelOperationPort {
    fn metadata<'a>(
        &'a self,
        _principal: &'a UserPrincipal,
        _model_code: String,
    ) -> RuntimeModelMetadataFuture<'a> {
        Box::pin(async {
            Err(RuntimeModelOperationTargetError(ApiError::from(
                control_plane::errors::ControlPlaneError::Conflict(
                    "runtime_model_test_port_unavailable",
                ),
            )))
        })
    }

    fn execute<'a>(
        &'a self,
        _principal: &'a UserPrincipal,
        _input: RuntimeModelOperationInput,
    ) -> RuntimeModelOperationFuture<'a> {
        Box::pin(async {
            Err(RuntimeModelOperationTargetError(ApiError::from(
                control_plane::errors::ControlPlaneError::Conflict(
                    "runtime_model_test_port_unavailable",
                ),
            )))
        })
    }
}

#[cfg(any(test, feature = "test-support"))]
pub(crate) fn compile_registry_for_test(
) -> Result<Arc<CompiledInterfaceRegistry>, interface_runtime::RegistryCompilationError> {
    compile_registry(Arc::new(UnavailableRuntimeModelOperationPort))
}
