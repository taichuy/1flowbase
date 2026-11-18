use super::run_compat_route_test;
use std::{
    future::Future,
    pin::Pin,
    rc::Rc,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    task::{Context, Poll},
};

#[test]
fn compat_runtime_owns_non_send_future_through_completion_and_panic() {
    struct Probe {
        panics: bool,
        dropped_inside_runtime: Arc<AtomicBool>,
        _not_send: Rc<()>,
    }

    impl Future for Probe {
        type Output = ();

        fn poll(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<()> {
            assert!(tokio::runtime::Handle::try_current().is_ok());
            assert!(!self.panics, "fixture poll panic");
            Poll::Ready(())
        }
    }

    impl Drop for Probe {
        fn drop(&mut self) {
            self.dropped_inside_runtime.store(
                tokio::runtime::Handle::try_current().is_ok(),
                Ordering::SeqCst,
            );
        }
    }

    for panics in [false, true] {
        let dropped_inside_runtime = Arc::new(AtomicBool::new(false));
        let observed = dropped_inside_runtime.clone();
        let outcome = std::panic::catch_unwind(move || {
            run_compat_route_test(move || {
                assert!(tokio::runtime::Handle::try_current().is_err());
                Probe {
                    panics,
                    dropped_inside_runtime,
                    _not_send: Rc::new(()),
                }
            });
        });
        assert_eq!(outcome.is_err(), panics);
        assert!(observed.load(Ordering::SeqCst));
    }
}
