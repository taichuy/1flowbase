//! Explicit invocation cancellation. Dropping a caller is deliberately not a signal.
use super::*;
use std::sync::Mutex as StdMutex;
use tokio::sync::watch;

#[derive(Clone)]
pub(crate) struct ProviderCallCancellation {
    signal: watch::Sender<bool>,
    cleanup_failure: Arc<StdMutex<Option<String>>>,
}

impl ProviderCallCancellation {
    pub(crate) fn new() -> Self {
        Self {
            signal: watch::channel(false).0,
            cleanup_failure: Arc::default(),
        }
    }

    pub(crate) fn cancel(&self) {
        self.signal.send_replace(true);
    }

    pub(crate) fn is_cancelled(&self) -> bool {
        *self.signal.borrow()
    }

    pub(crate) fn subscribe(&self) -> watch::Receiver<bool> {
        self.signal.subscribe()
    }

    pub(crate) async fn cancelled(&self) {
        let mut signal = self.subscribe();
        while !*signal.borrow_and_update() {
            if signal.changed().await.is_err() {
                return;
            }
        }
    }

    pub(crate) fn failed_cleanup(&self, message: impl Into<String>) {
        let mut failure = match self.cleanup_failure.lock() {
            Ok(failure) => failure,
            Err(poisoned) => poisoned.into_inner(),
        };
        failure.get_or_insert_with(|| message.into());
    }

    pub(crate) fn cleanup_failure(&self) -> Option<String> {
        match self.cleanup_failure.lock() {
            Ok(failure) => failure.clone(),
            Err(poisoned) => poisoned.into_inner().clone(),
        }
    }

    pub(crate) async fn run<T>(
        cancellation: Option<&Self>,
        future: impl std::future::Future<Output = FrameworkResult<T>>,
    ) -> FrameworkResult<T> {
        match cancellation {
            None => future.await,
            Some(cancellation) => tokio::select! {
                biased;
                _ = cancellation.cancelled() => Err(Self::error()),
                result = future => result,
            },
        }
    }

    pub(crate) fn error() -> PluginFrameworkError {
        PluginFrameworkError::runtime(ProviderRuntimeError::normalize(
            "provider_request_cancelled",
            "provider invocation explicitly cancelled",
            None,
        ))
    }
}
