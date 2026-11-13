use super::*;
use control_plane_contracts::ports::{
    ApplicationRepository, CreateApplicationInput, EventCenterRepository,
};
use domain::event_center::*;
use serde_json::json;
use std::collections::BTreeMap;
use time::OffsetDateTime;

#[test]
fn event_configuration_has_distinct_explicit_settings_owners() {
    assert_eq!(
        inventory::static_owner("event_definitions"),
        Some(("system.events".into(), false))
    );
    assert_eq!(
        inventory::static_owner("workflow_event_triggers"),
        Some(("system.applications".into(), false))
    );
}

#[tokio::test]
async fn selective_event_and_application_configuration_roundtrip_without_cross_owner_rows() {
    let (db, actor) = fixture().await;
    let store = crate::PgControlPlaneStore::new(db.clone());
    let tenant = store.upsert_root_tenant().await.unwrap();
    let workspace = store
        .upsert_workspace(tenant.id, "Event backup scope")
        .await
        .unwrap();
    let now = OffsetDateTime::now_utc();
    let mut definition = EventDefinitionRecord {
        event_kind: None,
        interface_event: None,

        id: Uuid::now_v7(),
        workspace_id: workspace.id,
        contract_id: "custom.backup.orders".into(),
        name: "Original event".into(),
        description: "Configuration only".into(),
        payload_schema: json!({"type":"object"}),
        enabled: true,
        created_by: actor,
        created_at: now,
        updated_at: now,
    };
    store.create_event_definition(&definition).await.unwrap();
    let application = store
        .create_application(&CreateApplicationInput {
            actor_user_id: actor,
            workspace_id: workspace.id,
            application_type: domain::ApplicationType::Workflow,
            workflow_trigger_type: Some(domain::WorkflowTriggerType::Event),
            workflow_trigger_config: None,
            name: "Event backup workflow".into(),
            description: String::new(),
            icon: None,
            icon_type: None,
            icon_background: None,
        })
        .await
        .unwrap();
    let original_configuration = WorkflowEventTriggerInput {
        source: None,
        enabled: false,
        contract_id: definition.contract_id.clone(),
        trigger_phase: EventTriggerPhase::Published,
        input_mapping: BTreeMap::from([("input".into(), "/payload".into())]),
    };
    let mut trigger = WorkflowEventTriggerRecord {
        id: Uuid::now_v7(),
        workspace_id: workspace.id,
        application_id: application.id,
        configuration: original_configuration.clone(),
        updated_by: actor,
        updated_at: now,
    };
    assert!(store.save_workflow_event_trigger(&trigger).await.unwrap());
    let repository = PgSelectiveBackupRepository::new(db.clone());
    let catalog = repository.catalog().await.unwrap();
    let events = catalog
        .iter()
        .find(|category| category.feature_id == "system.events")
        .unwrap();
    assert_eq!(
        events.structure_tables,
        vec!["event_definitions".to_string()]
    );
    assert!(events.data_tables.is_empty());
    let applications = catalog
        .iter()
        .find(|category| category.feature_id == "system.applications")
        .unwrap();
    assert!(applications
        .structure_tables
        .contains(&"workflow_event_triggers".into()));
    assert!(!applications
        .structure_tables
        .contains(&"event_definitions".into()));
    let event_backup = capture(&repository, select("events", true, false)).await;
    let application_backup = capture(&repository, select("applications", true, false)).await;
    let event_preview = repository
        .preflight(reader(event_backup.clone()), "source", "target")
        .await
        .unwrap();
    assert!(
        event_preview.failures.is_empty(),
        "{:?}",
        event_preview.failures
    );
    assert_eq!(
        event_preview.selected_tables,
        vec!["event_definitions".to_string()]
    );
    let application_preview = repository
        .preflight(reader(application_backup.clone()), "source", "target")
        .await
        .unwrap();
    assert!(
        application_preview.failures.is_empty(),
        "{:?}",
        application_preview.failures
    );
    assert!(application_preview
        .selected_tables
        .contains(&"workflow_event_triggers".into()));
    assert!(!application_preview
        .selected_tables
        .contains(&"event_definitions".into()));
    assert!(!event_preview
        .selected_tables
        .contains(&"applications".into()));
    assert!(!event_preview.selected_tables.contains(&"workspaces".into()));
    definition.name = "Changed event".into();
    store.update_event_definition(&definition).await.unwrap();
    trigger.configuration.trigger_phase = EventTriggerPhase::Consumed;
    trigger.configuration.input_mapping =
        BTreeMap::from([("source".into(), "/source_event_id".into())]);
    store.save_workflow_event_trigger(&trigger).await.unwrap();
    repository
        .restore(reader(event_backup), "source", "target", true)
        .await
        .unwrap();
    assert_eq!(
        store
            .get_event_definition(workspace.id, definition.id)
            .await
            .unwrap()
            .unwrap()
            .name,
        "Original event"
    );
    assert_eq!(
        store
            .get_workflow_event_trigger(workspace.id, application.id)
            .await
            .unwrap()
            .unwrap()
            .configuration,
        trigger.configuration
    );
    definition.name = "Independent event owner".into();
    store.update_event_definition(&definition).await.unwrap();
    repository
        .restore(reader(application_backup), "source", "target", true)
        .await
        .unwrap();
    assert_eq!(
        store
            .get_workflow_event_trigger(workspace.id, application.id)
            .await
            .unwrap()
            .unwrap()
            .configuration,
        original_configuration
    );
    assert_eq!(
        store
            .get_event_definition(workspace.id, definition.id)
            .await
            .unwrap()
            .unwrap()
            .name,
        "Independent event owner"
    );
}
