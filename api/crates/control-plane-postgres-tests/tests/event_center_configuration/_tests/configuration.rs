use control_plane::event_center::EventCenterConfigurationService;
use control_plane_contracts::{
    ports::{ApplicationRepository, CreateApplicationInput, EventCenterRepository},
    ControlPlaneContractError,
};
use domain::{event_center::*, ActorContext};
use serde_json::json;
use std::collections::BTreeMap;
use uuid::Uuid;

fn input() -> CreateEventDefinitionInput {
    CreateEventDefinitionInput {
        contract_id: "custom.orders.created".into(),
        name: "Order created".into(),
        description: "Event configuration fixture".into(),
        payload_schema: json!({"type":"object","properties":{"order_id":{"type":"string"}}}),
        enabled: true,
    }
}
fn trigger() -> WorkflowEventTriggerInput {
    WorkflowEventTriggerInput {
        source: None,
        enabled: false,
        contract_id: input().contract_id,
        trigger_phase: EventTriggerPhase::Published,
        input_mapping: BTreeMap::from([("order_id".into(), "/payload/order_id".into())]),
    }
}

#[tokio::test]
async fn definitions_roundtrip_with_workspace_scoped_identity_and_namespace_constraints() {
    let (store, workspace, user) = super::support::seed_store().await;
    let owner = ActorContext::root_in_scope(user.id, workspace.tenant_id, workspace.id, "root");
    let other_workspace = store
        .upsert_workspace(workspace.tenant_id, "Other event scope")
        .await
        .unwrap();
    let other = ActorContext {
        current_workspace_id: other_workspace.id,
        ..owner.clone()
    };
    let service = EventCenterConfigurationService::new(store.clone());
    let created = service.create_definition(&owner, input()).await.unwrap();
    let persisted = EventCenterConfigurationService::new(store.clone())
        .get_definition(&owner, created.id)
        .await
        .unwrap();
    assert_eq!(persisted.contract_id, created.contract_id);
    assert_eq!(persisted.payload_schema, created.payload_schema);
    assert_eq!(persisted.created_by, owner.user_id);
    assert!(service.get_definition(&other, created.id).await.is_err());
    assert!(service.list_definitions(&other).await.unwrap().is_empty());
    let update = UpdateEventDefinitionInput {
        name: "Revised order".into(),
        description: "Updated".into(),
        payload_schema: json!({"type":"object","required":["order_id"],"properties":{"order_id":{"type":"string"}}}),
        enabled: true,
    };
    assert!(service
        .update_definition(&other, created.id, update.clone())
        .await
        .is_err());
    service
        .update_definition(&owner, created.id, update)
        .await
        .unwrap();
    assert_eq!(
        service
            .get_definition(&owner, created.id)
            .await
            .unwrap()
            .name,
        "Revised order"
    );
    let duplicate = service
        .create_definition(&owner, input())
        .await
        .unwrap_err();
    assert_eq!(
        duplicate.downcast_ref::<ControlPlaneContractError>(),
        Some(&ControlPlaneContractError::Conflict(
            "event_definition_identity"
        ))
    );
    assert!(service.create_definition(&other, input()).await.is_ok());
    for identity in ["1flowbase.orders", "acme.plugin.orders", "custom..orders"] {
        let mut invalid = input();
        invalid.contract_id = identity.into();
        assert!(service.create_definition(&owner, invalid).await.is_err());
        // Exercise the formal database constraint as well as the service boundary.
        let mut forged = created.clone();
        forged.id = Uuid::now_v7();
        forged.contract_id = identity.into();
        assert!(store.create_event_definition(&forged).await.is_err());
    }
    let mut forged_update = created.clone();
    forged_update.workspace_id = other_workspace.id;
    forged_update.name = "Cross-scope overwrite".into();
    assert!(!store.update_event_definition(&forged_update).await.unwrap());
    assert_eq!(
        service
            .get_definition(&owner, created.id)
            .await
            .unwrap()
            .name,
        "Revised order"
    );
}

#[tokio::test]
async fn workflow_configuration_reloads_and_cannot_write_another_workspace_application() {
    let (store, workspace, user) = super::support::seed_store().await;
    let owner = ActorContext::root_in_scope(user.id, workspace.tenant_id, workspace.id, "root");
    let other_workspace = store
        .upsert_workspace(workspace.tenant_id, "Other workflow scope")
        .await
        .unwrap();
    let other = ActorContext {
        current_workspace_id: other_workspace.id,
        ..owner.clone()
    };
    let service = EventCenterConfigurationService::new(store.clone());
    service.create_definition(&owner, input()).await.unwrap();
    service.create_definition(&other, input()).await.unwrap();
    let application = store
        .create_application(&CreateApplicationInput {
            actor_user_id: owner.user_id,
            workspace_id: workspace.id,
            application_type: domain::ApplicationType::Workflow,
            workflow_trigger_type: Some(domain::WorkflowTriggerType::Event),
            workflow_trigger_config: None,
            name: "Event fixture".into(),
            description: String::new(),
            icon: None,
            icon_type: None,
            icon_background: None,
        })
        .await
        .unwrap();
    let first = service
        .save_workflow_trigger(&owner, application.id, trigger())
        .await
        .unwrap();
    let reloaded = EventCenterConfigurationService::new(store.clone())
        .get_workflow_trigger(&owner, application.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(reloaded.configuration, trigger());
    assert_eq!(reloaded.id, first.id);
    assert_eq!(reloaded.workspace_id, workspace.id);
    assert_eq!(reloaded.updated_by, owner.user_id);
    let mut revised = trigger();
    revised.source = Some(EventSourceSelection {
        filters: Default::default(),

        source_id: "native.available-later".into(),
        scope: EventSourceScope::Workspace {
            workspace_id: workspace.id,
        },
    });
    revised.trigger_phase = EventTriggerPhase::Consumed;
    revised.input_mapping = BTreeMap::from([("source".into(), "/source_event_id".into())]);
    let saved = service
        .save_workflow_trigger(&owner, application.id, revised.clone())
        .await
        .unwrap();
    assert_eq!(saved.id, first.id);
    assert_eq!(
        store
            .get_workflow_event_trigger(workspace.id, application.id)
            .await
            .unwrap()
            .unwrap()
            .configuration,
        revised
    );
    // Other scope has an available identically named event: rejection must come
    // from application ownership, rather than accidentally fail early in catalog lookup.
    assert!(service
        .save_workflow_trigger(&other, application.id, trigger())
        .await
        .is_err());
    assert!(service
        .get_workflow_trigger(&other, application.id)
        .await
        .unwrap()
        .is_none());
    let mut forged = saved.clone();
    forged.workspace_id = other_workspace.id;
    assert!(!store.save_workflow_event_trigger(&forged).await.unwrap());
    assert_eq!(
        store
            .get_workflow_event_trigger(workspace.id, application.id)
            .await
            .unwrap()
            .unwrap()
            .configuration,
        revised
    );
    assert!(store
        .list_enabled_workflow_event_triggers(workspace.id)
        .await
        .unwrap()
        .is_empty());
    assert!(store
        .list_workflow_event_trigger_workspaces()
        .await
        .unwrap()
        .contains(&workspace.id));
}
