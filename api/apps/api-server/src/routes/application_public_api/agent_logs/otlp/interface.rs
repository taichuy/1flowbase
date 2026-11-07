use super::*;

pub(super) struct Authorization;
impl InterfaceAuthorizationPort<ApplicationPrincipal> for Authorization {
    fn adapter_reference(&self) -> AuthorizationAdapterReference {
        AuthorizationAdapterReference::new(OWNER)
            .expect("static OTLP trace interface identity is valid")
    }
    fn authorize(
        &self,
        _request: InterfaceAuthorizationRequest<ApplicationPrincipal>,
    ) -> InterfaceAuthorizationFuture<'_> {
        Box::pin(async { Ok(()) })
    }
}
pub(crate) fn compile_registry<R>(
    store: R,
) -> Result<Arc<CompiledInterfaceRegistry>, RegistryCompilationError>
where
    R: BillingRepository + OrchestrationRuntimeRepository + Clone + Send + Sync + 'static,
{
    let owner = InterfaceOwner::new(OWNER).expect("static OTLP trace interface identity is valid");
    let operation =
        AuthorizationOperation::new(ID).expect("static OTLP trace interface identity is valid");
    let id = InterfaceId::new(ID).expect("static OTLP trace interface identity is valid");
    let identity = InterfaceIdentity::new(
        id.clone(),
        InterfaceVersion::new("1").expect("static OTLP trace interface identity is valid"),
    );
    let handler =
        HandlerReference::new(OWNER).expect("static OTLP trace interface identity is valid");
    let contracts = InterfaceContracts::unary(
        ContractIdentity::new(Input::CONTRACT_ID, "1")
            .expect("static OTLP trace interface identity is valid"),
        ContractIdentity::new(Output::CONTRACT_ID, "1")
            .expect("static OTLP trace interface identity is valid"),
        ContractIdentity::new(TargetError::CONTRACT_ID, "1")
            .expect("static OTLP trace interface identity is valid"),
    );
    let mut compiler = RegistryCompiler::new(
        GraphFingerprint::new("graph:application-agent-logs-otlp-v1")
            .expect("static OTLP trace interface identity is valid"),
        [operation.clone()],
        [owner.clone()],
    );
    compiler.register_definition(
        InterfaceDefinition::new(
            identity.clone(),
            contracts.clone(),
            InterfaceAccess::new(
                PrincipalProfile::Application,
                InterfaceAuthenticationPolicy::Authenticated,
                operation,
                InterfaceScope::Workspace,
            ),
            InterfaceExecution::new(
                InterfaceExecutionMode::Unary,
                handler.clone(),
                TargetReference::new(OWNER).expect("static OTLP trace interface identity is valid"),
            ),
            InterfaceAuditPolicy::Mutating,
            InterfaceErrorPolicy::TypedTarget,
            InterfaceLifecycle::BootSnapshot,
            owner,
        )
        .with_event_observation(),
    )?;
    compiler.register_authentication_adapter(
        &id,
        1,
        InterfaceExtensionRegistration::new(
            PluginIdentity::new("api-server.application-authentication")
                .expect("static OTLP trace interface identity is valid"),
            InterfaceExtensionTier::BuiltIn,
            InterfaceExtensionPoint::AuthenticationAdapter,
            InterfaceExtensionPermission::Authenticate,
            InterfaceScope::Workspace,
            InterfaceExtensionIsolation::TrustedInProcess,
            [],
        )
        .expect("static OTLP trace interface identity is valid"),
        ActivatedAuthenticationAdapter::new(
            PluginIdentity::new("api-server.application-authentication")
                .expect("static OTLP trace interface identity is valid"),
            InterfaceExtensionTier::BuiltIn,
            AuthenticationAdapterReference::new("api-server.application-api-key")
                .expect("static OTLP trace interface identity is valid"),
            AuthenticationActivationIdentity::new("api-server.application-api-key.activation.v1")
                .expect("static OTLP trace interface identity is valid"),
            PrincipalProfile::Application,
        ),
    )?;
    compiler.register_binding(
        ProtocolBinding::new(
            BindingId::new(BINDING).expect("static OTLP trace interface identity is valid"),
            identity,
            contracts,
            ProtocolProjection::http(
                RouteIdentity::new("POST", ROUTE)
                    .expect("static OTLP trace interface identity is valid"),
            ),
        ),
        InvocationAdapterPlan::new(
            AuthenticationAdapterReference::new("api-server.application-api-key")
                .expect("static OTLP trace interface identity is valid"),
            AuthorizationAdapterReference::new(OWNER)
                .expect("static OTLP trace interface identity is valid"),
            None,
        ),
    )?;
    compiler.bind_handler::<Input, Output, TargetError, ApplicationPrincipal>(
        &id,
        handler,
        Arc::new(Handler(store)),
    )?;
    compiler.compile()
}
