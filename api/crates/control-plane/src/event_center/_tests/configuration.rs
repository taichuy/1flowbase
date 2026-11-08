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
    async fn list_workflow_event_triggers(
        &self,
        workspace: Uuid,
    ) -> anyhow::Result<Vec<WorkflowEventTriggerRecord>> {
        Ok(self
            .triggers
            .lock()
            .unwrap()
            .iter()
            .filter(|row| row.workspace_id == workspace)
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
        interface_event: InterfaceEventBinding::legacy(
            "orders.create",
            InterfaceEventPhase::Before,
        ),
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
    value.interface_event = value
        .interface_event
        .with_results(vec![InterfaceEventTerminal::Failed]);
    assert!(service.create_definition(&owner, value).await.is_err());
    let mut value = input();
    value.interface_event =
        InterfaceEventBinding::legacy("undeclared", InterfaceEventPhase::Before);
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

#[tokio::test]
async fn unchanged_rule_can_be_disabled_after_its_interface_is_removed() {
    let actor = actor();
    let repository = MemoryRepository::default();
    let mut record = EventDefinitionRecord {
        interface_event: Some(input().interface_event),
        id: Uuid::now_v7(),
        workspace_id: actor.current_workspace_id,
        contract_id: "custom.interface.removed".into(),
        name: "Removed source".into(),
        description: String::new(),
        payload_schema: serde_json::json!({"type":"object"}),
        enabled: true,
        created_by: actor.user_id,
        created_at: OffsetDateTime::now_utc(),
        updated_at: OffsetDateTime::now_utc(),
    };
    repository.create_event_definition(&record).await.unwrap();
    let service = EventCenterConfigurationService::new(repository);
    let mut update = UpdateEventDefinitionInput {
        name: record.name.clone(),
        description: record.description.clone(),
        interface_event: record.interface_event.clone().unwrap(),
        enabled: false,
    };
    record = service
        .update_definition(&actor, record.id, update.clone())
        .await
        .unwrap();
    assert!(!record.enabled);
    update.enabled = true;
    assert!(
        service
            .update_definition(&actor, record.id, update)
            .await
            .is_err(),
        "missing declaration cannot be reactivated"
    );
}

fn api_details(owner: &ActorContext) -> ApiEventOperationDetails {
    let request = serde_json::json!({"type":"object","properties":{"query":{"type":"object","properties":{"page":{"type":"integer"}}}},"additionalProperties":false});
    let result = serde_json::json!({"type":"object","properties":{"items":{"type":"array","items":{"type":"string"}}},"required":["items"]});
    ApiEventOperationDetails {
        operation: ApiEventOperationSummary {
            operation_id: "list_applications".into(),
            method: "GET".into(),
            path: "/api/console/settings/applications".into(),
            name: "List applications".into(),
            description: String::new(),
            scope: EventSourceScope::Workspace {
                workspace_id: owner.current_workspace_id,
            },
            before: ApiEventPhaseAvailability::Available,
            completion: ApiEventPhaseAvailability::Available,
        },
        before_schema: Some(api_operation_event_schema(
            InterfaceEventPhase::Before,
            request.clone(),
            result.clone(),
        )),
        completion_schema: Some(api_operation_event_schema(
            InterfaceEventPhase::Completion,
            request.clone(),
            result.clone(),
        )),
        request_schema: Some(request),
        result_schema: Some(result),
    }
}

#[tokio::test]
async fn api_rules_use_selected_operation_schema_and_reject_scope_phase_and_identity_mismatch() {
    let owner = actor();
    let details = api_details(&owner);
    let service = EventCenterConfigurationService::new(MemoryRepository::default())
        .with_api_operations(owner.current_workspace_id, vec![details.clone()]);
    let mut value = input();
    value.interface_event =
        InterfaceEventBinding::api("list_applications", InterfaceEventPhase::Before);
    let created = service
        .create_definition(&owner, value.clone())
        .await
        .unwrap();
    assert_eq!(
        created.payload_schema,
        details.before_schema.clone().unwrap()
    );
    assert!(created.payload_schema["properties"]
        .get("operation_id")
        .is_some());
    assert!(created.payload_schema["properties"]
        .get("interface_id")
        .is_none());
    value.interface_event =
        InterfaceEventBinding::legacy("list_applications", InterfaceEventPhase::Before);
    assert!(service
        .create_definition(&owner, value.clone())
        .await
        .is_err());
    value.interface_event =
        InterfaceEventBinding::api("unknown_operation", InterfaceEventPhase::Before);
    assert!(service
        .create_definition(&owner, value.clone())
        .await
        .is_err());
    value.interface_event =
        InterfaceEventBinding::api("list_applications", InterfaceEventPhase::Completion)
            .with_results(vec![InterfaceEventTerminal::Failed]);
    let completion = service
        .create_definition(&owner, value.clone())
        .await
        .unwrap();
    assert_eq!(
        completion.payload_schema,
        details.completion_schema.clone().unwrap()
    );
    assert!(service
        .create_definition(&actor(), value.clone())
        .await
        .is_err());
    let mut unavailable = details.clone();
    unavailable.operation.completion = ApiEventPhaseAvailability::Unavailable {
        reason: ApiEventUnavailableReason::MissingStreamingTerminal,
    };
    let service = EventCenterConfigurationService::new(MemoryRepository::default())
        .with_api_operations(owner.current_workspace_id, vec![unavailable]);
    assert!(service
        .create_definition(&owner, value.clone())
        .await
        .is_err());
    let mut foreign = details.clone();
    foreign.operation.scope = EventSourceScope::Workspace {
        workspace_id: Uuid::now_v7(),
    };
    let service = EventCenterConfigurationService::new(MemoryRepository::default())
        .with_api_operations(owner.current_workspace_id, vec![foreign]);
    assert!(service
        .create_definition(&owner, value.clone())
        .await
        .is_err());
    let mut missing = details;
    missing.completion_schema = None;
    let service = EventCenterConfigurationService::new(MemoryRepository::default())
        .with_api_operations(owner.current_workspace_id, vec![missing]);
    assert!(service.create_definition(&owner, value).await.is_err());
}

#[tokio::test]
async fn unchanged_legacy_target_preserves_saved_schema_and_workflow_mapping() {
    let owner = actor();
    let service = declared_service(&owner);
    let created = service.create_definition(&owner, input()).await.unwrap();
    let mut mapping = trigger_input();
    mapping.contract_id = created.contract_id.clone();
    mapping
        .input_mapping
        .insert("order".into(), "/input/order_id".into());
    mapping.enabled = false;
    let application = Uuid::now_v7();
    service
        .save_workflow_trigger(&owner, application, mapping.clone())
        .await
        .unwrap();
    let repository = service.repository;
    let mut changed = source_entry(
        EventSourceScope::Workspace {
            workspace_id: owner.current_workspace_id,
        },
        vec![EventTriggerPhase::Published],
    );
    changed.interface_event = created.interface_event.clone();
    changed.payload_schema =
        serde_json::json!({"type":"object","properties":{"different":{"type":"integer"}}});
    let service = EventCenterConfigurationService::with_registered_events(
        repository,
        owner.current_workspace_id,
        vec![changed],
    );
    let updated = service
        .update_definition(
            &owner,
            created.id,
            UpdateEventDefinitionInput {
                name: "Rename existing rule".into(),
                description: String::new(),
                interface_event: created.interface_event.clone().unwrap(),
                enabled: true,
            },
        )
        .await
        .unwrap();
    assert_eq!(updated.payload_schema, created.payload_schema);
    assert_eq!(updated.interface_event, created.interface_event);
    assert_eq!(
        service
            .get_workflow_trigger(&owner, application)
            .await
            .unwrap()
            .unwrap()
            .configuration,
        mapping
    );
}
