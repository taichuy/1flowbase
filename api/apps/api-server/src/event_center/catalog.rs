use domain::{
    event_center::{EventCatalogEntry, EventDefinitionOrigin},
    ActorContext,
};
use extension_contracts::extension_bus::{EffectiveExtensionGraph, ExtensionPointKind, ModuleKind};
use std::collections::BTreeMap;

/// The pinned graph is the registration truth for this invocation and workspace.
/// A declaration alone does not open an adapter for ephemeral workflow triggers.
pub(crate) fn registered_events(
    boot: Option<&crate::extension_bus::ExtensionBootSnapshot>,
    actor: &ActorContext,
) -> Vec<EventCatalogEntry> {
    let Some(boot) = boot else {
        return Vec::new();
    };
    let mut entries = BTreeMap::new();
    project_graph(boot.graph(), &mut entries);
    if let Some(managed) = boot.managed_snapshot(actor.current_workspace_id) {
        project_graph(&managed.graph, &mut entries);
    }
    entries.into_values().collect()
}

fn project_graph(
    graph: &EffectiveExtensionGraph,
    entries: &mut BTreeMap<(String, String), EventCatalogEntry>,
) {
    for point in graph.points() {
        let descriptor = point.descriptor();
        if descriptor.point_kind != ExtensionPointKind::EventStream {
            continue;
        }
        let contract_id = descriptor.contract.contract_id.as_str().to_owned();
        let contract_version = descriptor.contract.contract_version.as_str().to_owned();
        entries.insert(
            (contract_id.clone(), contract_version.clone()),
            EventCatalogEntry {
                id: None,
                enabled: true,
                owner_id: descriptor.owner_module_id.as_str().to_owned(),
                name: contract_id.clone(),
                description: descriptor.point_id.as_str().to_owned(),
                contract_id,
                contract_version,
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
            },
        );
    }
}
