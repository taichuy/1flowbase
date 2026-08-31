use std::sync::Arc;

use interface_runtime::{BindingId, InterfaceContract, UserPrincipal};

use crate::routes::console_interface::{
    compile_registry, ConsoleInterfaceDeclaration, ConsoleInterfaceFuture, ConsoleInterfacePort,
};

struct Input;
impl InterfaceContract for Input {
    const CONTRACT_ID: &'static str = "fixture-console-multi-binding-input";
    const CONTRACT_VERSION: &'static str = "1";
}

struct Output;
impl InterfaceContract for Output {
    const CONTRACT_ID: &'static str = "fixture-console-multi-binding-output";
    const CONTRACT_VERSION: &'static str = "1";
}

struct Port;
impl ConsoleInterfacePort<Input, Output> for Port {
    fn execute<'a>(
        &'a self,
        _principal: &'a UserPrincipal,
        _input: Input,
    ) -> ConsoleInterfaceFuture<'a, Output> {
        Box::pin(async { unreachable!("compilation fixture does not dispatch") })
    }
}

#[test]
fn one_console_operation_can_publish_multiple_static_bindings() {
    let declarations = Box::leak(Box::new([
        ConsoleInterfaceDeclaration {
            interface_id: "fixture.operation.read",
            binding_id: "http.fixture.operation.primary.v1",
            method: "GET",
            path: "/api/console/fixture/primary",
            mutating: false,
        },
        ConsoleInterfaceDeclaration {
            interface_id: "fixture.operation.read",
            binding_id: "http.fixture.operation.alias.v1",
            method: "GET",
            path: "/api/console/fixture/alias",
            mutating: false,
        },
    ]));
    let registry = compile_registry(
        "api-server.fixture-console-multi-binding",
        "graph:fixture-console-multi-binding-v1",
        declarations,
        Arc::new(Port),
    )
    .unwrap();

    let primary = registry
        .plan(&BindingId::new("http.fixture.operation.primary.v1").unwrap())
        .unwrap();
    let alias = registry
        .plan(&BindingId::new("http.fixture.operation.alias.v1").unwrap())
        .unwrap();
    assert_eq!(
        primary.definition().interface_id(),
        alias.definition().interface_id()
    );
    assert_eq!(
        primary.definition().authorization_operation().as_str(),
        "fixture.operation.read"
    );
}
