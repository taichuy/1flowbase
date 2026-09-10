use super::*;

// #2021 AC-003: each nested invocation must leave stack headroom for the actual
// handler and protocol adapters. This guards inline state, not total poll stack;
// the real HTTP/MCP replay separately exercises the full poll chain.
#[tokio::test]
async fn invocation_inline_state_has_bounded_stack_cost() {
    let snapshot = compile_snapshot("graph:stack-budget", 1, false, Arc::new(Mutex::new(None)));
    let kernel = InterfaceInvocationKernel::with_target_admission(
        Arc::new(Authorization { reject: false }),
        Arc::new(Admission { reject: false }),
    );
    let invocation = kernel.invoke::<Input, Output, TargetError>(snapshot, envelope(actor(true)));
    let bytes = std::mem::size_of_val(&invocation);
    assert!(bytes <= 16 * 1024, "invocation inline state uses {bytes} bytes");
    let result = invocation.await.unwrap();
    assert_eq!(result.receipt().terminal(), InterfaceInvocationTerminal::Completed);
}
