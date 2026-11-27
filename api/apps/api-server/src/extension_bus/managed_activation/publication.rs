//! Connect validated runtime candidates to the host's single ingress publication owner.
use super::*;
use crate::managed_publication::{ManagedGenerationPublisher, PreparedGeneration};
use crate::managed_services::ManagedServiceRegistration;

impl ManagedExtensionComposition {
    pub(crate) fn attach_generation_publisher(
        self: &Arc<Self>,
        publisher: std::sync::Weak<ManagedGenerationPublisher>,
    ) -> Result<()> {
        self.generation_publisher
            .set(publisher)
            .map_err(|_| anyhow::anyhow!("managed generation publisher already attached"))?;
        self.start_reclamation();
        Ok(())
    }

    pub(crate) async fn current_snapshots(&self) -> BTreeMap<Uuid, Arc<ManagedWorkspaceSnapshot>> {
        self.snapshots.lock().await.current.clone()
    }

    /// Complete startup graphs after the interface catalogue is attached, before ingress is pinned.
    #[cfg_attr(test, api_test_selection::group(contracts))]
    #[cfg(test)]
    pub(crate) async fn refresh_boot_snapshots(self: &Arc<Self>) -> Result<()> {
        anyhow::ensure!(
            self.generation_publisher.get().is_none() && self.interface_module.get().is_some(),
            "managed boot refresh requires an attached interface catalogue and unpublished ingress"
        );
        let snapshots = self.current_snapshots().await;
        anyhow::ensure!(
            snapshots
                .values()
                .all(|snapshot| !snapshot.bindings.is_empty()),
            "managed boot snapshot has no installation binding to refresh"
        );
        let workspaces = snapshots
            .keys()
            .copied()
            .collect::<std::collections::BTreeSet<_>>();
        drop(snapshots);
        let operation = self
            .operations
            .admit(control_plane_contracts::ports::ManagedOwnedOperation::Candidate)?;
        let owner = self.clone();
        tokio::spawn(async move {
            let _operation = operation;
            owner
                .rebuild_workspaces_owned_with_event_catalog_and_pin(workspaces, true, None, None)
                .await
                .map(|_| ())
        })
        .await
        .context("managed boot refresh owner terminated")?
    }

    /// Complete startup graphs, synchronize the boot event catalog inside the open authority
    /// transaction, publish `visible.current` before releasing database locks, and pin the boot
    /// snapshot only after the transaction commits.
    pub(crate) async fn refresh_boot_snapshots_with_event_catalog_and_pin<B, P>(
        self: &Arc<Self>,
        build_event_catalog: B,
        pin_after_commit: P,
    ) -> Result<BTreeMap<Uuid, Arc<ManagedWorkspaceSnapshot>>>
    where
        B: FnOnce(
                BTreeMap<Uuid, Arc<ManagedWorkspaceSnapshot>>,
            ) -> std::pin::Pin<
                Box<
                    dyn std::future::Future<
                            Output = Result<domain::event_center::EventCatalogSnapshot>,
                        > + Send,
                >,
            > + Send
            + 'static,
        P: FnOnce(&BTreeMap<Uuid, Arc<ManagedWorkspaceSnapshot>>) -> Result<()> + Send + 'static,
    {
        anyhow::ensure!(
            self.generation_publisher.get().is_none() && self.interface_module.get().is_some(),
            "managed boot refresh requires an attached interface catalogue and unpublished ingress"
        );
        let snapshots = self.current_snapshots().await;
        anyhow::ensure!(
            snapshots
                .values()
                .all(|snapshot| !snapshot.bindings.is_empty()),
            "managed boot snapshot has no installation binding to refresh"
        );
        let workspaces = snapshots
            .keys()
            .copied()
            .collect::<std::collections::BTreeSet<_>>();
        drop(snapshots);
        let operation = self
            .operations
            .admit(control_plane_contracts::ports::ManagedOwnedOperation::Candidate)?;
        let owner = self.clone();
        tokio::spawn(async move {
            let _operation = operation;
            owner
                .rebuild_workspaces_owned_with_event_catalog_and_pin(
                    workspaces,
                    true,
                    Some(Box::new(build_event_catalog)),
                    Some(Box::new(pin_after_commit)),
                )
                .await
        })
        .await
        .context("managed boot refresh owner terminated")?
    }

    pub(super) fn publisher(&self) -> Option<Arc<ManagedGenerationPublisher>> {
        self.generation_publisher
            .get()
            .and_then(std::sync::Weak::upgrade)
    }

    pub(super) async fn candidate_services(
        &self,
        system_packages: Option<&[PreparedPackage]>,
    ) -> Result<Vec<ManagedServiceRegistration>> {
        let installations = match system_packages {
            Some(packages) => packages
                .iter()
                .map(|p| (p.installation.id, p.installation.clone()))
                .collect::<BTreeMap<_, _>>(),
            None => self
                .snapshot(domain::SYSTEM_SCOPE_ID)
                .await
                .map(|snapshot| {
                    snapshot
                        .bindings
                        .values()
                        .map(|binding| (binding.installation.id, binding.installation.clone()))
                        .collect()
                })
                .unwrap_or_default(),
        };
        crate::managed_services::load_installations(
            &self.store,
            &self.node_id,
            &installations.into_values().collect::<Vec<_>>(),
        )
        .await
    }

    pub(super) async fn prepare_host_generation(
        &self,
        registrations: Vec<ManagedServiceRegistration>,
        snapshots: BTreeMap<Uuid, Arc<ManagedWorkspaceSnapshot>>,
    ) -> Result<Option<(Arc<ManagedGenerationPublisher>, PreparedGeneration)>> {
        match self.publisher() {
            Some(publisher) => {
                let prepared = publisher.prepare(registrations, snapshots).await?;
                Ok(Some((publisher, prepared)))
            }
            None => Ok(None), // Boot assembly and standalone execution fixtures have no ingress yet.
        }
    }
}
