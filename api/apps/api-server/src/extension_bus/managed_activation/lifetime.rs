use super::*;
pub use api_host_kernel::extension_bus::managed_activation::lifetime::*;
impl ManagedExtensionComposition {
    pub(crate) fn close_owned_admission(&self) {
        self.reclamation.stop();
        self.operations.close();
    }
    pub(crate) async fn wait_owned_shutdown(&self, budget: std::time::Duration) -> Result<()> {
        self.reclamation.wait_stopped(budget).await?;
        self.operations.wait(budget).await
    }
    pub(crate) async fn wait_for_shutdown(&self, budget: std::time::Duration) -> Result<()> {
        self.reclamation.wait_stopped(budget).await?;
        self.operations.wait(budget).await?;
        // Assembly completes before graph gates close; admitted Create/E2 references may finish.
        let lifetimes = {
            let visible = self.snapshots.lock().await;
            visible
                .current
                .values()
                .chain(visible.retained.values().flatten())
                .map(|snapshot| snapshot.lifetime.clone())
                .collect::<Vec<_>>()
        };
        for lifetime in &lifetimes {
            lifetime.close_for_shutdown();
        }
        tokio::time::timeout(budget, async {
            for lifetime in &lifetimes {
                lifetime.wait_references().await;
            }
        })
        .await
        .map_err(|_| {
            anyhow::anyhow!(
                "managed frozen references unfinished: {}",
                lifetimes.iter().map(|l| l.reference_count()).sum::<usize>()
            )
        })?;
        Ok(())
    }
    /// Called only after every owned operation, frozen reference and runtime worker has completed.
    pub(crate) async fn cleanup_after_shutdown(&self) {
        *self.snapshots.lock().await = ManagedSnapshots::default();
        self.refresh_lifecycle_source_observations().await;
    }
}

#[cfg_attr(test, api_test_selection::group(core))]
#[cfg(test)]
#[path = "_tests/lifetime.rs"]
mod tests;
