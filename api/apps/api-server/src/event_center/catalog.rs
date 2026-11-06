use domain::{event_center::*, ActorContext};
use extension_contracts::extension_bus::{EffectiveExtensionGraph, ExtensionPointKind, ModuleKind};
use std::{collections::BTreeMap, sync::Arc};

/// Composition-root capability evidence. Lightweight states default to unavailable adapters.
#[derive(Default)]
pub struct EventSourceCatalog {
    pub(crate) runtime_bound: bool,
    pub(crate) native_plan:
        Option<Arc<plugin_framework::extension_bus::EffectiveLifecycleSubscriberPlan>>,
    pub(crate) lifecycle_observer:
        Option<Arc<control_plane::lifecycle_outbox_dispatcher::LifecycleSourceObserver>>,
}
impl EventSourceCatalog {
    /// Shared production/fixture assembly. Call before extracting stream/store clones or
    /// starting producers/workers; keep the returned catalog with the serving generation.
    pub(crate) fn attach_local(
        infrastructure: &mut crate::host_infrastructure::HostInfrastructureRegistry,
        store: storage_durable_postgres::MainDurableStore,
        runtime: &control_plane::event_center::EventCenterRuntime,
        native_plan: Option<Arc<plugin_framework::extension_bus::EffectiveLifecycleSubscriberPlan>>,
    ) -> anyhow::Result<(storage_durable_postgres::MainDurableStore, Arc<Self>)> {
        let observer: Arc<dyn control_plane_contracts::ports::EventSourceObserver> =
            Arc::new(runtime.clone());
        crate::host_infrastructure::attach_local_event_source_observer(
            infrastructure,
            observer.clone(),
        )?;
        let lifecycle = Arc::new(
            control_plane::lifecycle_outbox_dispatcher::LifecycleSourceObserver::new(observer),
        );
        if let Some(plan) = &native_plan {
            crate::host_extensions::lifecycle::register_native_lifecycle_sources(
                &lifecycle,
                plan,
                EventSourceScope::Unbound,
            );
        }
        let store = store.with_lifecycle_observer(lifecycle.clone());
        Ok((
            store,
            Arc::new(Self {
                runtime_bound: true,
                native_plan,
                lifecycle_observer: Some(lifecycle),
            }),
        ))
    }

    pub(crate) fn registered_events(
        &self,
        boot: Option<&crate::extension_bus::ExtensionBootSnapshot>,
        actor: &ActorContext,
        runtime: &control_plane::event_center::EventCenterRuntime,
    ) -> Vec<EventCatalogEntry> {
        let scopes = visible_scopes(actor);
        let configured = runtime.snapshot(actor.current_workspace_id).configured;
        let mut entries = BTreeMap::new();
        // Definitions/capabilities exist before the first execution. They do not create leases,
        // consumers or Subscribed observations. Live run instances remain their own owners.
        for scope in &scopes {
            if *scope == EventSourceScope::Unbound {
                continue;
            }
            for mut source in control_plane::ports::runtime_event_source_descriptors(
                scope.clone(),
                EventSourceBinding::Active,
            ) {
                if !configured || !self.runtime_bound {
                    unavailable(&mut source);
                }
                insert(&mut entries, source.into());
            }
            if let Some(plan) = &self.native_plan {
                for mut source in
                    crate::host_extensions::lifecycle::native_lifecycle_source_descriptors(
                        plan,
                        scope.clone(),
                    )
                {
                    if !configured || self.lifecycle_observer.is_none() {
                        unavailable(&mut source);
                    }
                    insert(&mut entries, source.into());
                }
            }
        }
        // Actual registrations, including legacy Unbound queue owners, precede graph
        // fallback so a declaration cannot add a second placeholder for the same point.
        for scope in scopes {
            for source in runtime.snapshot_sources(scope).sources {
                insert(&mut entries, source.descriptor.into());
            }
        }
        if let Some(boot) = boot {
            project_graph(boot.graph(), actor, &mut entries);
            if let Some(managed) = boot.managed_snapshot(actor.current_workspace_id) {
                project_graph(&managed.graph, actor, &mut entries);
            }
        }
        entries.into_values().collect()
    }
}
pub(crate) fn visible_scopes(actor: &ActorContext) -> Vec<EventSourceScope> {
    let mut scopes = vec![EventSourceScope::Workspace {
        workspace_id: actor.current_workspace_id,
    }];
    if actor.is_root {
        scopes.extend([EventSourceScope::System, EventSourceScope::Unbound]);
    }
    scopes
}
fn unavailable(source: &mut EventSourceDescriptor) {
    source.binding = EventSourceBinding::Declaration;
    source.coverage = EventSourceCoverage::Unavailable;
    source.supported_phases.clear();
}
type Key = (EventSourceSelection, String, String);
fn insert(entries: &mut BTreeMap<Key, EventCatalogEntry>, entry: EventCatalogEntry) {
    if let Some(source) = &entry.source {
        entries.insert(
            (
                source.selection.clone(),
                entry.contract_id.clone(),
                entry.contract_version.clone(),
            ),
            entry,
        );
    }
}
fn project_graph(
    graph: &EffectiveExtensionGraph,
    actor: &ActorContext,
    entries: &mut BTreeMap<Key, EventCatalogEntry>,
) {
    for point in graph.points() {
        let descriptor = point.descriptor();
        if descriptor.point_kind != ExtensionPointKind::EventStream {
            continue;
        }
        if entries.values().any(|entry| {
            entry.contract_id == descriptor.contract.contract_id.as_str()
                && entry.contract_version == descriptor.contract.contract_version.as_str()
                && entry
                    .source
                    .as_ref()
                    .is_some_and(|source| source.point_id == descriptor.point_id.as_str())
        }) {
            continue;
        }
        let contract_id = descriptor.contract.contract_id.as_str().to_owned();
        // A graph declaration by itself has no trusted publication scope or executable adapter.
        // Unbound definitions are visible only in the explicit root projection.
        if !actor.is_root {
            continue;
        }
        insert(
            entries,
            EventCatalogEntry {
                id: None,
                enabled: false,
                owner_id: descriptor.owner_module_id.as_str().into(),
                name: contract_id.clone(),
                description: descriptor.point_id.as_str().into(),
                contract_id,
                contract_version: descriptor.contract.contract_version.as_str().into(),
                payload_schema: descriptor
                    .contract
                    .payload_schema
                    .clone()
                    .unwrap_or(serde_json::Value::Null),
                origin: if point.provenance().module_kind() == ModuleKind::BootCore {
                    EventDefinitionOrigin::Core
                } else {
                    EventDefinitionOrigin::Plugin
                },
                supported_phases: Vec::new(),
                source: Some(EventCatalogSource {
                    selection: EventSourceSelection {
                        source_id: format!("declaration:{}", descriptor.point_id.as_str()),
                        scope: EventSourceScope::Unbound,
                    },
                    point_id: descriptor.point_id.as_str().into(),
                    binding: EventSourceBinding::Declaration,
                    coverage: EventSourceCoverage::Unavailable,
                }),
            },
        );
    }
}

#[cfg_attr(test, api_test_selection::group(contracts))]
#[cfg(test)]
#[path = "_tests/catalog.rs"]
mod tests;
