use std::sync::{Arc, Mutex};

use plugin_framework::error::{FrameworkResult, PluginFrameworkError};
use tokio::sync::Notify;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PluginScopeState {
    Mounted,
    Disposing,
    Disposed,
}

#[derive(Debug)]
struct PluginScopeAdmission {
    state: PluginScopeState,
    in_flight: usize,
}

/// Per-mount runtime ownership boundary. A new load receives a new generation; disposal closes
/// admission first and waits for already-admitted operations to end before effects are removed.
#[derive(Debug)]
pub(crate) struct PluginScope {
    generation: u64,
    admission: Mutex<PluginScopeAdmission>,
    drained: Notify,
}

pub(crate) struct PluginScopeLease {
    scope: Arc<PluginScope>,
}

impl Drop for PluginScopeLease {
    fn drop(&mut self) {
        if let Ok(mut admission) = self.scope.admission.lock() {
            admission.in_flight = admission.in_flight.saturating_sub(1);
            if admission.in_flight == 0 {
                self.scope.drained.notify_waiters();
            }
        }
    }
}

impl PluginScope {
    pub(crate) fn mounted(generation: u64) -> Arc<Self> {
        Arc::new(Self {
            generation,
            admission: Mutex::new(PluginScopeAdmission {
                state: PluginScopeState::Mounted,
                in_flight: 0,
            }),
            drained: Notify::new(),
        })
    }

    pub(crate) fn admit(self: &Arc<Self>) -> FrameworkResult<PluginScopeLease> {
        let mut admission = self.lock_admission()?;
        if admission.state != PluginScopeState::Mounted {
            return Err(PluginFrameworkError::invalid_provider_package(format!(
                "plugin scope generation {} is not accepting calls",
                self.generation
            )));
        }
        admission.in_flight = admission.in_flight.saturating_add(1);
        Ok(PluginScopeLease {
            scope: Arc::clone(self),
        })
    }

    pub(crate) async fn dispose(&self) -> FrameworkResult<()> {
        {
            let mut admission = self.lock_admission()?;
            match admission.state {
                PluginScopeState::Mounted => admission.state = PluginScopeState::Disposing,
                PluginScopeState::Disposing | PluginScopeState::Disposed => return Ok(()),
            }
        }
        self.wait_until_drained().await;
        self.lock_admission()?.state = PluginScopeState::Disposed;
        Ok(())
    }

    async fn wait_until_drained(&self) {
        loop {
            let notified = self.drained.notified();
            if self
                .admission
                .lock()
                .map(|admission| admission.in_flight == 0)
                .unwrap_or(true)
            {
                return;
            }
            notified.await;
        }
    }

    fn lock_admission(&self) -> FrameworkResult<std::sync::MutexGuard<'_, PluginScopeAdmission>> {
        self.admission.lock().map_err(|_| {
            PluginFrameworkError::invalid_provider_package("plugin scope admission is unavailable")
        })
    }
}

#[cfg(test)]
mod tests {
    use super::PluginScope;

    #[tokio::test]
    async fn dispose_rejects_new_calls_and_waits_for_the_mounted_generation() {
        let scope = PluginScope::mounted(7);
        let lease = scope.admit().unwrap();
        let disposing_scope = scope.clone();
        let disposing = tokio::spawn(async move { disposing_scope.dispose().await });

        tokio::task::yield_now().await;
        assert!(scope.admit().is_err());
        assert!(!disposing.is_finished());

        drop(lease);
        disposing.await.unwrap().unwrap();
        assert!(scope.admit().is_err());
    }
}
