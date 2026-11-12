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
        name: "Order interface observed".into(),
        description: "Event configuration fixture".into(),
        interface_event: InterfaceEventBinding::legacy(
            "orders.create",
            InterfaceEventPhase::Before,
        ),
        enabled: true,
    }
}

// The declared owner contract is separate from user rule input. Rules inherit it verbatim.
fn declared_service<R: EventCenterRepository>(
    repository: R,
    actor: &ActorContext,
) -> EventCenterConfigurationService<R> {
    let entry = EventCatalogEntry {
        created_at: None,
        updated_at: None,
        interface_event: Some(input().interface_event),
        group_path: vec!["interfaces".into(), "orders".into()],
        source: None,
        id: None,
        scope: EventSourceScope::Workspace {
            workspace_id: actor.current_workspace_id,
        },
        enabled: None,
        owner_id: Some("orders".into()),
        contract_id: "interface.orders.create.before".into(),
        name: "Order create Before".into(),
        description: String::new(),
        payload_schema: interface_event_schema(
            InterfaceEventPhase::Before,
            json!({"type":"object","properties":{"order_id":{"type":"string"}},"required":["order_id"],"additionalProperties":false}),
            json!({"type":"object"}),
        ),
        origin: EventDefinitionOrigin::Core,
        supported_phases: vec![EventTriggerPhase::Published],
    };
    EventCenterConfigurationService::with_registered_events(
        repository,
        actor.current_workspace_id,
        vec![entry],
    )
}

fn trigger(contract_id: &str) -> WorkflowEventTriggerInput {
    WorkflowEventTriggerInput {
        source: None,
        enabled: false,
        contract_id: contract_id.into(),
        trigger_phase: EventTriggerPhase::Published,
        input_mapping: BTreeMap::from([("order_id".into(), "/payload/input/order_id".into())]),
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
    let service = declared_service(store.clone(), &owner);
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
    assert!(created.contract_id.starts_with("custom.interface."));
    assert_eq!(persisted.interface_event, Some(input().interface_event));
    assert_eq!(
        persisted.payload_schema["properties"]["input"]["properties"]["order_id"]["type"],
        "string"
    );
    let update = UpdateEventDefinitionInput {
        name: "Revised order".into(),
        description: "Updated".into(),
        interface_event: input().interface_event,
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
    // Repeated rules for one capability receive distinct identities. The repository still
    // rejects duplicate persisted identities and namespaces reserved for declared owners.
    let second = service.create_definition(&owner, input()).await.unwrap();
    assert_ne!(second.id, created.id);
    assert_ne!(second.contract_id, created.contract_id);
    let mut duplicate = created.clone();
    duplicate.id = Uuid::now_v7();
    let duplicate_error = store.create_event_definition(&duplicate).await.unwrap_err();
    assert_eq!(
        duplicate_error.downcast_ref::<ControlPlaneContractError>(),
        Some(&ControlPlaneContractError::Conflict(
            "event_definition_identity"
        ))
    );
    assert!(declared_service(store.clone(), &other)
        .create_definition(&other, input())
        .await
        .is_ok());
    let mut unavailable = input();
    unavailable.interface_event =
        InterfaceEventBinding::legacy("orders.undeclared", InterfaceEventPhase::Before);
    assert!(service
        .create_definition(&owner, unavailable)
        .await
        .is_err());
    for identity in ["1flowbase.orders", "acme.plugin.orders", "custom..orders"] {
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
    let service = declared_service(store.clone(), &owner);
    let definition = service.create_definition(&owner, input()).await.unwrap();
    let other_definition = declared_service(store.clone(), &other)
        .create_definition(&other, input())
        .await
        .unwrap();
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
        .save_workflow_trigger(&owner, application.id, trigger(&definition.contract_id))
        .await
        .unwrap();
    let reloaded = EventCenterConfigurationService::new(store.clone())
        .get_workflow_trigger(&owner, application.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(reloaded.configuration, trigger(&definition.contract_id));
    assert_eq!(reloaded.id, first.id);
    assert_eq!(reloaded.workspace_id, workspace.id);
    assert_eq!(reloaded.updated_by, owner.user_id);
    let mut revised = trigger(&definition.contract_id);
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
    // Other scope has its own available rule: rejection must come from application
    // ownership, rather than accidentally fail early in catalog lookup.
    assert!(service
        .save_workflow_trigger(
            &other,
            application.id,
            trigger(&other_definition.contract_id)
        )
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
    assert_eq!(
        store
            .list_workflow_event_triggers(workspace.id)
            .await
            .unwrap()
            .len(),
        1,
        "legacy disabled configurations remain discoverable for publication-driven activation"
    );
    assert!(store
        .list_workflow_event_trigger_workspaces()
        .await
        .unwrap()
        .contains(&workspace.id));
}
