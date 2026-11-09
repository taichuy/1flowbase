//! Direct private ownership/type guards; original lifecycle cases remain in their owners.
use super::super::{
    invoke_typed, DriverPhase, MapOutput, PreparedUnary, UnaryDriver, UnaryRequest, UnaryTargetPoll,
};
use super::*;
use crate::*;
use std::{cell::Cell, marker::PhantomPinned, sync::Mutex, task::Waker};

#[derive(Debug, Default)]
struct Trace {
    events: Mutex<Vec<&'static str>>,
    address: Mutex<Option<usize>>,
}
impl Trace {
    fn note(&self, event: &'static str) {
        self.events.lock().unwrap().push(event);
    }
    fn count(&self, event: &str) -> usize {
        self.events
            .lock()
            .unwrap()
            .iter()
            .filter(|x| **x == event)
            .count()
    }
    fn before(&self, first: &str, second: &str) {
        let events = self.events.lock().unwrap();
        assert!(
            events.iter().position(|x| *x == first).unwrap()
                < events.iter().position(|x| *x == second).unwrap(),
            "{events:?}"
        );
    }
}
#[derive(Debug)]
struct Token<const N: usize> {
    _bytes: [u8; N],
    trace: Arc<Trace>,
    kind: &'static str,
    _pin: PhantomPinned,
}
impl<const N: usize> Token<N> {
    fn new(trace: &Arc<Trace>, kind: &'static str) -> Self {
        Self {
            _bytes: [0; N],
            trace: trace.clone(),
            kind,
            _pin: PhantomPinned,
        }
    }
}
impl<const N: usize> InterfaceContract for Token<N> {
    const CONTRACT_ID: &'static str = "runtime-revision.token";
    const CONTRACT_VERSION: &'static str = "1";
}
impl<const N: usize> Drop for Token<N> {
    fn drop(&mut self) {
        self.trace.note(self.kind);
    }
}
#[derive(Clone, Copy)]
enum Mode {
    Success,
    Failure,
    Pending,
}
// Cell makes the actual native future !Sync; PhantomPinned makes it !Unpin.
struct Native {
    trace: Arc<Trace>,
    mode: Mode,
    first_pending: bool,
    polls: Cell<usize>,
    value: Mutex<Option<Result<Token<0>, InterfaceTargetFailure<Token<0>>>>>,
    _pin: PhantomPinned,
}
impl Native {
    fn new(trace: &Arc<Trace>, mode: Mode, first_pending: bool) -> Self {
        let value = if matches!(mode, Mode::Failure) {
            Err(InterfaceTargetFailure::new(
                "guard-error",
                Token::new(trace, "error-drop"),
            ))
        } else {
            Ok(Token::new(trace, "output-drop"))
        };
        Self {
            trace: trace.clone(),
            mode,
            first_pending,
            polls: Cell::new(0),
            value: Mutex::new(Some(value)),
            _pin: PhantomPinned,
        }
    }
}
impl Future for Native {
    type Output = Result<Token<0>, InterfaceTargetFailure<Token<0>>>;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.as_ref().get_ref();
        let address = this as *const Self as usize;
        {
            let mut saved = this.trace.address.lock().unwrap();
            assert_eq!(
                *saved.get_or_insert(address),
                address,
                "pinned native target moved"
            );
        }
        this.trace.note("target-poll");
        let polls = this.polls.get();
        this.polls.set(polls + 1);
        if matches!(this.mode, Mode::Pending) {
            return Poll::Pending;
        }
        if this.first_pending && polls == 0 {
            cx.waker().wake_by_ref();
            return Poll::Pending;
        }
        Poll::Ready(this.value.lock().unwrap().take().unwrap())
    }
}
impl Drop for Native {
    fn drop(&mut self) {
        self.trace.note("target-drop");
    }
}
struct Authorization(Arc<Trace>);
impl InterfaceAuthorizationPort<PublicPrincipal> for Authorization {
    fn adapter_reference(&self) -> AuthorizationAdapterReference {
        AuthorizationAdapterReference::new("guard.authz").unwrap()
    }
    fn authorize(
        &self,
        _: InterfaceAuthorizationRequest<PublicPrincipal>,
    ) -> InterfaceAuthorizationFuture<'_> {
        self.0.note("authorize");
        Box::pin(async { Ok(()) })
    }
}
struct Failure(Arc<Trace>);
impl InterfaceFailureHook for Failure {
    fn failed<'a>(
        &'a self,
        _: InterfaceHookContext,
        classification: &'a str,
    ) -> InterfaceFailureHookFuture<'a> {
        assert_eq!(classification, "guard-error");
        assert_eq!(self.0.count("error-drop"), 0);
        assert_eq!(self.0.count("target-drop"), 1);
        Box::pin(async move {
            assert_eq!(self.0.count("error-drop"), 0);
            self.0.note("failure");
        })
    }
}

fn active<'a>(
    authorization: &'a Authorization,
    trace: &Arc<Trace>,
    mode: Mode,
) -> TypedUnaryBridge<'a, Token<0>, Token<0>, Token<0>, PublicPrincipal> {
    let mut bridge = TypedUnaryBridge::new(
        authorization,
        PublicPrincipal::new(),
        Token::new(trace, "input-drop"),
    );
    bridge.phase = Phase::Invoking {
        target: Some(Box::pin(Native::new(trace, mode, true))),
        result: None,
    };
    bridge
}
#[test]
fn runtime_revision_common_layout_send_unpin_and_lazy_start() {
    fn send_unpin<T: Send + Unpin>() {}
    send_unpin::<MapOutput<'static, Token<65536>>>();
    send_unpin::<UnaryDriver<'static>>();
    send_unpin::<UnaryTargetPoll<'static>>();
    assert_eq!(
        std::mem::size_of::<MapOutput<'static, Token<0>>>(),
        std::mem::size_of::<MapOutput<'static, Token<65536>>>()
    );
    assert!(std::mem::size_of::<MapOutput<'static, Token<65536>>>() <= 16 * 1024);
    assert_eq!(
        std::mem::size_of::<TypedUnaryBridge<'static, Token<0>, Token<0>, Token<0>, PublicPrincipal>>(
        ),
        std::mem::size_of::<
            TypedUnaryBridge<'static, Token<0>, Token<65536>, Token<131072>, PublicPrincipal>,
        >()
    );
    println!(
        "runtime_revision_layout R={} H={} driver={} mapper={} target_poll={}",
        std::mem::size_of::<UnaryRequest<'static>>(),
        std::mem::size_of::<PreparedUnary<'static>>(),
        std::mem::size_of::<UnaryDriver<'static>>(),
        std::mem::size_of::<MapOutput<'static, Token<0>>>(),
        std::mem::size_of::<UnaryTargetPoll<'static>>()
    );
    let trace = Arc::new(Trace::default());
    let kernel =
        InterfaceInvocationKernel::<PublicPrincipal>::new(Arc::new(Authorization(trace.clone())));
    let snapshot =
        RegistryCompiler::new(GraphFingerprint::new("graph:guard-empty").unwrap(), [], [])
            .compile()
            .unwrap();
    let request = InvocationEnvelope::with_principal(
        InvocationLineage::root(InvocationId::now_v7()),
        BindingId::new("guard.unresolved").unwrap(),
        InterfaceProtocol::Http,
        AuthenticationAdapterReference::new("guard.authn").unwrap(),
        AuthenticationActivationIdentity::new("guard.activation").unwrap(),
        PublicPrincipal::new(),
        None,
        Token::<0>::new(&trace, "input-drop"),
    );
    let future = invoke_typed::<Token<0>, Token<0>, Token<0>, PublicPrincipal>(
        &kernel, snapshot, request, None,
    );
    assert!(matches!(&future.driver.phase, DriverPhase::Unstarted(..)));
    assert!(trace.events.lock().unwrap().is_empty());
    drop(future);
    assert_eq!(trace.count("input-drop"), 1);
    assert_eq!(trace.count("target-drop"), 0);
}
#[tokio::test]
async fn runtime_revision_native_pin_ready_and_error_source_survive_failure() {
    for mode in [Mode::Success, Mode::Failure] {
        let trace = Arc::new(Trace::default());
        let authorization = Authorization(trace.clone());
        let mut bridge = active(&authorization, &trace, mode);
        let success = UnaryTargetPoll { typed: &mut bridge }.await;
        assert!(trace.count("target-poll") >= 2);
        assert_eq!(trace.count("target-drop"), 1);
        if success {
            assert_eq!(trace.count("output-drop"), 0);
            let output = bridge.take_output();
            assert_eq!(trace.count("output-drop"), 0);
            drop(output);
            assert_eq!(trace.count("output-drop"), 1);
        } else {
            let graph = GraphFingerprint::new("graph:guard-failure").unwrap();
            let hooks = TypedInterfaceHookPlan::<Token<0>, Token<0>>::new(graph.clone())
                .bind_failure(
                    PluginIdentity::new("guard.failure").unwrap(),
                    Arc::new(Failure(trace.clone())),
                );
            let context = InterfaceHookContext::new(
                PublicPrincipal::new().summary(),
                InvocationId::now_v7(),
                graph,
                RegistryFingerprint::new("registry:guard").unwrap(),
            );
            hooks
                .run_failure(
                    &mut InvocationFinalization::default(),
                    &context,
                    bridge.error_classification(),
                )
                .await;
            assert_eq!(trace.count("failure"), 1);
            trace.before("target-drop", "failure");
            assert_eq!(trace.count("error-drop"), 0);
            let InterfaceInvocationError::TargetFailed(error) = bridge.take_error() else {
                panic!("original target source")
            };
            let source = error.into_source::<Token<0>>().unwrap();
            drop(source);
            assert_eq!(trace.count("error-drop"), 1);
        }
        drop(bridge);
        assert_eq!(trace.count("target-drop"), 1);
    }
}
#[test]
fn runtime_revision_target_slots_reject_overlap_and_abort_exactly_once() {
    let trace = Arc::new(Trace::default());
    let authorization = Authorization(trace.clone());
    let mut bridge = TypedUnaryBridge::<Token<0>, Token<0>, Token<0>, PublicPrincipal>::new(
        &authorization,
        PublicPrincipal::new(),
        Token::new(&trace, "input-drop"),
    );
    bridge.phase = Phase::Invoking {
        target: Some(Box::pin(Native::new(&trace, Mode::Pending, false))),
        result: Some(Ok(Box::new(Token::new(&trace, "output-drop")))),
    };
    let mut cx = Context::from_waker(Waker::noop());
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| bridge.poll_target(&mut cx)))
            .is_err()
    );
    bridge.abort_target();
    bridge.abort_target();
    drop(bridge);
    assert_eq!(trace.count("target-drop"), 1);
    assert_eq!(trace.count("output-drop"), 2);
}
