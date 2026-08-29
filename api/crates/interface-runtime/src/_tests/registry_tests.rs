use std::sync::Arc;

use crate::{
    AdmissionAdapterReference, AuthenticationAdapterReference, AuthorizationAdapterReference,
    AuthorizationOperation, BindingId, ContractIdentity, GraphFingerprint, HandlerReference,
    InterfaceAccess, InterfaceAuditPolicy, InterfaceAuthenticationPolicy, InterfaceContract,
    InterfaceContracts, InterfaceDefinition, InterfaceErrorPolicy, InterfaceExecution,
    InterfaceExecutionMode, InterfaceHandler, InterfaceHandlerContext, InterfaceHandlerFuture,
    InterfaceId, InterfaceIdentity, InterfaceLifecycle, InterfaceOwner, InterfaceScope,
    InterfaceTargetFailure, InterfaceVersion, InvocationAdapterPlan, PrincipalProfile,
    ProtocolBinding, ProtocolProjection, RegistryCompilationError, RegistryCompiler, RouteIdentity,
    TargetReference, UserPrincipal,
};

struct Input;
impl InterfaceContract for Input {
    const CONTRACT_ID: &'static str = "test-input";
    const CONTRACT_VERSION: &'static str = "1";
}

struct Output;
impl InterfaceContract for Output {
    const CONTRACT_ID: &'static str = "test-output";
    const CONTRACT_VERSION: &'static str = "1";
}

struct WrongOutput;
impl InterfaceContract for WrongOutput {
    const CONTRACT_ID: &'static str = "wrong-output";
    const CONTRACT_VERSION: &'static str = "1";
}

struct TargetError;
impl InterfaceContract for TargetError {
    const CONTRACT_ID: &'static str = "test-target-error";
    const CONTRACT_VERSION: &'static str = "1";
}

struct TestHandler;
impl InterfaceHandler<Input, Output, TargetError> for TestHandler {
    fn invoke(
        &self,
        _context: InterfaceHandlerContext,
        _input: Input,
    ) -> InterfaceHandlerFuture<Output, TargetError> {
        Box::pin(async { Ok(Output) })
    }
}

struct WrongHandler;
impl InterfaceHandler<Input, WrongOutput, TargetError> for WrongHandler {
    fn invoke(
        &self,
        _context: InterfaceHandlerContext,
        _input: Input,
    ) -> InterfaceHandlerFuture<WrongOutput, TargetError> {
        Box::pin(async { Err(InterfaceTargetFailure::new("unused", TargetError)) })
    }
}

fn operation() -> AuthorizationOperation {
    AuthorizationOperation::new("test.permission").unwrap()
}

fn owner() -> InterfaceOwner {
    InterfaceOwner::new("test.owner").unwrap()
}

fn contracts() -> InterfaceContracts {
    InterfaceContracts::unary(
        ContractIdentity::new(Input::CONTRACT_ID, Input::CONTRACT_VERSION).unwrap(),
        ContractIdentity::new(Output::CONTRACT_ID, Output::CONTRACT_VERSION).unwrap(),
        ContractIdentity::new("test-target-error", "1").unwrap(),
    )
}

fn identity(id: &str) -> InterfaceIdentity {
    InterfaceIdentity::new(
        InterfaceId::new(id).unwrap(),
        InterfaceVersion::new("1").unwrap(),
    )
}

fn definition(id: &str) -> InterfaceDefinition {
    InterfaceDefinition::new(
        identity(id),
        contracts(),
        InterfaceAccess::new(
            PrincipalProfile::User,
            InterfaceAuthenticationPolicy::Authenticated,
            operation(),
            InterfaceScope::System,
        ),
        InterfaceExecution::new(
            InterfaceExecutionMode::Unary,
            HandlerReference::new("test.handler").unwrap(),
            TargetReference::new("test.target").unwrap(),
        ),
        InterfaceAuditPolicy::ReadOnly,
        InterfaceErrorPolicy::TypedTarget,
        InterfaceLifecycle::BootSnapshot,
        owner(),
    )
}

fn binding(id: &str, interface_id: &str, path: &str) -> ProtocolBinding {
    ProtocolBinding::new(
        BindingId::new(id).unwrap(),
        identity(interface_id),
        contracts(),
        ProtocolProjection::http(RouteIdentity::new("GET", path).unwrap()),
    )
}

fn adapter_plan() -> InvocationAdapterPlan {
    InvocationAdapterPlan::new(
        AuthenticationAdapterReference::new("test.authn").unwrap(),
        AuthorizationAdapterReference::new("test.authz").unwrap(),
        Some(AdmissionAdapterReference::new("test.admission").unwrap()),
    )
}

fn compiler() -> RegistryCompiler {
    RegistryCompiler::new(
        GraphFingerprint::new("graph:test").unwrap(),
        [operation()],
        [owner()],
    )
}

fn register_complete(compiler: &mut RegistryCompiler, id: &str, binding_id: &str, path: &str) {
    compiler.register_definition(definition(id)).unwrap();
    compiler
        .register_binding(binding(binding_id, id, path), adapter_plan())
        .unwrap();
    compiler
        .bind_handler::<Input, Output, TargetError, UserPrincipal>(
            &InterfaceId::new(id).unwrap(),
            HandlerReference::new("test.handler").unwrap(),
            Arc::new(TestHandler),
        )
        .unwrap();
}

#[test]
fn compiles_definition_binding_and_plan_into_deterministic_snapshot() {
    let mut first = compiler();
    register_complete(
        &mut first,
        "test.read",
        "http.test.read.v1",
        "/api/console/test",
    );
    let first = first.compile().unwrap();

    let mut second = compiler();
    register_complete(
        &mut second,
        "test.read",
        "http.test.read.v1",
        "/api/console/test",
    );
    let second = second.compile().unwrap();

    assert_eq!(first.fingerprint(), second.fingerprint());
    assert_eq!(first.definitions().len(), 1);
    assert_eq!(first.bindings().len(), 1);
    let binding_id = BindingId::new("http.test.read.v1").unwrap();
    let plan = first.plan(&binding_id).unwrap();
    assert!(plan.binding_fingerprint().as_str().starts_with("sha256:"));
    assert!(plan.fingerprint().as_str().starts_with("sha256:"));
    assert_ne!(
        plan.binding_fingerprint().as_str(),
        plan.fingerprint().as_str()
    );
    assert_eq!(
        first
            .definition_by_route(&RouteIdentity::new("GET", "/api/console/test").unwrap())
            .unwrap()
            .interface_id()
            .as_str(),
        "test.read"
    );
}

#[test]
fn rejects_duplicate_interface_binding_and_projection_identity() {
    let mut compiler = compiler();
    compiler
        .register_definition(definition("test.read"))
        .unwrap();
    assert!(matches!(
        compiler.register_definition(definition("test.read")),
        Err(RegistryCompilationError::DuplicateInterface(_))
    ));
    compiler
        .register_binding(
            binding("http.test.read.v1", "test.read", "/api/console/test"),
            adapter_plan(),
        )
        .unwrap();
    assert!(matches!(
        compiler.register_binding(
            binding("http.test.read.v1", "test.read", "/api/console/other",),
            adapter_plan()
        ),
        Err(RegistryCompilationError::DuplicateBinding(_))
    ));
    assert!(matches!(
        compiler.register_binding(
            binding("http.test.other.v1", "test.read", "/api/console/test",),
            adapter_plan()
        ),
        Err(RegistryCompilationError::DuplicateProjection(_))
    ));
}

#[test]
fn rejects_missing_or_mismatched_typed_handler_and_missing_binding() {
    let mut missing = compiler();
    missing
        .register_definition(definition("test.missing"))
        .unwrap();
    missing
        .register_binding(
            binding(
                "http.test.missing.v1",
                "test.missing",
                "/api/console/missing",
            ),
            adapter_plan(),
        )
        .unwrap();
    assert!(matches!(
        missing.compile(),
        Err(RegistryCompilationError::MissingHandler(_))
    ));

    let mut mismatch = compiler();
    mismatch
        .register_definition(definition("test.mismatch"))
        .unwrap();
    mismatch
        .register_binding(
            binding(
                "http.test.mismatch.v1",
                "test.mismatch",
                "/api/console/mismatch",
            ),
            adapter_plan(),
        )
        .unwrap();
    mismatch
        .bind_handler::<Input, WrongOutput, TargetError, UserPrincipal>(
            &InterfaceId::new("test.mismatch").unwrap(),
            HandlerReference::new("test.handler").unwrap(),
            Arc::new(WrongHandler),
        )
        .unwrap();
    assert!(matches!(
        mismatch.compile(),
        Err(RegistryCompilationError::ContractMismatch(_))
    ));

    let mut missing_binding = compiler();
    missing_binding
        .register_definition(definition("test.unbound"))
        .unwrap();
    missing_binding
        .bind_handler::<Input, Output, TargetError, UserPrincipal>(
            &InterfaceId::new("test.unbound").unwrap(),
            HandlerReference::new("test.handler").unwrap(),
            Arc::new(TestHandler),
        )
        .unwrap();
    assert!(matches!(
        missing_binding.compile(),
        Err(RegistryCompilationError::MissingBinding(_))
    ));
}

#[test]
fn rejects_unknown_operation_inactive_owner_and_binding_contract_or_version_mismatch() {
    let mut unknown_operation = RegistryCompiler::new(
        GraphFingerprint::new("graph:test").unwrap(),
        Vec::<AuthorizationOperation>::new(),
        [owner()],
    );
    register_complete(
        &mut unknown_operation,
        "test.unknown",
        "http.test.unknown.v1",
        "/api/console/unknown",
    );
    assert!(matches!(
        unknown_operation.compile(),
        Err(RegistryCompilationError::UnknownAuthorizationOperation(_))
    ));

    let mut inactive = RegistryCompiler::new(
        GraphFingerprint::new("graph:test").unwrap(),
        [operation()],
        Vec::<InterfaceOwner>::new(),
    );
    register_complete(
        &mut inactive,
        "test.inactive",
        "http.test.inactive.v1",
        "/api/console/inactive",
    );
    assert!(matches!(
        inactive.compile(),
        Err(RegistryCompilationError::InactiveOwner(_))
    ));

    let mut version = compiler();
    version
        .register_definition(definition("test.version"))
        .unwrap();
    let mut wrong_identity = identity("test.version");
    wrong_identity = InterfaceIdentity::new(
        wrong_identity.interface_id().clone(),
        InterfaceVersion::new("2").unwrap(),
    );
    version
        .register_binding(
            ProtocolBinding::new(
                BindingId::new("http.test.version.v2").unwrap(),
                wrong_identity,
                contracts(),
                ProtocolProjection::http(
                    RouteIdentity::new("GET", "/api/console/version").unwrap(),
                ),
            ),
            adapter_plan(),
        )
        .unwrap();
    version
        .bind_handler::<Input, Output, TargetError, UserPrincipal>(
            &InterfaceId::new("test.version").unwrap(),
            HandlerReference::new("test.handler").unwrap(),
            Arc::new(TestHandler),
        )
        .unwrap();
    assert!(matches!(
        version.compile(),
        Err(RegistryCompilationError::BindingVersionMismatch(_))
    ));

    let mut contract = compiler();
    contract
        .register_definition(definition("test.contract"))
        .unwrap();
    contract
        .register_binding(
            ProtocolBinding::new(
                BindingId::new("http.test.contract.v1").unwrap(),
                identity("test.contract"),
                InterfaceContracts::unary(
                    ContractIdentity::new(Input::CONTRACT_ID, Input::CONTRACT_VERSION).unwrap(),
                    ContractIdentity::new(WrongOutput::CONTRACT_ID, WrongOutput::CONTRACT_VERSION)
                        .unwrap(),
                    ContractIdentity::new("test-target-error", "1").unwrap(),
                ),
                ProtocolProjection::http(
                    RouteIdentity::new("GET", "/api/console/contract").unwrap(),
                ),
            ),
            adapter_plan(),
        )
        .unwrap();
    contract
        .bind_handler::<Input, Output, TargetError, UserPrincipal>(
            &InterfaceId::new("test.contract").unwrap(),
            HandlerReference::new("test.handler").unwrap(),
            Arc::new(TestHandler),
        )
        .unwrap();
    assert!(matches!(
        contract.compile(),
        Err(RegistryCompilationError::BindingContractMismatch(_))
    ));
}

#[test]
fn rejects_handler_bound_with_the_wrong_principal_profile() {
    let mut compiler = compiler();
    compiler
        .register_definition(InterfaceDefinition::new(
            identity("test.public"),
            contracts(),
            InterfaceAccess::new(
                PrincipalProfile::Public,
                InterfaceAuthenticationPolicy::Anonymous,
                operation(),
                InterfaceScope::System,
            ),
            InterfaceExecution::new(
                InterfaceExecutionMode::Unary,
                HandlerReference::new("test.handler").unwrap(),
                TargetReference::new("test.target").unwrap(),
            ),
            InterfaceAuditPolicy::ReadOnly,
            InterfaceErrorPolicy::TypedTarget,
            InterfaceLifecycle::BootSnapshot,
            owner(),
        ))
        .unwrap();
    compiler
        .register_binding(
            binding("http.test.public.v1", "test.public", "/api/public/test"),
            adapter_plan(),
        )
        .unwrap();
    compiler
        .bind_handler::<Input, Output, TargetError, UserPrincipal>(
            &InterfaceId::new("test.public").unwrap(),
            HandlerReference::new("test.handler").unwrap(),
            Arc::new(TestHandler),
        )
        .unwrap();
    assert!(matches!(
        compiler.compile(),
        Err(RegistryCompilationError::PrincipalProfileMismatch(_))
    ));
}
