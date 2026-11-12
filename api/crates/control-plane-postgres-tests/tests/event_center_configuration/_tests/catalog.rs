use control_plane::event_center::EventCenterConfigurationService;
use control_plane_contracts::ports::{EventCatalogRepository, EventCenterRepository};
use domain::{event_center::*, ActorContext};
use serde_json::json;
use uuid::Uuid;

fn declaration(scope: EventSourceScope) -> EventCatalogEntry {
    EventSourceDescriptor {
        source_id: "fixture.source".into(),
        owner_id: "fixture.plugin".into(),
        point_id: "fixture.point".into(),
        contract_id: "fixture.fact".into(),
        name: "Fixture fact".into(),
        description: String::new(),
        payload_schema: json!({"type":"object"}),
        origin: EventDefinitionOrigin::Plugin,
        scope,
        binding: EventSourceBinding::Active,
        coverage: EventSourceCoverage::Live,
        supported_phases: vec![EventTriggerPhase::Published],
    }
    .into()
}

#[tokio::test]
async fn catalog_persists_declarations_preserves_restart_times_and_records_removal() {
    let (store, workspace, user) = super::support::seed_store().await;
    let actor = ActorContext::root_in_scope(user.id, workspace.tenant_id, workspace.id, "root");
    let scope = EventSourceScope::Workspace {
        workspace_id: workspace.id,
    };
    let mut snapshot = EventCatalogSnapshot {
        scopes: vec![scope.clone()],
        entries: vec![declaration(scope)],
    };
    store.synchronize_event_catalog(&snapshot).await.unwrap();
    // The read service has no registered-events input: its sole catalog is durable storage.
    let service = EventCenterConfigurationService::new(store.clone());
    let first = service.catalog(&actor).await.unwrap();
    assert_eq!(first.len(), 1);
    assert!(first[0].created_at.is_some());
    assert_eq!(first[0].created_at, first[0].updated_at);
    store.synchronize_event_catalog(&snapshot).await.unwrap();
    assert_eq!(service.catalog(&actor).await.unwrap(), first);

    snapshot.entries[0].description = "Upgraded schema owner".into();
    store.synchronize_event_catalog(&snapshot).await.unwrap();
    let upgraded = service.catalog(&actor).await.unwrap().remove(0);
    assert_eq!(upgraded.created_at, first[0].created_at);
    assert!(upgraded.updated_at > first[0].updated_at);
    assert_eq!(upgraded.description, "Upgraded schema owner");

    snapshot.entries.clear();
    store.synchronize_event_catalog(&snapshot).await.unwrap();
    let removed = service.catalog(&actor).await.unwrap().remove(0);
    assert_eq!(removed.created_at, first[0].created_at);
    assert_eq!(removed.enabled, Some(false));
    assert!(removed.supported_phases.is_empty());
    store.synchronize_event_catalog(&snapshot).await.unwrap();
    assert_eq!(service.catalog(&actor).await.unwrap()[0], removed);

    snapshot
        .entries
        .push(declaration(snapshot.scopes[0].clone()));
    store.synchronize_event_catalog(&snapshot).await.unwrap();
    let restored = service.catalog(&actor).await.unwrap().remove(0);
    assert_eq!(restored.created_at, first[0].created_at);
    assert_eq!(
        restored.supported_phases,
        vec![EventTriggerPhase::Published]
    );
    assert!(restored.updated_at > removed.updated_at);
}

#[tokio::test]
async fn custom_rules_share_the_directory_and_keep_original_timestamps_and_scope() {
    let (store, workspace, user) = super::support::seed_store().await;
    let actor = ActorContext::root_in_scope(user.id, workspace.tenant_id, workspace.id, "root");
    let now = time::OffsetDateTime::now_utc();
    let created = now - time::Duration::days(2);
    let rule = EventDefinitionRecord {
        id: Uuid::now_v7(),
        workspace_id: workspace.id,
        created_by: user.id,
        contract_id: "custom.fixture".into(),
        interface_event: None,
        name: "Custom rule".into(),
        description: String::new(),
        payload_schema: json!({"type":"object"}),
        enabled: true,
        created_at: created,
        updated_at: now,
    };
    store.create_event_definition(&rule).await.unwrap();
    let persisted = store
        .get_event_definition(workspace.id, rule.id)
        .await
        .unwrap()
        .unwrap();
    let service = EventCenterConfigurationService::new(store.clone());
    let entry = service.catalog(&actor).await.unwrap().remove(0);
    assert_eq!(entry.id, Some(rule.id));
    assert_eq!(entry.created_at, Some(persisted.created_at));
    assert_eq!(entry.updated_at, Some(persisted.updated_at));
    let scope = EventSourceScope::Workspace {
        workspace_id: workspace.id,
    };
    store
        .synchronize_event_catalog(&EventCatalogSnapshot {
            scopes: vec![scope],
            entries: vec![],
        })
        .await
        .unwrap();
    assert_eq!(service.catalog(&actor).await.unwrap()[0], entry);
    let outsider = ActorContext {
        is_root: false,
        current_workspace_id: Uuid::now_v7(),
        ..actor
    };
    assert!(service.catalog(&outsider).await.unwrap().is_empty());
}
