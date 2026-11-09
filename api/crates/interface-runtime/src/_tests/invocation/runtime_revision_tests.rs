//! Full-kernel drop guards reuse this owner's registry, ports and request fixture.
use super::*;
use std::{
    cell::Cell,
    marker::PhantomPinned,
    task::{Context, Poll},
};
type Events = Arc<Mutex<Vec<&'static str>>>;
fn count(events: &Events, name: &str) -> usize {
    events
        .lock()
        .unwrap()
        .iter()
        .filter(|x| **x == name)
        .count()
}
fn before(events: &Events, first: &str, second: &str) {
    let events = events.lock().unwrap();
    assert!(
        events.iter().position(|x| *x == first).unwrap()
            < events.iter().position(|x| *x == second).unwrap(),
        "{events:?}"
    );
}
// Original Output(u8) has no Drop; this guard-only value exposes result ownership.
struct OwnedOutput {
    value: u8,
    events: Events,
    _pin: PhantomPinned,
}
impl InterfaceContract for OwnedOutput {
    const CONTRACT_ID: &'static str = Output::CONTRACT_ID;
    const CONTRACT_VERSION: &'static str = Output::CONTRACT_VERSION;
}
impl Drop for OwnedOutput {
    fn drop(&mut self) {
        self.events.lock().unwrap().push("output-drop");
    }
}
struct Native {
    events: Events,
    pending: bool,
    value: Mutex<Option<OwnedOutput>>,
    _not_sync: Cell<()>,
    _pin: PhantomPinned,
}
impl std::future::Future for Native {
    type Output = Result<OwnedOutput, InterfaceTargetFailure<TargetError>>;
    fn poll(self: std::pin::Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Self::Output> {
        if self.pending {
            Poll::Pending
        } else {
            Poll::Ready(Ok(self.value.lock().unwrap().take().unwrap()))
        }
    }
}
impl Drop for Native {
    fn drop(&mut self) {
        self.events.lock().unwrap().push("target-drop");
    }
}
struct Handler {
    events: Events,
    pending: bool,
}
impl InterfaceHandler<Input, OwnedOutput, TargetError> for Handler {
    fn invoke(
        &self,
        _: InterfaceHandlerContext,
        input: Input,
    ) -> InterfaceHandlerFuture<OwnedOutput, TargetError> {
        self.events.lock().unwrap().push("target-construct");
        Box::pin(Native {
            events: self.events.clone(),
            pending: self.pending,
            value: Mutex::new(Some(OwnedOutput {
                value: input.0 + 3,
                events: self.events.clone(),
                _pin: PhantomPinned,
            })),
            _not_sync: Cell::new(()),
            _pin: PhantomPinned,
        })
    }
}
struct Completion {
    events: Events,
    pending: bool,
}
struct End {
    events: Events,
    pending: bool,
}
impl std::future::Future for End {
    type Output = ();
    fn poll(self: std::pin::Pin<&mut Self>, _: &mut Context<'_>) -> Poll<()> {
        self.events.lock().unwrap().push("completion-poll");
        if self.pending {
            Poll::Pending
        } else {
            Poll::Ready(())
        }
    }
}
impl Drop for End {
    fn drop(&mut self) {
        self.events.lock().unwrap().push("completion-future-drop");
    }
}
impl InterfaceCompletionHook for Completion {
    fn completed(
        &self,
        _: InterfaceHookContext,
        _: InterfaceInvocationTerminal,
    ) -> InterfaceCompletionHookFuture<'_> {
        assert_eq!(
            count(&self.events, "target-drop"),
            1,
            "actual target future must drop before Completion"
        );
        self.events.lock().unwrap().push("completion-construct");
        Box::pin(End {
            events: self.events.clone(),
            pending: self.pending,
        })
    }
}
struct Failure(Events);
impl crate::InterfaceFailureHook for Failure {
    fn failed<'a>(
        &'a self,
        _: InterfaceHookContext,
        _: &'a str,
    ) -> crate::InterfaceFailureHookFuture<'a> {
        self.0.lock().unwrap().push("failure");
        Box::pin(async {})
    }
}
fn snapshot(
    events: &Events,
    pending: bool,
    completion_pending: bool,
) -> Arc<crate::CompiledInterfaceRegistry> {
    let mut compiler = compiler("graph:runtime-revision");
    compiler.register_definition(definition()).unwrap();
    activate_authentication(&mut compiler);
    compiler
        .register_binding(binding(), adapter_plan())
        .unwrap();
    compiler
        .bind_handler::<Input, OwnedOutput, TargetError, UserPrincipal>(
            &interface_id(),
            HandlerReference::new("invocation.handler").unwrap(),
            Arc::new(Handler {
                events: events.clone(),
                pending,
            }),
        )
        .unwrap();
    let plugin = PluginIdentity::new("guard.completion").unwrap();
    compiler
        .register_extension(
            &interface_id(),
            2,
            hook_registration(plugin.as_str(), InterfaceExtensionPoint::Completion),
        )
        .unwrap();
    let failure = PluginIdentity::new("guard.failure").unwrap();
    compiler
        .register_extension(
            &interface_id(),
            3,
            InterfaceExtensionRegistration::new(
                failure.clone(),
                InterfaceExtensionTier::HostExtension,
                InterfaceExtensionPoint::Failure,
                InterfaceExtensionPermission::ObserveFailure,
                InterfaceScope::System,
                InterfaceExtensionIsolation::TrustedInProcess,
                [InterfaceExtensionFact::FailureClassification],
            )
            .unwrap(),
        )
        .unwrap();
    compiler
        .bind_hook_plan(
            &interface_id(),
            Arc::new(
                TypedInterfaceHookPlan::<Input, OwnedOutput>::new(
                    GraphFingerprint::new("graph:runtime-revision").unwrap(),
                )
                .bind_failure(failure, Arc::new(Failure(events.clone())))
                .bind_completion(
                    plugin,
                    Arc::new(Completion {
                        events: events.clone(),
                        pending: completion_pending,
                    }),
                ),
            ),
        )
        .unwrap();
    compiler.compile().unwrap()
}
fn kernel() -> InterfaceInvocationKernel {
    InterfaceInvocationKernel::with_target_admission(
        Arc::new(Authorization { reject: false }),
        Arc::new(Admission { reject: false }),
    )
}
fn request(cancellation: InvocationCancellation) -> InvocationEnvelope<Input> {
    let base = envelope(actor(true));
    InvocationEnvelope::with_principal_and_controls(
        base.lineage().clone(),
        base.binding_id().clone(),
        base.protocol(),
        base.authentication_adapter().clone(),
        base.authentication_activation().clone(),
        base.principal().clone(),
        InvocationControls::new(None, cancellation, None),
        Input(2),
    )
}
async fn pending<F: std::future::Future>(mut future: std::pin::Pin<&mut F>) {
    std::future::poll_fn(|cx| {
        assert!(future.as_mut().poll(cx).is_pending());
        Poll::Ready(())
    })
    .await;
}
#[tokio::test]
async fn runtime_revision_kernel_ready_cancel_and_completion_ownership() {
    // Ready completion transfers O once to its caller before O is destroyed.
    let events = Arc::new(Mutex::new(Vec::new()));
    let outcome = kernel()
        .invoke::<Input, OwnedOutput, TargetError>(
            snapshot(&events, false, false),
            request(InvocationCancellation::new()),
        )
        .await
        .unwrap();
    assert_eq!(outcome.value().value, 5);
    assert_eq!(count(&events, "output-drop"), 0);
    before(&events, "target-drop", "completion-construct");
    drop(outcome);
    assert_eq!(count(&events, "output-drop"), 1);
    for target_pending in [false, true] {
        let events = Arc::new(Mutex::new(Vec::new()));
        let kernel = kernel();
        let cancellation = InvocationCancellation::new();
        {
            let mut future = std::pin::pin!(kernel.invoke::<Input, OwnedOutput, TargetError>(
                snapshot(&events, target_pending, true),
                request(cancellation.clone())
            ));
            fn assert_send<T: Send>(_: &T) {}
            assert_send(&future);
            pending(future.as_mut()).await;
            if target_pending {
                cancellation.cancel();
                pending(future.as_mut()).await;
            }
            assert_eq!(count(&events, "target-drop"), 1);
            assert_eq!(count(&events, "output-drop"), usize::from(target_pending));
            assert!(count(&events, "completion-poll") >= 1);
            assert_eq!(count(&events, "failure"), 0);
        }
        before(&events, "target-drop", "completion-construct");
        if !target_pending {
            before(&events, "completion-future-drop", "output-drop");
        } else {
            before(&events, "output-drop", "completion-construct");
        }
        assert_eq!(count(&events, "target-drop"), 1);
        assert_eq!(count(&events, "output-drop"), 1);
    }
}
#[tokio::test]
async fn runtime_revision_kernel_outer_drop_pending_target_once() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let kernel = kernel();
    {
        let mut future = std::pin::pin!(kernel.invoke::<Input, OwnedOutput, TargetError>(
            snapshot(&events, true, true),
            request(InvocationCancellation::new())
        ));
        pending(future.as_mut()).await;
    }
    assert_eq!(count(&events, "target-drop"), 1);
    assert_eq!(count(&events, "output-drop"), 1);
    assert_eq!(count(&events, "completion-construct"), 0);
}
