use super::ErasedRouteHandler;
use axum::{
    body::{to_bytes, Body},
    extract::{Json, Request},
    handler::Handler,
    http::{HeaderMap, Method, StatusCode},
    response::{IntoResponse, Response},
    routing::{any, get, post, MethodFilter},
    Router,
};
use std::{
    future::Future,
    panic::{catch_unwind, AssertUnwindSafe},
    pin::Pin,
    rc::Rc,
    sync::{Arc, Mutex},
    task::{Context, Poll, Wake, Waker},
};
use tower::ServiceExt;

type Trace = Arc<Mutex<Vec<&'static str>>>;

// A non-Clone/Send/Sync marker proves that only the original Handler bounds apply.
struct First {
    _not_thread_safe: Rc<()>,
}
struct Second;

#[derive(Clone, Copy)]
enum Outcome {
    Pending,
    Ready,
    CallPanic,
    PollPanic,
}

struct State {
    token: Box<u8>,
    trace: Trace,
}

impl Clone for State {
    fn clone(&self) -> Self {
        self.trace.lock().unwrap().push("state.clone");
        Self {
            token: self.token.clone(),
            trace: self.trace.clone(),
        }
    }
}

struct Probe {
    trace: Trace,
    expected_token: usize,
    outcome: Outcome,
}

impl Clone for Probe {
    fn clone(&self) -> Self {
        self.trace.lock().unwrap().push("handler.clone");
        Self {
            trace: self.trace.clone(),
            expected_token: self.expected_token,
            outcome: self.outcome,
        }
    }
}

impl Drop for Probe {
    fn drop(&mut self) {
        self.trace.lock().unwrap().push("handler.drop");
    }
}

struct ProbeFuture {
    probe: Probe,
    _state: State,
}

impl Future for ProbeFuture {
    type Output = Response;

    fn poll(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Self::Output> {
        self.probe.trace.lock().unwrap().push("future.poll");
        match self.probe.outcome {
            Outcome::Pending => Poll::Pending,
            Outcome::Ready => Poll::Ready("ready".into_response()),
            Outcome::PollPanic => panic!("original future panic"),
            Outcome::CallPanic => unreachable!(),
        }
    }
}

impl Drop for ProbeFuture {
    fn drop(&mut self) {
        self.probe.trace.lock().unwrap().push("future.drop");
    }
}

macro_rules! probe_handler {
    ($marker:ty, $event:literal) => {
        impl Handler<$marker, State> for Probe {
            type Future = ProbeFuture;

            fn call(self, _: Request, state: State) -> Self::Future {
                assert_eq!(
                    self.expected_token,
                    state.token.as_ref() as *const u8 as usize
                );
                self.trace.lock().unwrap().push($event);
                if matches!(self.outcome, Outcome::CallPanic) {
                    panic!("original handler panic");
                }
                ProbeFuture {
                    probe: self,
                    _state: state,
                }
            }
        }
    };
}
probe_handler!(First, "handler.call.first");
probe_handler!(Second, "handler.call.second");

fn probe(outcome: Outcome) -> (Probe, State, Trace) {
    let trace = Trace::default();
    let state = State {
        token: Box::new(17),
        trace: trace.clone(),
    };
    let handler = Probe {
        trace: trace.clone(),
        expected_token: state.token.as_ref() as *const u8 as usize,
        outcome,
    };
    (handler, state, trace)
}

fn request() -> Request {
    Request::new(Body::empty())
}

struct Noop;
impl Wake for Noop {
    fn wake(self: Arc<Self>) {}
}

fn poll_once<F: Future + ?Sized>(future: Pin<&mut F>) -> Poll<F::Output> {
    let waker = Waker::from(Arc::new(Noop));
    future.poll(&mut Context::from_waker(&waker))
}

#[test]
fn clone_and_call_are_immediate_and_preserve_the_exact_state() {
    let (raw, state, raw_trace) = probe(Outcome::Ready);
    let cloned = raw.clone();
    let raw_future = <Probe as Handler<First, State>>::call(cloned, request(), state);
    let expected = raw_trace.lock().unwrap().clone();
    assert_eq!(expected, ["handler.clone", "handler.call.first"]);

    let (original, state, erased_trace) = probe(Outcome::Ready);
    let erased = ErasedRouteHandler::new::<_, First>(original);
    let cloned = erased.clone();
    assert_eq!(*erased_trace.lock().unwrap(), ["handler.clone"]);
    let erased_future =
        <ErasedRouteHandler<State> as Handler<(), State>>::call(cloned, request(), state);
    assert_eq!(*erased_trace.lock().unwrap(), expected);
    drop(raw_future);
    drop(erased_future);
    drop(raw);
    drop(erased);
    assert_eq!(*erased_trace.lock().unwrap(), *raw_trace.lock().unwrap());
}

fn lifecycle(outcome: Outcome, polls: usize, erase: bool) -> Vec<&'static str> {
    let (handler, state, trace) = probe(outcome);
    let mut future: Pin<Box<dyn Future<Output = Response> + Send>> = if erase {
        let erased = ErasedRouteHandler::new::<_, First>(handler);
        <ErasedRouteHandler<State> as Handler<(), State>>::call(erased, request(), state)
    } else {
        Box::pin(<Probe as Handler<First, State>>::call(
            handler,
            request(),
            state,
        ))
    };
    assert_eq!(*trace.lock().unwrap(), ["handler.call.first"]);
    for _ in 0..polls {
        let result = poll_once(future.as_mut());
        assert_eq!(result.is_ready(), matches!(outcome, Outcome::Ready));
        // A ready future is deliberately held; neither original value may drop yet.
        assert!(!trace
            .lock()
            .unwrap()
            .iter()
            .any(|event| event.ends_with("drop")));
    }
    drop(future);
    let events = trace.lock().unwrap().clone();
    assert!(events.ends_with(&["future.drop", "handler.drop"]));
    events
}

#[test]
fn unpolled_pending_cancel_and_ready_held_drop_match_raw_handler() {
    for (outcome, polls) in [
        (Outcome::Ready, 0),
        (Outcome::Pending, 1),
        (Outcome::Ready, 1),
    ] {
        assert_eq!(
            lifecycle(outcome, polls, true),
            lifecycle(outcome, polls, false)
        );
    }
}

#[test]
fn original_handler_and_future_panics_keep_their_timing_and_drop_trace() {
    for outcome in [Outcome::CallPanic, Outcome::PollPanic] {
        let mut traces = Vec::new();
        for erase in [false, true] {
            let (handler, state, trace) = probe(outcome);
            let result = catch_unwind(AssertUnwindSafe(|| {
                let mut future: Pin<Box<dyn Future<Output = Response> + Send>> = if erase {
                    <ErasedRouteHandler<State> as Handler<(), State>>::call(
                        ErasedRouteHandler::new::<_, First>(handler),
                        request(),
                        state,
                    )
                } else {
                    Box::pin(<Probe as Handler<First, State>>::call(
                        handler,
                        request(),
                        state,
                    ))
                };
                assert_eq!(*trace.lock().unwrap(), ["handler.call.first"]);
                let _ = poll_once(future.as_mut());
            }));
            assert!(result.is_err());
            traces.push(trace.lock().unwrap().clone());
        }
        assert_eq!(traces[0], traces[1]);
    }
}

#[test]
fn same_handler_keeps_the_selected_original_marker() {
    let (handler, state, trace) = probe(Outcome::Ready);
    let erased = ErasedRouteHandler::new::<_, Second>(handler);
    let future = <ErasedRouteHandler<State> as Handler<(), State>>::call(erased, request(), state);
    assert_eq!(*trace.lock().unwrap(), ["handler.call.second"]);
    drop(future);
}

#[test]
fn oracle_rejects_arc_shared_clone_and_lazy_call_counterexamples() {
    let (handler, state, trace) = probe(Outcome::Ready);
    let raw = handler.clone();
    let raw_future = <Probe as Handler<First, State>>::call(raw, request(), state);
    let oracle = trace.lock().unwrap().clone();
    assert_eq!(oracle, ["handler.clone", "handler.call.first"]);

    let (handler, _, trace) = probe(Outcome::Ready);
    let shared = Arc::new(handler);
    let shared_clone = shared.clone();
    assert_ne!(*trace.lock().unwrap(), &oracle[..1]);
    drop(shared_clone);
    drop(shared);

    let (handler, state, trace) = probe(Outcome::Ready);
    let cloned = handler.clone();
    let lazy =
        async move { <Probe as Handler<First, State>>::call(cloned, request(), state).await };
    assert_ne!(*trace.lock().unwrap(), oracle);
    drop(lazy);
    assert!(!trace.lock().unwrap().contains(&"handler.call.first"));
    drop(raw_future);
}

async fn success() -> impl IntoResponse {
    (
        StatusCode::CREATED,
        [("x-handler", "original")],
        "response-body",
    )
}

async fn failure() -> Result<&'static str, (StatusCode, &'static str)> {
    Err((StatusCode::CONFLICT, "original-error"))
}

async fn json(value: Json<serde_json::Value>) -> Json<serde_json::Value> {
    value
}

struct RouteCapture(Trace);

impl Clone for RouteCapture {
    fn clone(&self) -> Self {
        self.0.lock().unwrap().push("handler.clone");
        Self(self.0.clone())
    }
}

impl Drop for RouteCapture {
    fn drop(&mut self) {
        self.0.lock().unwrap().push("handler.drop");
    }
}

impl RouteCapture {
    fn respond(self) -> std::future::Ready<Response> {
        self.0.lock().unwrap().push("handler.call");
        std::future::ready("traced".into_response())
    }
}

fn traced_handler(trace: Trace) -> impl FnOnce() -> std::future::Ready<Response> + Clone {
    let capture = RouteCapture(trace);
    move || capture.respond()
}

#[derive(Debug, PartialEq)]
struct HttpResult {
    status: StatusCode,
    headers: HeaderMap,
    body: Vec<u8>,
}

async fn response(router: Router, method: Method, body: &'static str, json: bool) -> HttpResult {
    let mut builder = Request::builder().method(method).uri("/");
    if json {
        builder = builder.header("content-type", "application/json");
    }
    let response = router
        .oneshot(builder.body(Body::from(body)).unwrap())
        .await
        .unwrap();
    let (parts, body) = response.into_parts();
    HttpResult {
        status: parts.status,
        headers: parts.headers,
        body: to_bytes(body, usize::MAX).await.unwrap().to_vec(),
    }
}

#[tokio::test]
async fn actual_axum_router_clone_call_and_drop_trace_match() {
    let mut results = Vec::new();
    for erase in [false, true] {
        let trace = Trace::default();
        let handler = traced_handler(trace.clone());
        let methods = if erase {
            get(ErasedRouteHandler::new(handler))
        } else {
            get(handler)
        };
        let router = Router::new().route("/", methods);
        let cloned = router.clone();
        let result = response(cloned, Method::GET, "", false).await;
        drop(router);
        let events = trace.lock().unwrap().clone();
        assert!(events.contains(&"handler.clone"));
        assert!(events.contains(&"handler.call"));
        results.push((result, events));
    }
    assert_eq!(results[0], results[1]);
}

#[tokio::test]
async fn actual_axum_get_head_post_and_405_allow_match() {
    let raw = Router::new().route("/", get(success).post(failure));
    let erased = Router::new().route(
        "/",
        axum::routing::on(MethodFilter::GET, ErasedRouteHandler::new(success))
            .on(MethodFilter::POST, ErasedRouteHandler::new(failure)),
    );
    for method in [
        Method::GET,
        Method::HEAD,
        Method::POST,
        Method::PUT,
        Method::OPTIONS,
    ] {
        let expected = response(raw.clone(), method.clone(), "", false).await;
        assert_eq!(
            response(erased.clone(), method.clone(), "", false).await,
            expected
        );
        match method {
            Method::GET => {
                assert_eq!(expected.status, StatusCode::CREATED);
                assert_eq!(expected.body, b"response-body");
            }
            Method::HEAD => {
                assert_eq!(expected.status, StatusCode::CREATED);
                assert!(expected.body.is_empty());
            }
            Method::POST => {
                assert_eq!(expected.status, StatusCode::CONFLICT);
                assert_eq!(expected.body, b"original-error");
            }
            _ => {
                assert_eq!(expected.status, StatusCode::METHOD_NOT_ALLOWED);
                assert_eq!(expected.headers["allow"], "GET,HEAD,POST");
            }
        }
    }
}

#[tokio::test]
async fn actual_axum_any_accepts_custom_methods_and_skips_allow_header() {
    let raw = Router::new().route("/", any(success));
    let erased = Router::new().route("/", any(ErasedRouteHandler::new(success)));
    for method in [
        Method::GET,
        Method::HEAD,
        Method::OPTIONS,
        Method::from_bytes(b"CUSTOM").unwrap(),
    ] {
        let expected = response(raw.clone(), method.clone(), "", false).await;
        assert_eq!(expected.status, StatusCode::CREATED);
        assert!(!expected.headers.contains_key("allow"));
        assert_eq!(response(erased.clone(), method, "", false).await, expected);
    }
}

#[tokio::test]
async fn actual_axum_non_function_response_handler_preserves_private_marker() {
    let raw = Router::new().route("/", get(StatusCode::ACCEPTED));
    let erased = Router::new().route("/", get(ErasedRouteHandler::new(StatusCode::ACCEPTED)));
    for method in [Method::GET, Method::HEAD, Method::POST] {
        let expected = response(raw.clone(), method.clone(), "", false).await;
        assert_eq!(response(erased.clone(), method, "", false).await, expected);
    }
}

#[tokio::test]
async fn actual_axum_extractor_success_and_rejections_match() {
    let raw = Router::new().route("/", post(json));
    let erased = Router::new().route("/", post(ErasedRouteHandler::new(json)));
    for (body, content_type, status) in [
        ("{\"value\":17}", true, StatusCode::OK),
        ("{", true, StatusCode::BAD_REQUEST),
        ("{}", false, StatusCode::UNSUPPORTED_MEDIA_TYPE),
    ] {
        let expected = response(raw.clone(), Method::POST, body, content_type).await;
        assert_eq!(expected.status, status);
        assert_eq!(
            response(erased.clone(), Method::POST, body, content_type).await,
            expected
        );
    }
}
