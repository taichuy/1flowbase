//! A cancelled protocol future leaves a counted cleanup owner until the child is reaped.
use crate::plugin_scope::PluginScopeLease;
use std::sync::Arc;
use tokio::process::Child;
pub(super) struct ManagedChild {
    child: Option<Child>,
    lease: Option<Arc<PluginScopeLease>>,
}
impl ManagedChild {
    pub(super) fn new(child: Child, lease: Arc<PluginScopeLease>) -> Self {
        Self {
            child: Some(child),
            lease: Some(lease),
        }
    }
    pub(super) fn child(&mut self) -> &mut Child {
        self.child.as_mut().expect("managed child owner")
    }
}
impl Drop for ManagedChild {
    fn drop(&mut self) {
        let Some(mut child) = self.child.take() else {
            return;
        };
        let lease = self.lease.take().expect("managed child lease");
        // Successful exchanges have already waited. All other exits kill and await actual reap.
        if matches!(child.try_wait(), Ok(Some(_))) {
            return;
        }
        tokio::spawn(async move {
            if let Err(error) = child.start_kill() {
                tracing::warn!(%error, "managed child kill request failed; awaiting reap");
            }
            if let Err(error) = child.wait().await {
                tracing::error!(%error, "managed child reap failed; shutdown remains unfinished");
                // Do not turn an unknown process state into a clean resource receipt.
                std::mem::forget(lease);
                return;
            }
            drop(lease);
        });
    }
}
