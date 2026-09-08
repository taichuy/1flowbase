use std::sync::{Arc, Mutex};

use extension_package_runtime::error::{FrameworkResult, PluginFrameworkError};
use runtime_core::runtime_backend::RuntimeBackendError;
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
    drains: usize,
}

/// Per-mount runtime ownership boundary. A new load receives a new generation; disposal closes
/// admission first and waits for already-admitted operations to end before effects are removed.
#[derive(Debug)]
pub(crate) struct PluginScope {
    generation: u64,
    admission: Mutex<PluginScopeAdmission>,
    drained: Notify,
    managed_budget: Option<Arc<tokio::sync::Semaphore>>,
}

pub(crate) struct PluginScopeLease {
    scope: Arc<PluginScope>,
    _managed_permit: Option<tokio::sync::OwnedSemaphorePermit>,
}

pub(crate) struct PluginScopeDrain {
    scope: Arc<PluginScope>,
}
impl Drop for PluginScopeDrain {
    fn drop(&mut self) {
        if let Ok(mut admission) = self.scope.admission.lock() {
            admission.drains = admission.drains.saturating_sub(1);
        }
    }
}
impl PluginScopeDrain {
    pub(crate) async fn wait_drained(&self) -> FrameworkResult<()> {
        self.scope.wait_until_drained().await
    }
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
                drains: 0,
            }),
            drained: Notify::new(),
            managed_budget: None,
        })
    }

    pub(crate) fn managed(generation: u64, budget: Arc<tokio::sync::Semaphore>) -> Arc<Self> {
        Arc::new(Self {
            generation,
            admission: Mutex::new(PluginScopeAdmission {
                state: PluginScopeState::Mounted,
                in_flight: 0,
                drains: 0,
            }),
            drained: Notify::new(),
            managed_budget: Some(budget),
        })
    }

    pub(crate) fn admit(self: &Arc<Self>) -> FrameworkResult<PluginScopeLease> {
        self.admit_budgeted().map_err(|error| match error {
            RuntimeBackendError::Contract(error) => *error,
            error => PluginFrameworkError::invalid_provider_package(error.to_string()),
        })
    }

    fn admit_budgeted(self: &Arc<Self>) -> Result<PluginScopeLease, RuntimeBackendError> {
        let mut admission = self.lock_admission()?;
        if admission.state != PluginScopeState::Mounted || admission.drains != 0 {
            return Err(PluginFrameworkError::invalid_provider_package(format!(
                "plugin scope generation {} is not accepting calls",
                self.generation
            ))
            .into());
        }
        let managed_permit = if let Some(budget) = &self.managed_budget {
            if admission.in_flight >= 32 {
                return Err(RuntimeBackendError::Execution {
                    target_id: "managed-capacity".into(),
                    message: "managed scope call capacity exhausted".into(),
                });
            }
            Some(budget.clone().try_acquire_owned().map_err(|_| {
                RuntimeBackendError::Execution {
                    target_id: "managed-capacity".into(),
                    message: "managed worker capacity exhausted".into(),
                }
            })?)
        } else {
            None
        };
        admission.in_flight = admission.in_flight.checked_add(1).ok_or_else(|| {
            PluginFrameworkError::invalid_provider_package("scope call count exhausted")
        })?;
        Ok(PluginScopeLease {
            scope: Arc::clone(self),
            _managed_permit: managed_permit,
        })
    }

    pub(crate) fn admit_generation(
        self: &Arc<Self>,
        generation: u64,
    ) -> Result<PluginScopeLease, RuntimeBackendError> {
        if self.generation != generation {
            return Err(PluginFrameworkError::invalid_provider_package(
                "plugin scope generation does not match execution handle",
            )
            .into());
        }
        self.admit_budgeted()
    }

    pub(crate) fn begin_drain(self: &Arc<Self>) -> FrameworkResult<PluginScopeDrain> {
        let mut admission = self.lock_admission()?;
        if admission.state != PluginScopeState::Mounted {
            return Err(PluginFrameworkError::invalid_provider_package(
                "managed scope is already disposing",
            ));
        }
        admission.drains = admission.drains.checked_add(1).ok_or_else(|| {
            PluginFrameworkError::invalid_provider_package("managed drain count exhausted")
        })?;
        Ok(PluginScopeDrain {
            scope: self.clone(),
        })
    }

    pub(crate) fn close_admission(&self) -> FrameworkResult<()> {
        let mut admission = self.lock_admission()?;
        if admission.state == PluginScopeState::Mounted {
            admission.state = PluginScopeState::Disposing;
        }
        Ok(())
    }

    pub(crate) async fn dispose(&self) -> FrameworkResult<()> {
        self.close_admission()?;
        self.wait_until_drained().await?;
        self.lock_admission()?.state = PluginScopeState::Disposed;
        Ok(())
    }

    async fn wait_until_drained(&self) -> FrameworkResult<()> {
        loop {
            let notified = self.drained.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            if self.lock_admission()?.in_flight == 0 {
                return Ok(());
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
