//! The same tests overlay the original observer and the shared driver candidate.
use super::{InterfaceObserverStatus, InvocationFinalization};
use crate::{InterfaceExtensionPoint, PluginIdentity};
use std::{
    any::Any,
    cell::Cell,
    future::{poll_fn, ready, Future},
    marker::PhantomPinned,
    panic::{catch_unwind, AssertUnwindSafe},
    pin::Pin,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
    task::{Context, Poll},
    time::Duration,
};
use tokio::time::{sleep, Instant};

type Trace = Arc<Mutex<Vec<&'static str>>>;

fn note(trace: &Trace, event: &'static str) {
    trace.lock().unwrap().push(event);
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Ready,
    Panic,
    Pending,
    PendingThenPanic,
}

struct Probe {
    trace: Trace,
    mode: Mode,
    polls: usize,
    drop_panics: bool,
}
impl Probe {
    fn new(trace: Trace, mode: Mode, drop_panics: bool) -> Self {
        note(&trace, "construct");
        Self {
            trace,
            mode,
            polls: 0,
            drop_panics,
        }
    }
}
impl Future for Probe {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        let this = self.get_mut();
        this.polls += 1;
        match this.mode {
            Mode::Ready => {
                note(&this.trace, "poll-ready");
                Poll::Ready(())
            }
            Mode::PendingThenPanic if this.polls == 1 => {
                note(&this.trace, "poll-pending");
                cx.waker().wake_by_ref();
                Poll::Pending
            }
            Mode::Panic | Mode::PendingThenPanic => {
                note(&this.trace, "poll-panic");
                panic!("private observer poll payload");
            }
            Mode::Pending => {
                note(&this.trace, "poll-pending");
                Poll::Pending
            }
        }
    }
}
impl Drop for Probe {
    fn drop(&mut self) {
        note(&self.trace, "future-drop");
        if self.drop_panics {
            panic!("observer-drop-sentinel");
        }
    }
}

struct Capture {
    trace: Trace,
    drop_panics: bool,
}
impl Drop for Capture {
    fn drop(&mut self) {
        note(&self.trace, "capture-drop");
        if self.drop_panics {
            panic!("observer-drop-sentinel");
        }
    }
}

async fn catch_poll_unwind<F: Future>(future: F) -> Result<F::Output, Box<dyn Any + Send>> {
    let mut future = Box::pin(future);
    poll_fn(
        |cx| match catch_unwind(AssertUnwindSafe(|| future.as_mut().poll(cx))) {
            Ok(poll) => poll.map(Ok),
            Err(payload) => Poll::Ready(Err(payload)),
        },
    )
    .await
}

fn assert_drop_panic(payload: Box<dyn Any + Send>) {
    assert_eq!(
        payload.downcast_ref::<&str>(),
        Some(&"observer-drop-sentinel")
    );
}

#[tokio::test]
async fn observer_is_lazy_and_uses_the_remaining_shared_deadline() {
    let plugin = PluginIdentity::new("driver.lazy").unwrap();
    let trace = Trace::default();
    let capture = Capture {
        trace: Arc::clone(&trace),
        drop_panics: false,
    };
    let mut budget = InvocationFinalization::default();
    let observation = budget.observe(&plugin, InterfaceExtensionPoint::After, move || {
        let _capture = capture;
        Probe::new(Trace::default(), Mode::Ready, false)
    });
    assert!(trace.lock().unwrap().is_empty());
    drop(observation);
    assert_eq!(*trace.lock().unwrap(), ["capture-drop"]);
    assert!(budget.deadline.is_none());
    assert!(budget.records.is_empty());

    let deadline = Instant::now() + Duration::from_millis(200);
    budget.deadline = Some(deadline);
    budget
        .observe(&plugin, InterfaceExtensionPoint::After, || async {
            sleep(Duration::from_millis(5)).await;
        })
        .await;
    assert_eq!(
        budget.records[0].status(),
        InterfaceObserverStatus::Executed
    );
    assert_eq!(budget.deadline, Some(deadline));
    assert!(
        Instant::now() < deadline,
        "first observer consumes only part of the budget"
    );
    budget
        .observe(&plugin, InterfaceExtensionPoint::Failure, || {
            std::future::pending()
        })
        .await;
    assert_eq!(
        budget.records[1].status(),
        InterfaceObserverStatus::TimedOut
    );
    assert_eq!(budget.deadline, Some(deadline));
    let constructs = AtomicUsize::new(0);
    let token = budget
        .observe(&plugin, InterfaceExtensionPoint::Completion, || {
            constructs.fetch_add(1, Ordering::SeqCst);
            ready(())
        })
        .await;
    assert_eq!(constructs.load(Ordering::SeqCst), 0);
    assert_eq!(budget.records.len(), 3);
    assert_eq!(budget.records[2].plugin(), &plugin);
    assert_eq!(
        budget.records[2].point(),
        InterfaceExtensionPoint::Completion
    );
    assert_eq!(budget.records[2].status(), InterfaceObserverStatus::NotRun);
    assert_eq!(
        budget.records[2].reason(),
        Some("finalization-budget-exhausted")
    );
    let record = budget.records[2].clone();
    budget.record_reported_failure(token);
    assert_eq!(budget.records[2], record);
}

#[tokio::test]
async fn observer_catches_a_second_poll_panic_with_the_original_reason() {
    let plugin = PluginIdentity::new("driver.second-poll").unwrap();
    let trace = Trace::default();
    let mut budget = InvocationFinalization::default();
    let token = budget
        .observe(&plugin, InterfaceExtensionPoint::Failure, || {
            Probe::new(Arc::clone(&trace), Mode::PendingThenPanic, false)
        })
        .await;
    assert_eq!(
        *trace.lock().unwrap(),
        ["construct", "poll-pending", "poll-panic", "future-drop"]
    );
    assert_eq!(budget.records.len(), 1);
    assert_eq!(budget.records[0].plugin(), &plugin);
    assert_eq!(budget.records[0].point(), InterfaceExtensionPoint::Failure);
    assert_eq!(budget.records[0].status(), InterfaceObserverStatus::Failed);
    assert_eq!(budget.records[0].reason(), Some("observer-panicked"));
    let record = budget.records[0].clone();
    budget.record_reported_failure(token);
    assert_eq!(budget.records[0], record);
}

#[tokio::test]
async fn observer_drops_before_recording_and_preserves_outer_drop_and_drop_panics() {
    let plugin = PluginIdentity::new("driver.drop").unwrap();
    for mode in [Mode::Ready, Mode::Panic, Mode::Pending] {
        for drop_panics in [false, true] {
            let trace = Trace::default();
            let mut budget = InvocationFinalization::default();
            if mode == Mode::Pending {
                budget.deadline = Some(Instant::now() + Duration::from_millis(30));
            }
            let outcome =
                catch_poll_unwind(budget.observe(&plugin, InterfaceExtensionPoint::After, || {
                    Probe::new(Arc::clone(&trace), mode, drop_panics)
                }))
                .await;
            assert_eq!(trace.lock().unwrap().last(), Some(&"future-drop"));
            if drop_panics {
                match outcome {
                    Err(payload) => assert_drop_panic(payload),
                    Ok(_) => panic!("future Drop panic must propagate"),
                }
                assert!(budget.records.is_empty(), "Drop precedes record push");
                continue;
            }
            let token = match outcome {
                Ok(token) => token,
                Err(_) => panic!("non-panicking Drop must finish observation"),
            };
            assert_eq!(budget.records.len(), 1);
            assert_eq!(budget.records[0].plugin(), &plugin);
            assert_eq!(budget.records[0].point(), InterfaceExtensionPoint::After);
            let (status, reason) = match mode {
                Mode::Ready => (InterfaceObserverStatus::Executed, None),
                Mode::Panic => (InterfaceObserverStatus::Failed, Some("observer-panicked")),
                Mode::Pending => (
                    InterfaceObserverStatus::TimedOut,
                    Some("finalization-budget-exhausted"),
                ),
                Mode::PendingThenPanic => unreachable!(),
            };
            assert_eq!(budget.records[0].status(), status);
            assert_eq!(budget.records[0].reason(), reason);
            let record = budget.records[0].clone();
            budget.record_reported_failure(token);
            if status == InterfaceObserverStatus::Executed {
                assert_eq!(budget.records[0].status(), InterfaceObserverStatus::Failed);
                assert_eq!(
                    budget.records[0].reason(),
                    Some("observer-reported-failure")
                );
            } else {
                assert_eq!(budget.records[0], record);
            }
            let next = budget
                .observe(&plugin, InterfaceExtensionPoint::Completion, || {
                    assert_eq!(trace.lock().unwrap().last(), Some(&"future-drop"));
                    Probe::new(Arc::clone(&trace), Mode::Ready, false)
                })
                .await;
            assert_eq!(budget.records.len(), 2);
            assert_eq!(
                budget.records[1].status(),
                if mode == Mode::Pending {
                    InterfaceObserverStatus::NotRun
                } else {
                    InterfaceObserverStatus::Executed
                }
            );
            if mode == Mode::Pending {
                let record = budget.records[1].clone();
                budget.record_reported_failure(next);
                assert_eq!(budget.records[1], record);
            }
        }
    }

    for drop_panics in [false, true] {
        let trace = Trace::default();
        let mut budget = InvocationFinalization::default();
        let mut observation = Box::pin(budget.observe(
            &plugin,
            InterfaceExtensionPoint::Completion,
            || Probe::new(Arc::clone(&trace), Mode::Pending, drop_panics),
        ));
        assert!(poll_fn(|cx| Poll::Ready(observation.as_mut().poll(cx)))
            .await
            .is_pending());
        let outcome = catch_unwind(AssertUnwindSafe(|| drop(observation)));
        match outcome {
            Err(payload) if drop_panics => assert_drop_panic(payload),
            Ok(()) if !drop_panics => {}
            _ => panic!("outer drop must preserve the future destructor outcome"),
        }
        assert!(budget.records.is_empty());
        assert_eq!(
            *trace.lock().unwrap(),
            ["construct", "poll-pending", "future-drop"]
        );
    }

    let trace = Trace::default();
    let constructs = Arc::new(AtomicUsize::new(0));
    let count = Arc::clone(&constructs);
    let capture = Capture {
        trace: Arc::clone(&trace),
        drop_panics: true,
    };
    let mut budget = InvocationFinalization {
        deadline: Some(Instant::now() - Duration::from_millis(1)),
        ..Default::default()
    };
    let outcome = catch_poll_unwind(budget.observe(
        &plugin,
        InterfaceExtensionPoint::Completion,
        move || {
            let _capture = capture;
            count.fetch_add(1, Ordering::SeqCst);
            ready(())
        },
    ))
    .await;
    match outcome {
        Err(payload) => assert_drop_panic(payload),
        Ok(_) => panic!("unused constructor capture Drop panic must propagate"),
    }
    assert_eq!(constructs.load(Ordering::SeqCst), 0);
    assert_eq!(*trace.lock().unwrap(), ["capture-drop"]);
    assert_eq!(
        budget.records.len(),
        1,
        "unused constructor drops after record push"
    );
    assert_eq!(budget.records[0].status(), InterfaceObserverStatus::NotRun);
    assert_eq!(
        budget.records[0].reason(),
        Some("finalization-budget-exhausted")
    );
}

struct BorrowedPinned<'a> {
    value: &'a mut Cell<usize>,
    _pin: PhantomPinned,
}
impl Future for BorrowedPinned<'_> {
    type Output = ();

    fn poll(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<()> {
        self.as_ref().get_ref().value.set(7);
        Poll::Ready(())
    }
}
fn assert_send<T: Send>(_: &T) {}

#[tokio::test]
async fn observer_preserves_borrowed_mut_nonstatic_pinned_futures_and_send() {
    let plugin = PluginIdentity::new("driver.borrowed").unwrap();
    let mut budget = InvocationFinalization::default();
    let mut value = Cell::new(0);
    let borrowed = &mut value;
    let observation = budget.observe(&plugin, InterfaceExtensionPoint::After, move || {
        BorrowedPinned {
            value: borrowed,
            _pin: PhantomPinned,
        }
    });
    assert_send(&observation);
    let first = observation.await;
    assert_eq!(value.get(), 7);
    let observation = budget.observe(&plugin, InterfaceExtensionPoint::Completion, || ready(()));
    assert_send(&observation);
    let second = observation.await;
    assert_eq!((first.0, second.0), (0, 1));
    budget.record_reported_failure(first);
    assert_eq!(budget.records[0].status(), InterfaceObserverStatus::Failed);
    assert_eq!(
        budget.records[0].reason(),
        Some("observer-reported-failure")
    );
    assert_eq!(
        budget.records[1].status(),
        InterfaceObserverStatus::Executed
    );
    assert_eq!(budget.records[1].reason(), None);
    budget.record_reported_failure(second);
    assert_eq!(budget.records[1].status(), InterfaceObserverStatus::Failed);
    assert_eq!(
        budget.records[1].reason(),
        Some("observer-reported-failure")
    );
}
