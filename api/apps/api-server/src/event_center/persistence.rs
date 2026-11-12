//! Source declaration snapshots are assembled at lifecycle boundaries, never on catalog reads.
use crate::{
    app_state::ApiState,
    extension_bus::{ManagedExtensionComposition, ManagedWorkspaceSnapshot},
};
use control_plane_contracts::ports::EventCatalogRepository;
use domain::{event_center::*, ActorContext};
use std::{collections::BTreeMap, sync::Arc};
use uuid::Uuid;

pub(crate) async fn snapshot(
    state: &ApiState,
    managed: &BTreeMap<Uuid, Arc<ManagedWorkspaceSnapshot>>,
) -> anyhow::Result<EventCatalogSnapshot> {
    let registry = state
        .extension_boot_snapshot
        .as_ref()
        .and_then(|boot| boot.interface_registry())
        .map(|registry| registry.snapshot());
    let mut scopes = std::collections::BTreeSet::new();
    let mut entries = BTreeMap::new();
    for workspace in state.store.event_catalog_workspaces().await? {
        let actor = ActorContext::root_in_scope(
            Uuid::nil(),
            workspace.tenant_id,
            workspace.workspace_id,
            "root",
        );
        scopes.extend(super::catalog::visible_scopes(&actor));
        let mut declared = state.event_sources.declared_events_with_interfaces(
            &actor,
            &state.event_center,
            registry.as_deref(),
        );
        if let Some(managed) = managed.get(&workspace.workspace_id) {
            declared.extend(ManagedExtensionComposition::event_catalog_declarations(
                workspace.workspace_id,
                managed,
            ));
        }
        for entry in declared {
            let key = serde_json::to_string(&(
                &entry.scope,
                &entry.contract_id,
                entry
                    .source
                    .as_ref()
                    .map(|source| &source.selection.source_id),
                entry
                    .source
                    .as_ref()
                    .map(|source| &source.selection.filters),
            ))?;
            if let Some(existing) = entries.get(&key) {
                anyhow::ensure!(
                    existing == &entry,
                    "conflicting event declaration {}",
                    entry.contract_id
                );
            } else {
                entries.insert(key, entry);
            }
        }
    }
    let snapshot = EventCatalogSnapshot {
        scopes: scopes.into_iter().collect(),
        entries: entries.into_values().collect(),
    };
    snapshot.validate().map_err(anyhow::Error::msg)?;
    Ok(snapshot)
}
