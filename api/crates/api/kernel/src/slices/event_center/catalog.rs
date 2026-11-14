use domain::{event_center::*, ActorContext};
use std::sync::Arc;
pub struct EventSourceCatalog {
    pub runtime_bound: bool,
    pub native_plan: Option<Arc<plugin_framework::extension_bus::EffectiveLifecycleSubscriberPlan>>,
    pub lifecycle_observer:
        Option<Arc<control_plane::lifecycle_outbox_dispatcher::LifecycleSourceObserver>>,
    pub projection: Arc<dyn EventSourceCatalogPort>,
}
pub trait EventSourceCatalogPort: Send + Sync {
    fn catalog_entries(
        &self,
        catalog: &EventSourceCatalog,
        actor: &ActorContext,
        runtime: &control_plane::event_center::EventCenterRuntime,
        registry: Option<&interface_runtime::CompiledInterfaceRegistry>,
        runtime_instances: bool,
    ) -> Vec<EventCatalogEntry>;
}
impl EventSourceCatalog {
    pub fn registered_events(
        &self,
        actor: &ActorContext,
        runtime: &control_plane::event_center::EventCenterRuntime,
    ) -> Vec<EventCatalogEntry> {
        self.registered_events_with_interfaces(actor, runtime, None)
    }
    pub fn registered_events_with_interfaces(
        &self,
        actor: &ActorContext,
        runtime: &control_plane::event_center::EventCenterRuntime,
        registry: Option<&interface_runtime::CompiledInterfaceRegistry>,
    ) -> Vec<EventCatalogEntry> {
        self.projection
            .catalog_entries(self, actor, runtime, registry, true)
    }
    pub fn declared_events_with_interfaces(
        &self,
        actor: &ActorContext,
        runtime: &control_plane::event_center::EventCenterRuntime,
        registry: Option<&interface_runtime::CompiledInterfaceRegistry>,
    ) -> Vec<EventCatalogEntry> {
        self.projection
            .catalog_entries(self, actor, runtime, registry, false)
    }
}
