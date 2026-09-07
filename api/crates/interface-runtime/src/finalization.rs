//! Invocation-owned observer budget, independent of business cancellation/deadline.
//! Timeouts are cooperative: trusted native hooks must not block the executor.
use crate::{InterfaceExtensionPoint, PluginIdentity};
use std::{
    future::{poll_fn, Future},
    panic::{catch_unwind, AssertUnwindSafe},
    time::Duration,
};
use tokio::time::Instant;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InterfaceObserverStatus {
    Executed,
    Failed,
    TimedOut,
    NotRun,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InterfaceObserverRecord {
    plugin: PluginIdentity,
    point: InterfaceExtensionPoint,
    status: InterfaceObserverStatus,
    reason: Option<&'static str>,
}
impl InterfaceObserverRecord {
    pub fn plugin(&self) -> &PluginIdentity {
        &self.plugin
    }
    pub fn point(&self) -> InterfaceExtensionPoint {
        self.point
    }
    pub fn status(&self) -> InterfaceObserverStatus {
        self.status
    }
    pub fn reason(&self) -> Option<&'static str> {
        self.reason
    }
}

#[derive(Default)]
pub(crate) struct InvocationFinalization {
    deadline: Option<Instant>,
    pub(crate) records: Vec<InterfaceObserverRecord>,
}
impl InvocationFinalization {
    pub(crate) async fn observe<F: Future<Output = ()>>(
        &mut self,
        plugin: &PluginIdentity,
        point: InterfaceExtensionPoint,
        construct: impl FnOnce() -> F,
    ) {
        let deadline = *self
            .deadline
            .get_or_insert_with(|| Instant::now() + Duration::from_millis(1000));
        let (status, reason) = if Instant::now() >= deadline {
            (
                InterfaceObserverStatus::NotRun,
                Some("finalization-budget-exhausted"),
            )
        } else {
            // Catch construction as well as polling; no panic payload enters diagnostics.
            match catch_unwind(AssertUnwindSafe(construct)) {
                Err(_) => (InterfaceObserverStatus::Failed, Some("observer-panicked")),
                Ok(future) => {
                    let mut future = Box::pin(future);
                    let guarded = poll_fn(|cx| {
                        match catch_unwind(AssertUnwindSafe(|| future.as_mut().poll(cx))) {
                            Ok(poll) => poll.map(Ok),
                            Err(_) => std::task::Poll::Ready(Err(())),
                        }
                    });
                    match tokio::time::timeout_at(deadline, guarded).await {
                        Ok(Ok(())) => (InterfaceObserverStatus::Executed, None),
                        Ok(Err(())) => (InterfaceObserverStatus::Failed, Some("observer-panicked")),
                        Err(_) => (
                            InterfaceObserverStatus::TimedOut,
                            Some("finalization-budget-exhausted"),
                        ),
                    }
                }
            }
        };
        self.records.push(InterfaceObserverRecord {
            plugin: plugin.clone(),
            point,
            status,
            reason,
        });
    }
}
