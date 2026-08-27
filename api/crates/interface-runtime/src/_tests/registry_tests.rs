use std::sync::Arc;

use crate::{
    ContractIdentity, GraphFingerprint, HandlerReference, InterfaceContract, InterfaceDefinition,
    InterfaceHandler, InterfaceHandlerContext, InterfaceHandlerFuture, InterfaceId, InterfaceOwner,
    InterfaceTargetError, PermissionIdentity, RegistryCompilationError, RegistryCompiler,
    RouteIdentity, TargetReference,
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

struct TestHandler;
impl InterfaceHandler<Input, Output> for TestHandler {
    fn invoke(
        &self,
        _context: InterfaceHandlerContext,
        _input: Input,
    ) -> InterfaceHandlerFuture<Output> {
        Box::pin(async { Ok(Output) })
    }
}

struct WrongHandler;
impl InterfaceHandler<Input, WrongOutput> for WrongHandler {
    fn invoke(
        &self,
        _context: InterfaceHandlerContext,
        _input: Input,
    ) -> InterfaceHandlerFuture<WrongOutput> {
        Box::pin(async { Err(InterfaceTargetError::classified("unused")) })
    }
}

fn permission() -> PermissionIdentity {
    PermissionIdentity::new("test.permission").unwrap()
}

fn definition(id: &str, path: &str) -> InterfaceDefinition {
    InterfaceDefinition::new(
        InterfaceId::new(id).unwrap(),
        ContractIdentity::new(Input::CONTRACT_ID, Input::CONTRACT_VERSION).unwrap(),
        ContractIdentity::new(Output::CONTRACT_ID, Output::CONTRACT_VERSION).unwrap(),
        Some(RouteIdentity::new("GET", path).unwrap()),
        permission(),
        HandlerReference::new("test.handler").unwrap(),
        TargetReference::new("test.target").unwrap(),
        InterfaceOwner::new("test.owner").unwrap(),
    )
}

fn compiler() -> RegistryCompiler {
    RegistryCompiler::new(GraphFingerprint::new("graph:test").unwrap(), [permission()])
}

#[test]
fn compiles_typed_definition_and_handler_into_deterministic_snapshot() {
    let mut first = compiler();
    first
        .register_definition(definition("test.read", "/api/console/test"))
        .unwrap();
    first
        .bind_handler::<Input, Output>(
            &InterfaceId::new("test.read").unwrap(),
            HandlerReference::new("test.handler").unwrap(),
            Arc::new(TestHandler),
        )
        .unwrap();
    let first = first.compile().unwrap();

    let mut second = compiler();
    second
        .register_definition(definition("test.read", "/api/console/test"))
        .unwrap();
    second
        .bind_handler::<Input, Output>(
            &InterfaceId::new("test.read").unwrap(),
            HandlerReference::new("test.handler").unwrap(),
            Arc::new(TestHandler),
        )
        .unwrap();
    let second = second.compile().unwrap();

    assert_eq!(first.fingerprint(), second.fingerprint());
    assert_eq!(first.definitions().len(), 1);
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
fn rejects_duplicate_identity_and_route() {
    let mut duplicate_identity = compiler();
    duplicate_identity
        .register_definition(definition("test.read", "/api/console/one"))
        .unwrap();
    assert!(matches!(
        duplicate_identity.register_definition(definition("test.read", "/api/console/two")),
        Err(RegistryCompilationError::DuplicateInterface(_))
    ));

    let mut duplicate_route = compiler();
    duplicate_route
        .register_definition(definition("test.one", "/api/console/shared"))
        .unwrap();
    assert!(matches!(
        duplicate_route.register_definition(definition("test.two", "/api/console/shared")),
        Err(RegistryCompilationError::DuplicateRoute { .. })
    ));
}

#[test]
fn rejects_missing_handler_contract_mismatch_and_unknown_permission() {
    let mut missing = compiler();
    missing
        .register_definition(definition("test.missing", "/api/console/missing"))
        .unwrap();
    assert!(matches!(
        missing.compile(),
        Err(RegistryCompilationError::MissingHandler(_))
    ));

    let mut mismatch = compiler();
    mismatch
        .register_definition(definition("test.mismatch", "/api/console/mismatch"))
        .unwrap();
    mismatch
        .bind_handler::<Input, WrongOutput>(
            &InterfaceId::new("test.mismatch").unwrap(),
            HandlerReference::new("test.handler").unwrap(),
            Arc::new(WrongHandler),
        )
        .unwrap();
    assert!(matches!(
        mismatch.compile(),
        Err(RegistryCompilationError::ContractMismatch(_))
    ));

    let mut unknown_permission = RegistryCompiler::new(
        GraphFingerprint::new("graph:test").unwrap(),
        Vec::<PermissionIdentity>::new(),
    );
    unknown_permission
        .register_definition(definition("test.unknown", "/api/console/unknown"))
        .unwrap();
    unknown_permission
        .bind_handler::<Input, Output>(
            &InterfaceId::new("test.unknown").unwrap(),
            HandlerReference::new("test.handler").unwrap(),
            Arc::new(TestHandler),
        )
        .unwrap();
    assert!(matches!(
        unknown_permission.compile(),
        Err(RegistryCompilationError::UnknownPermission(_))
    ));
}
