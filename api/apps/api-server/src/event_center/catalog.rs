use domain::{event_center::*, ActorContext};
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
    /// Opt an isolated fixture into canonical/runtime capabilities without exposing
    /// mutable composition evidence through the public library facade.
    #[cfg(any(test, feature = "test-support"))]
    pub fn for_test_support() -> Self {
        Self {
            runtime_bound: true,
            ..Self::default()
        }
    }

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
        let business = super::business::BusinessSourceObserver::new(runtime);
        let store = store
            .with_lifecycle_observer(lifecycle.clone())
            .with_application_lifecycle_observer(business.clone())
            .with_communication_lifecycle_observer(business);
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
        actor: &ActorContext,
        runtime: &control_plane::event_center::EventCenterRuntime,
    ) -> Vec<EventCatalogEntry> {
        self.registered_events_with_interfaces(actor, runtime, None)
    }

    pub(crate) fn registered_events_with_interfaces(
        &self,
        actor: &ActorContext,
        runtime: &control_plane::event_center::EventCenterRuntime,
        registry: Option<&interface_runtime::CompiledInterfaceRegistry>,
    ) -> Vec<EventCatalogEntry> {
        self.catalog_entries(actor, runtime, registry, true)
    }

    pub(crate) fn declared_events_with_interfaces(
        &self,
        actor: &ActorContext,
        runtime: &control_plane::event_center::EventCenterRuntime,
        registry: Option<&interface_runtime::CompiledInterfaceRegistry>,
    ) -> Vec<EventCatalogEntry> {
        self.catalog_entries(actor, runtime, registry, false)
    }

    fn catalog_entries(
        &self,
        actor: &ActorContext,
        runtime: &control_plane::event_center::EventCenterRuntime,
        registry: Option<&interface_runtime::CompiledInterfaceRegistry>,
        runtime_instances: bool,
    ) -> Vec<EventCatalogEntry> {
        let scopes = visible_scopes(actor);
        let configured = runtime.configured();
        let mut entries = BTreeMap::new();
        if !runtime_instances || (configured && self.runtime_bound) {
            if let Some(registry) = registry {
                for entry in super::interface::catalog(registry, actor) {
                    insert(&mut entries, entry);
                }
            }
        }
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
            if !runtime_instances || (configured && self.runtime_bound) {
                for entry in super::business::catalog(scope.clone()) {
                    insert(&mut entries, entry);
                }
            }
            if let Some(plan) = &self.native_plan {
                for mut entry in crate::host_extensions::lifecycle::native_lifecycle_catalog_entries(
                    plan,
                    scope.clone(),
                ) {
                    if !configured || self.lifecycle_observer.is_none() {
                        entry.supported_phases.clear();
                    }
                    insert(&mut entries, entry);
                }
            }
        }
        // Actual scoped registrations contribute capabilities to the same definition.
        // Historical Unbound queue owners remain exclusively in the runtime projection.
        if runtime_instances {
            for scope in scopes {
                for source in runtime.snapshot_sources(scope).sources {
                    insert(&mut entries, source.descriptor.into());
                }
            }
        }
        entries
            .into_values()
            .filter(|entry| !runtime_instances || !entry.supported_phases.is_empty())
            .map(|mut entry| {
                if !configured || !self.runtime_bound {
                    entry.enabled = Some(false);
                    entry.supported_phases.clear();
                }
                if entry.group_path.is_empty() {
                    entry.group_path = if entry.contract_id.starts_with("runtime-event-") {
                        vec!["runtime".into()]
                    } else if entry.contract_id.starts_with("credit.") {
                        vec!["credits".into()]
                    } else if entry.contract_id.starts_with("model_definition.") {
                        vec!["data_models".into()]
                    } else {
                        vec!["plugins".into(), entry.owner_id.clone().unwrap_or_default()]
                    };
                }
                entry
            })
            .collect()
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
type Key = (EventSourceScope, String, BTreeMap<String, String>);
fn insert(entries: &mut BTreeMap<Key, EventCatalogEntry>, entry: EventCatalogEntry) {
    if entry.scope == EventSourceScope::Unbound {
        return;
    }
    let key = (
        entry.scope.clone(),
        entry.contract_id.clone(),
        entry
            .source
            .as_ref()
            .map(|s| s.selection.filters.clone())
            .unwrap_or_default(),
    );
    if let Some(existing) = entries.get_mut(&key) {
        // Registration validates current schema and defining owner. Multiple runtime leases
        // contribute capability phases, rather than duplicate definition rows.
        if existing.supported_phases.is_empty() {
            return;
        }
        if existing.owner_id != entry.owner_id
            || existing.payload_schema != entry.payload_schema
            || existing.origin != entry.origin
            || existing.source != entry.source
        {
            tracing::error!(contract_id = %entry.contract_id, "conflicting event catalog definition");
            existing.supported_phases.clear();
            return;
        }
        for phase in entry.supported_phases {
            if !existing.supported_phases.contains(&phase) {
                existing.supported_phases.push(phase);
            }
        }
    } else {
        entries.insert(key, entry);
    }
}

#[cfg_attr(test, api_test_selection::group(contracts))]
#[cfg(test)]
#[path = "_tests/catalog.rs"]
mod tests;
