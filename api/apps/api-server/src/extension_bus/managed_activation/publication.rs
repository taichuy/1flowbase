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
