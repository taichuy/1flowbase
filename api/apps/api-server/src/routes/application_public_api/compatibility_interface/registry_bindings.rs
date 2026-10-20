use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn register_stream_binding(
    compiler: &mut RegistryCompiler,
    owner: &InterfaceOwner,
    operation: &AuthorizationOperation,
    port: Arc<dyn CompatibilityBlockingPort>,
    interface_id: &str,
    binding_id: &str,
    handler: &str,
    method: &str,
    path: &str,
) -> Result<(), interface_runtime::RegistryCompilationError> {
    let interface_id = InterfaceId::new(interface_id).expect("static interface id is valid");
    let contracts = InterfaceContracts::server_stream(
        contract::<CompatibilityBlockingInput>(),
        contract::<CompatibilityStreamEvent>(),
        contract::<CompatibilityBlockingOutput>(),
        contract::<CompatibilityBlockingTargetError>(),
    );
    let identity = InterfaceIdentity::new(
        interface_id.clone(),
        InterfaceVersion::new("1").expect("static interface version is valid"),
    );
    let handler = HandlerReference::new(handler).expect("static handler is valid");
    compiler.register_definition(InterfaceDefinition::new(
        identity.clone(),
        contracts.clone(),
        InterfaceAccess::new(
            interface_runtime::PrincipalProfile::Application,
            InterfaceAuthenticationPolicy::Authenticated,
            operation.clone(),
            InterfaceScope::Workspace,
        ),
        InterfaceExecution::new(
            InterfaceExecutionMode::ServerStream,
            handler.clone(),
            TargetReference::new(TARGET).expect("static target is valid"),
        ),
        InterfaceAuditPolicy::Mutating,
        InterfaceErrorPolicy::TypedTarget,
        InterfaceLifecycle::BootSnapshot,
        owner.clone(),
    ))?;
    register_authentication(compiler, &interface_id)?;
    compiler.register_binding(
        ProtocolBinding::new(
            BindingId::new(binding_id).expect("static binding is valid"),
            identity,
            contracts,
            ProtocolProjection::http_variant(
                RouteIdentity::new(method, path).expect("static route is valid"),
                "streaming",
            ),
        ),
        InvocationAdapterPlan::new(
            AuthenticationAdapterReference::new(AUTHENTICATION_ADAPTER)
                .expect("static adapter is valid"),
            AuthorizationAdapterReference::new(AUTHORIZATION_ADAPTER)
                .expect("static adapter is valid"),
            None,
        ),
    )?;
    compiler.bind_stream_handler::<CompatibilityBlockingInput, CompatibilityStreamEvent, CompatibilityBlockingOutput, CompatibilityBlockingTargetError, ApplicationPrincipal>(
        &interface_id,
        handler,
        Arc::new(CompatibilityStreamHandler { port }),
    )?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(super) fn register_binding(
    compiler: &mut RegistryCompiler,
    owner: &InterfaceOwner,
    operation: &AuthorizationOperation,
    port: Arc<dyn CompatibilityBlockingPort>,
    interface_id: &str,
    binding_id: &str,
    handler: &str,
    method: &str,
    path: &str,
) -> Result<(), interface_runtime::RegistryCompilationError> {
    let interface_id = InterfaceId::new(interface_id).expect("static interface id is valid");
    let contracts = InterfaceContracts::unary(
        contract::<CompatibilityBlockingInput>(),
        contract::<CompatibilityBlockingOutput>(),
        contract::<CompatibilityBlockingTargetError>(),
    );
    let identity = InterfaceIdentity::new(
        interface_id.clone(),
        InterfaceVersion::new("1").expect("static interface version is valid"),
    );
    let handler = HandlerReference::new(handler).expect("static handler is valid");
    compiler.register_definition(InterfaceDefinition::new(
        identity.clone(),
        contracts.clone(),
        InterfaceAccess::new(
            interface_runtime::PrincipalProfile::Application,
            InterfaceAuthenticationPolicy::Authenticated,
            operation.clone(),
            InterfaceScope::Workspace,
        ),
        InterfaceExecution::new(
            InterfaceExecutionMode::Unary,
            handler.clone(),
            TargetReference::new(TARGET).expect("static target is valid"),
        ),
        InterfaceAuditPolicy::Mutating,
        InterfaceErrorPolicy::TypedTarget,
        InterfaceLifecycle::BootSnapshot,
        owner.clone(),
    ))?;
    register_authentication(compiler, &interface_id)?;
    compiler.register_binding(
        ProtocolBinding::new(
            BindingId::new(binding_id).expect("static binding is valid"),
            identity,
            contracts,
            ProtocolProjection::http(
                RouteIdentity::new(method, path).expect("static route is valid"),
            ),
        ),
        InvocationAdapterPlan::new(
            AuthenticationAdapterReference::new(AUTHENTICATION_ADAPTER)
                .expect("static adapter is valid"),
            AuthorizationAdapterReference::new(AUTHORIZATION_ADAPTER)
                .expect("static adapter is valid"),
            None,
        ),
    )?;
    compiler.bind_handler::<CompatibilityBlockingInput, CompatibilityBlockingOutput, CompatibilityBlockingTargetError, ApplicationPrincipal>(
        &interface_id,
        handler,
        Arc::new(CompatibilityBlockingHandler { port }),
    )?;
    Ok(())
}

pub(super) fn register_authentication(
    compiler: &mut RegistryCompiler,
    interface_id: &InterfaceId,
) -> Result<(), interface_runtime::RegistryCompilationError> {
    compiler.register_authentication_adapter(
        interface_id,
        1,
        interface_runtime::InterfaceExtensionRegistration::new(
            interface_runtime::PluginIdentity::new("api-server.application-authentication")
                .expect("static plugin is valid"),
            interface_runtime::InterfaceExtensionTier::BuiltIn,
            interface_runtime::InterfaceExtensionPoint::AuthenticationAdapter,
            interface_runtime::InterfaceExtensionPermission::Authenticate,
            InterfaceScope::Workspace,
            interface_runtime::InterfaceExtensionIsolation::TrustedInProcess,
            [],
        )
        .expect("built-in authentication registration is valid"),
        interface_runtime::ActivatedAuthenticationAdapter::new(
            interface_runtime::PluginIdentity::new("api-server.application-authentication")
                .expect("static plugin is valid"),
            interface_runtime::InterfaceExtensionTier::BuiltIn,
            AuthenticationAdapterReference::new(AUTHENTICATION_ADAPTER)
                .expect("static adapter is valid"),
            interface_runtime::AuthenticationActivationIdentity::new(
                "api-server.application-api-key.activation.v1",
            )
            .expect("static activation is valid"),
            interface_runtime::PrincipalProfile::Application,
        ),
    )
}
