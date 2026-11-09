//! Invocation-owned observer budget, independent of business cancellation/deadline.
//! Timeouts are cooperative: trusted native hooks must not block the executor.
use crate::{InterfaceExtensionPoint, PluginIdentity};
use std::{
    future::{poll_fn, Future},
    panic::{catch_unwind, AssertUnwindSafe},
    pin::Pin,
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

pub(crate) struct ObserverRecordToken(usize);

#[derive(Default)]
pub(crate) struct InvocationFinalization {
    deadline: Option<Instant>,
    pub(crate) records: Vec<InterfaceObserverRecord>,
}
impl InvocationFinalization {
    pub(crate) async fn observe<'future, F: Future<Output = ()> + Send + 'future>(
        &mut self,
        plugin: &PluginIdentity,
        point: InterfaceExtensionPoint,
        construct: impl FnOnce() -> F + Send,
    ) -> ObserverRecordToken {
        let mut construct = Some(construct);
        let mut factory = || {
            let construct = construct
                .take()
                .expect("the observer driver constructs at most once");
            let future = catch_unwind(AssertUnwindSafe(construct)).map_err(|_| ())?;
            Ok(Box::pin(future) as Pin<Box<dyn Future<Output = ()> + Send + 'future>>)
        };
        self.observe_driver(plugin, point, &mut factory).await
    }

    async fn observe_driver<'future>(
        &mut self,
        plugin: &PluginIdentity,
        point: InterfaceExtensionPoint,
        factory: &mut (dyn FnMut() -> Result<Pin<Box<dyn Future<Output = ()> + Send + 'future>>, ()>
                  + Send),
    ) -> ObserverRecordToken {
        let deadline = *self
            .deadline
            .get_or_insert_with(|| Instant::now() + Duration::from_millis(1000));
        let (status, reason) = if Instant::now() >= deadline {
            (
                InterfaceObserverStatus::NotRun,
                Some("finalization-budget-exhausted"),
            )
        } else {
            match factory() {
                Err(()) => (InterfaceObserverStatus::Failed, Some("observer-panicked")),
                Ok(mut future) => {
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
        let token = ObserverRecordToken(self.records.len());
        self.records.push(InterfaceObserverRecord {
            plugin: plugin.clone(),
            point,
            status,
            reason,
        });
        token
    }

    pub(crate) fn record_reported_failure(&mut self, token: ObserverRecordToken) {
        if let Some(record) = self.records.get_mut(token.0) {
            if record.status == InterfaceObserverStatus::Executed {
                record.status = InterfaceObserverStatus::Failed;
                record.reason = Some("observer-reported-failure");
            }
        }
    }
}

#[cfg(test)]
#[path = "_tests/finalization_driver_tests.rs"]
mod driver_tests;
