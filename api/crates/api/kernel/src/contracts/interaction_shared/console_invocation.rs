use access_control::ConsoleOperationRegistry;
use control_plane::ports::RoleConsolePolicyReader;
use interface_runtime::{AuthorizationAdapterReference,InterfaceAuthorizationError,InterfaceAuthorizationFuture,InterfaceAuthorizationPort,InterfaceAuthorizationRequest,InterfaceInvocationKernel};
use std::sync::Arc;
struct ConsoleInterfaceAuthorizationPort {
    policy_reader: Arc<dyn RoleConsolePolicyReader>,
    console_registry: Arc<ConsoleOperationRegistry>,
}

impl InterfaceAuthorizationPort for ConsoleInterfaceAuthorizationPort {
    fn adapter_reference(&self) -> AuthorizationAdapterReference {
        AuthorizationAdapterReference::new("api-server.console.compiled-operation")
            .expect("static adapter is valid")
    }

    fn authorize(
        &self,
        request: InterfaceAuthorizationRequest,
    ) -> InterfaceAuthorizationFuture<'_> {
        let policy_reader = Arc::clone(&self.policy_reader);
        let console_registry = Arc::clone(&self.console_registry);
        Box::pin(async move {
            // Console bindings authorize against their exact compiled HTTP operation.
            let (method, path) = request
                .binding()
                .projection()
                .http_route()
                .map(|route| (route.method(), route.path()))
                .ok_or_else(|| {
                    InterfaceAuthorizationError::classified("console_route_projection_missing")
                })?;
            let access = crate::middleware::require_settings_feature_permission::compiled_console_route_access(
                &console_registry,
                method,
                path,
            )
            .map_err(InterfaceAuthorizationError::classified)?;
            if request.principal().actor().is_root {
                return Ok(());
            }
            let policies = policy_reader
                .load_role_console_policies_for_user(request.principal().actor())
                .await
                .map_err(|error| {
                    InterfaceAuthorizationError::with_source(
                        "console_authorization_unavailable",
                        crate::error_response::ApiError::from(error),
                    )
                })?;
            if crate::middleware::require_settings_feature_permission::authorize_compiled_console_access(
                &access,
                request.principal().actor(),
                &policies,
            ) {
                Ok(())
            } else {
                Err(InterfaceAuthorizationError::with_source(
                    "console_operation_permission_denied",
                    crate::error_response::ApiError::from(
                        control_plane::errors::ControlPlaneError::PermissionDenied(
                            "console_operation_permission_denied",
                        ),
                    ),
                ))
            }
        })
    }
}

pub fn invocation_kernel(
    policy_reader: Arc<dyn RoleConsolePolicyReader>,
    console_registry: Arc<ConsoleOperationRegistry>,
) -> Arc<InterfaceInvocationKernel> {
    Arc::new(InterfaceInvocationKernel::new(Arc::new(
        ConsoleInterfaceAuthorizationPort {
            policy_reader,
            console_registry,
        },
    )))
}

pub fn invocation_kernel_with_admission(
    policy_reader: Arc<dyn RoleConsolePolicyReader>,
    console_registry: Arc<ConsoleOperationRegistry>,
    admission: Arc<dyn interface_runtime::InterfaceTargetAdmissionPort>,
) -> Arc<InterfaceInvocationKernel> {
    Arc::new(InterfaceInvocationKernel::with_target_admission(
        Arc::new(ConsoleInterfaceAuthorizationPort {
            policy_reader,
            console_registry,
        }),
        admission,
    ))
}
