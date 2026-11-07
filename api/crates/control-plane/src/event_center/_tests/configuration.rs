use super::*;
use async_trait::async_trait;
use std::{collections::BTreeMap, sync::Mutex};

#[derive(Default)]
struct MemoryRepository {
    definitions: Mutex<Vec<EventDefinitionRecord>>,
    triggers: Mutex<Vec<WorkflowEventTriggerRecord>>,
}
#[async_trait]
impl EventCenterRepository for MemoryRepository {
    async fn list_workflow_event_trigger_workspaces(&self) -> anyhow::Result<Vec<Uuid>> {
        Ok(self
            .triggers
            .lock()
            .unwrap()
            .iter()
            .map(|row| row.workspace_id)
            .collect())
    }

    async fn list_event_definitions(
        &self,
        workspace: Uuid,
    ) -> anyhow::Result<Vec<EventDefinitionRecord>> {
        Ok(self
            .definitions
            .lock()
            .unwrap()
            .iter()
            .filter(|row| row.workspace_id == workspace)
            .cloned()
            .collect())
    }
    async fn get_event_definition(
        &self,
        workspace: Uuid,
        id: Uuid,
    ) -> anyhow::Result<Option<EventDefinitionRecord>> {
        Ok(self
            .definitions
            .lock()
            .unwrap()
            .iter()
            .find(|row| row.workspace_id == workspace && row.id == id)
            .cloned())
    }
    async fn create_event_definition(&self, row: &EventDefinitionRecord) -> anyhow::Result<()> {
        let mut rows = self.definitions.lock().unwrap();
        if rows.iter().any(|existing| {
            existing.workspace_id == row.workspace_id && existing.contract_id == row.contract_id
        }) {
            return Err(Error::Conflict("event_definition_identity").into());
        }
        rows.push(row.clone());
        Ok(())
    }
    async fn update_event_definition(&self, row: &EventDefinitionRecord) -> anyhow::Result<bool> {
        let mut rows = self.definitions.lock().unwrap();
        match rows
            .iter_mut()
            .find(|existing| existing.workspace_id == row.workspace_id && existing.id == row.id)
        {
            Some(existing) => {
                *existing = row.clone();
                Ok(true)
            }
            None => Ok(false),
        }
    }
    async fn get_workflow_event_trigger(
        &self,
        workspace: Uuid,
        application: Uuid,
    ) -> anyhow::Result<Option<WorkflowEventTriggerRecord>> {
        Ok(self
            .triggers
            .lock()
            .unwrap()
            .iter()
            .find(|row| row.workspace_id == workspace && row.application_id == application)
            .cloned())
    }
    async fn save_workflow_event_trigger(
        &self,
        row: &WorkflowEventTriggerRecord,
    ) -> anyhow::Result<bool> {
        self.triggers.lock().unwrap().push(row.clone());
        Ok(true)
    }
    async fn list_enabled_workflow_event_triggers(
        &self,
        workspace: Uuid,
    ) -> anyhow::Result<Vec<WorkflowEventTriggerRecord>> {
        Ok(self
            .triggers
            .lock()
            .unwrap()
            .iter()
            .filter(|row| row.workspace_id == workspace && row.configuration.enabled)
            .cloned()
            .collect())
    }
}
fn actor() -> ActorContext {
    ActorContext::root(Uuid::now_v7(), Uuid::now_v7(), "root")
}
fn input() -> CreateEventDefinitionInput {
    CreateEventDefinitionInput {
        name: "Order interface observed".into(),
        description: String::new(),
        enabled: true,
        interface_event: InterfaceEventBinding {
            interface_id: "orders.create".into(),
            phase: InterfaceEventPhase::Before,
            results: Vec::new(),
        },
    }
}
fn trigger_input() -> WorkflowEventTriggerInput {
    WorkflowEventTriggerInput {
        source: None,
        enabled: true,
        contract_id: "custom.legacy".into(),
        trigger_phase: EventTriggerPhase::Consumed,
        input_mapping: BTreeMap::new(),
    }
}
fn declared_service(owner: &ActorContext) -> EventCenterConfigurationService<MemoryRepository> {
    let mut entry = source_entry(
        EventSourceScope::Workspace {
            workspace_id: owner.current_workspace_id,
        },
        vec![EventTriggerPhase::Published],
    );
    entry.interface_event = Some(input().interface_event);
    entry.payload_schema = interface_event_schema(
        InterfaceEventPhase::Before,
        serde_json::json!({"type":"object","properties":{"order_id":{"type":"string"}},"required":["order_id"],"additionalProperties":false}),
        serde_json::json!({"type":"object"}),
    );
    EventCenterConfigurationService::with_registered_events(
        MemoryRepository::default(),
        owner.current_workspace_id,
        vec![entry],
    )
}
#[tokio::test]
async fn rules_inherit_declared_schema_and_keep_workspace_identity() {
    let owner = actor();
    let outsider = actor();
    let service = declared_service(&owner);
    let created = service.create_definition(&owner, input()).await.unwrap();
    assert!(created.contract_id.starts_with("custom.interface."));
    assert_eq!(created.interface_event, Some(input().interface_event));
    assert_eq!(
        created.payload_schema["properties"]["input"]["properties"]["order_id"]["type"],
        "string"
    );
    assert!(service.get_definition(&outsider, created.id).await.is_err());
    let updated = service
        .update_definition(
            &owner,
            created.id,
            UpdateEventDefinitionInput {
                name: "Renamed".into(),
                description: String::new(),
                interface_event: input().interface_event,
                enabled: false,
            },
        )
        .await
        .unwrap();
    assert_eq!(updated.contract_id, created.contract_id);
    assert_eq!(updated.enabled, false);
    assert_eq!(updated.payload_schema, created.payload_schema);
    let mut trigger = trigger_input();
    trigger.contract_id = created.contract_id;
    assert!(service
        .validate_workflow_trigger(&owner, &trigger)
        .await
        .is_err());
    trigger.enabled = false;
    assert!(service
        .save_workflow_trigger(&owner, Uuid::now_v7(), trigger)
        .await
        .is_ok());
}
#[tokio::test]
async fn absent_interface_and_invalid_before_result_filter_are_rejected() {
    let owner = actor();
    assert!(
        EventCenterConfigurationService::new(MemoryRepository::default())
            .create_definition(&owner, input())
            .await
            .is_err()
    );
    let service = declared_service(&owner);
    let mut value = input();
    value
        .interface_event
        .results
        .push(InterfaceEventTerminal::Failed);
    assert!(service.create_definition(&owner, value).await.is_err());
    let mut value = input();
    value.interface_event.interface_id = "undeclared".into();
    assert!(service.create_definition(&owner, value).await.is_err());
    let mut value = input();
    value.name = "  ".into();
    assert!(service.create_definition(&owner, value).await.is_err());
}
#[tokio::test]
async fn rule_catalog_is_editable_only_for_user_configuration() {
    let owner = actor();
    let service = declared_service(&owner);
    let created = service.create_definition(&owner, input()).await.unwrap();
    let entries = service.catalog(&owner).await.unwrap();
    let entry = entries
        .into_iter()
        .find(|e| e.id == Some(created.id))
        .unwrap();
    assert_eq!(entry.enabled, Some(true));
    assert_eq!(entry.owner_id, None);
    assert_eq!(
        entry.scope,
        EventSourceScope::Workspace {
            workspace_id: owner.current_workspace_id
        }
    );
    assert!(entry.interface_event.is_some());
}
fn source_entry(scope: EventSourceScope, phases: Vec<EventTriggerPhase>) -> EventCatalogEntry {
    EventSourceDescriptor {
        source_id: "native.same-contract".into(),
        owner_id: "native".into(),
        point_id: "native.point".into(),
        contract_id: "native.fact".into(),
        name: "Fact".into(),
        description: String::new(),
        payload_schema: serde_json::json!({"type":"object"}),
        origin: EventDefinitionOrigin::Core,
        scope,
        binding: EventSourceBinding::Active,
        coverage: EventSourceCoverage::Live,
        supported_phases: phases,
    }
    .into()
}

#[tokio::test]
async fn source_selection_authorizes_scope_and_matches_identity_before_phase() {
    let mut owner = actor();
    owner.current_workspace_id = domain::SYSTEM_SCOPE_ID;
    let workspace = EventSourceScope::Workspace {
        workspace_id: owner.current_workspace_id,
    };
    let entries = vec![
        source_entry(workspace.clone(), vec![EventTriggerPhase::Consumed]),
        source_entry(EventSourceScope::System, vec![EventTriggerPhase::Published]),
    ];
    let service = EventCenterConfigurationService::with_registered_events(
        MemoryRepository::default(),
        owner.current_workspace_id,
        entries.clone(),
    );
    let mut selected = trigger_input();
    selected.contract_id = "native.fact".into();
    selected.trigger_phase = EventTriggerPhase::Published;
    selected.source = Some(EventSourceSelection {
        filters: Default::default(),

        source_id: "native.same-contract".into(),
        scope: EventSourceScope::System,
    });
    assert!(service
        .validate_workflow_trigger(&owner, &selected)
        .await
        .is_ok());
    let mut non_root = owner.clone();
    non_root.is_root = false;
    assert!(service
        .validate_workflow_trigger(&non_root, &selected)
        .await
        .is_err());
    selected.source.as_mut().unwrap().scope = workspace.clone();
    assert!(service
        .validate_workflow_trigger(&owner, &selected)
        .await
        .is_err());
    selected.trigger_phase = EventTriggerPhase::Consumed;
    assert!(service
        .validate_workflow_trigger(&non_root, &selected)
        .await
        .is_ok());
    let saved = service
        .save_workflow_trigger(&owner, Uuid::now_v7(), selected.clone())
        .await
        .unwrap();
    assert_eq!(saved.configuration.source, selected.source);
    selected.source.as_mut().unwrap().scope = EventSourceScope::Workspace {
        workspace_id: Uuid::now_v7(),
    };
    assert!(service
        .validate_workflow_trigger(&owner, &selected)
        .await
        .is_err());
    selected.source.as_mut().unwrap().scope = EventSourceScope::Unbound;
    assert!(service
        .validate_workflow_trigger(&owner, &selected)
        .await
        .is_err());
    selected.source = None;
    assert!(
        service
            .validate_workflow_trigger(&owner, &selected)
            .await
            .is_err(),
        "legacy None must never select a registered native source"
    );
    let mut unavailable = entries;
    unavailable[0].supported_phases.clear();
    let unavailable_service = EventCenterConfigurationService::with_registered_events(
        MemoryRepository::default(),
        owner.current_workspace_id,
        unavailable,
    );
    selected.source = saved.configuration.source;
    assert!(unavailable_service
        .validate_workflow_trigger(&owner, &selected)
        .await
        .is_err());
    selected.enabled = false;
    assert!(unavailable_service
        .validate_workflow_trigger(&owner, &selected)
        .await
        .is_ok());
}
