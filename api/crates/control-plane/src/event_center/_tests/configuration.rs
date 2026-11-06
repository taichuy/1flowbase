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
            existing.workspace_id == row.workspace_id
                && existing.contract_id == row.contract_id
                && existing.contract_version == row.contract_version
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
        contract_id: "custom.order.created".into(),
        contract_version: "1".into(),
        name: "Order created".into(),
        description: String::new(),
        payload_schema: serde_json::json!({"type":"object", "properties":{"order_id":{"type":"string"}}, "required":["order_id"]}),
        enabled: true,
    }
}
fn trigger_input() -> WorkflowEventTriggerInput {
    WorkflowEventTriggerInput {
        source: None,
        enabled: true,
        contract_id: "custom.order.created".into(),
        contract_version: "1".into(),
        trigger_phase: EventTriggerPhase::Consumed,
        input_mapping: BTreeMap::from([("order".into(), "/payload/order_id".into())]),
    }
}
#[tokio::test]
async fn unavailable_source_does_not_prevent_disabling_its_subscription() {
    let service = EventCenterConfigurationService::new(MemoryRepository::default());
    let owner = actor();
    let mut definition = input();
    definition.enabled = false;
    service.create_definition(&owner, definition).await.unwrap();
    let mut trigger = trigger_input();
    assert!(service
        .validate_workflow_trigger(&owner, &trigger)
        .await
        .is_err());
    trigger.enabled = false;
    let saved = service
        .save_workflow_trigger(&owner, Uuid::now_v7(), trigger)
        .await
        .unwrap();
    assert!(!saved.configuration.enabled);
}

#[tokio::test]
async fn definition_round_trip_and_workspace_isolation() {
    let service = EventCenterConfigurationService::new(MemoryRepository::default());
    let owner = actor();
    let outsider = actor();
    let created = service.create_definition(&owner, input()).await.unwrap();
    assert_eq!(created.workspace_id, owner.current_workspace_id);
    assert_eq!(created.created_by, owner.user_id);
    assert_eq!(
        service.get_definition(&owner, created.id).await.unwrap(),
        created
    );
    assert!(service
        .list_definitions(&outsider)
        .await
        .unwrap()
        .is_empty());
    assert!(service.get_definition(&outsider, created.id).await.is_err());
    let update = UpdateEventDefinitionInput {
        name: "Renamed".into(),
        description: String::new(),
        payload_schema: serde_json::json!({"type":"object"}),
        enabled: true,
    };
    assert!(service
        .update_definition(&outsider, created.id, update.clone())
        .await
        .is_err());
    let updated = service
        .update_definition(&owner, created.id, update)
        .await
        .unwrap();
    assert_eq!(updated.name, "Renamed");
    assert_eq!(updated.contract_id, created.contract_id);
    assert!(service.create_definition(&owner, input()).await.is_err());
    assert!(service.create_definition(&outsider, input()).await.is_ok());
}
#[tokio::test]
async fn rejects_owned_namespaces_invalid_schema_and_external_resolution() {
    let service = EventCenterConfigurationService::new(MemoryRepository::default());
    let owner = actor();
    for id in [
        "1flowbase.orders",
        "acme.plugin.orders",
        "custom.",
        "custom..orders",
    ] {
        let mut value = input();
        value.contract_id = id.into();
        assert!(service.create_definition(&owner, value).await.is_err());
    }
    for schema in [
        serde_json::json!({"type":"bogus"}),
        serde_json::json!({"$ref":"https://example.com/schema"}),
        serde_json::json!({"$ref":"file:///etc/passwd"}),
    ] {
        let mut value = input();
        value.payload_schema = schema;
        assert!(service.create_definition(&owner, value).await.is_err());
    }
    let mut value = input();
    value.payload_schema =
        serde_json::json!({"$defs":{"order":{"type":"object"}},"$ref":"#/$defs/order"});
    assert!(service.create_definition(&owner, value).await.is_ok());
}
#[tokio::test]
async fn triggers_require_visible_enabled_event_supported_phase_and_valid_pointers() {
    let owner = actor();
    let outsider = actor();
    let service = EventCenterConfigurationService::new(MemoryRepository::default());
    let created = service.create_definition(&owner, input()).await.unwrap();
    assert!(service
        .validate_workflow_trigger(&owner, &trigger_input())
        .await
        .is_ok());
    assert!(service
        .validate_workflow_trigger(&outsider, &trigger_input())
        .await
        .is_err());
    let mut invalid = trigger_input();
    invalid
        .input_mapping
        .insert("order".into(), "/bad~2".into());
    assert!(service
        .validate_workflow_trigger(&owner, &invalid)
        .await
        .is_err());
    let application_id = Uuid::now_v7();
    let saved = service
        .save_workflow_trigger(&owner, application_id, trigger_input())
        .await
        .unwrap();
    assert_eq!(
        saved.configuration.trigger_phase,
        EventTriggerPhase::Consumed
    );
    assert_eq!(saved.updated_by, owner.user_id);
    assert!(service
        .get_workflow_trigger(&outsider, application_id)
        .await
        .unwrap()
        .is_none());
    service
        .update_definition(
            &owner,
            created.id,
            UpdateEventDefinitionInput {
                name: created.name,
                description: created.description,
                payload_schema: created.payload_schema,
                enabled: false,
            },
        )
        .await
        .unwrap();
    assert!(service
        .validate_workflow_trigger(&owner, &trigger_input())
        .await
        .is_err());
    let service = EventCenterConfigurationService::with_registered_events(
        MemoryRepository::default(),
        owner.current_workspace_id,
        vec![EventCatalogEntry {
            source: None,
            id: None,
            enabled: true,
            owner_id: "acme".into(),
            contract_id: "acme.order".into(),
            contract_version: "1".into(),
            name: "Plugin order".into(),
            description: String::new(),
            payload_schema: serde_json::json!({}),
            origin: EventDefinitionOrigin::Plugin,
            supported_phases: vec![],
        }],
    );
    let mut unsupported = trigger_input();
    unsupported.contract_id = "acme.order".into();
    assert!(service
        .validate_workflow_trigger(&owner, &unsupported)
        .await
        .is_err());
}

fn source_entry(scope: EventSourceScope, phases: Vec<EventTriggerPhase>) -> EventCatalogEntry {
    EventSourceDescriptor {
        source_id: "native.same-contract".into(),
        owner_id: "native".into(),
        point_id: "native.point".into(),
        contract_id: "native.fact".into(),
        contract_version: "1".into(),
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
    unavailable[0].source.as_mut().unwrap().coverage = EventSourceCoverage::Unavailable;
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
