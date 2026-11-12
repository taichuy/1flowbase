use control_plane_contracts::{
    ports::{EventCatalogRepository, EventCenterRepository},
    ControlPlaneContractError as Error,
};
use domain::{event_center::*, ActorContext};
use serde_json::Value;
use time::OffsetDateTime;
use uuid::Uuid;

/// Configuration owner only. Application publication and input-contract checks belong to
/// the application's activation path; saving configuration does not activate a consumer.
pub struct EventCenterConfigurationService<R> {
    repository: R,
    registered_events: Vec<EventCatalogEntry>,
    registered_workspace_id: Option<Uuid>,
    api_operations: Vec<ApiEventOperationDetails>,
    api_workspace_id: Option<Uuid>,
}

impl<R: EventCenterRepository> EventCenterConfigurationService<R> {
    pub fn new(repository: R) -> Self {
        Self {
            repository,
            registered_events: Vec::new(),
            registered_workspace_id: None,
            api_operations: Vec::new(),
            api_workspace_id: None,
        }
    }

    /// Caller supplies an already authorized current-workspace registry projection.
    /// Registered events remain read-only and must advertise only implemented source phases.
    pub fn with_registered_events(
        repository: R,
        workspace_id: Uuid,
        registered_events: Vec<EventCatalogEntry>,
    ) -> Self {
        Self {
            repository,
            registered_events,
            registered_workspace_id: Some(workspace_id),
            api_operations: Vec::new(),
            api_workspace_id: None,
        }
    }

    /// Host supplies authorized details for the selected documented operation. The operation
    /// catalog and its typed codecs remain owned by the host; this service owns saved rules.
    pub fn with_api_operations(
        mut self,
        workspace_id: Uuid,
        operations: Vec<ApiEventOperationDetails>,
    ) -> Self {
        self.api_workspace_id = Some(workspace_id);
        self.api_operations = operations;
        self
    }

    pub async fn list_definitions(
        &self,
        actor: &ActorContext,
    ) -> anyhow::Result<Vec<EventDefinitionRecord>> {
        self.repository
            .list_event_definitions(actor.current_workspace_id)
            .await
    }

    pub async fn get_definition(
        &self,
        actor: &ActorContext,
        id: Uuid,
    ) -> anyhow::Result<EventDefinitionRecord> {
        self.repository
            .get_event_definition(actor.current_workspace_id, id)
            .await?
            .ok_or_else(|| Error::NotFound("event_definition").into())
    }

    pub async fn catalog(&self, actor: &ActorContext) -> anyhow::Result<Vec<EventCatalogEntry>>
    where
        R: EventCatalogRepository,
    {
        self.catalog_with_query(actor, EventCatalogQuery::default())
            .await
    }

    pub async fn catalog_with_query(
        &self,
        actor: &ActorContext,
        query: EventCatalogQuery,
    ) -> anyhow::Result<Vec<EventCatalogEntry>>
    where
        R: EventCatalogRepository,
    {
        self.repository.list_event_catalog(actor, query).await
    }

    pub async fn create_definition(
        &self,
        actor: &ActorContext,
        input: CreateEventDefinitionInput,
    ) -> anyhow::Result<EventDefinitionRecord> {
        let payload_schema = self.interface_schema(actor, &input.interface_event)?;
        validate_fields(&input.name, &payload_schema)?;
        let now = OffsetDateTime::now_utc();
        let record = EventDefinitionRecord {
            interface_event: Some(input.interface_event),

            id: Uuid::now_v7(),
            workspace_id: actor.current_workspace_id,
            contract_id: format!("custom.interface.{}", Uuid::now_v7().simple()),
            name: input.name,
            description: input.description,
            payload_schema,
            enabled: input.enabled,
            created_by: actor.user_id,
            created_at: now,
            updated_at: now,
        };
        self.repository.create_event_definition(&record).await?;
        Ok(record)
    }

    pub async fn update_definition(
        &self,
        actor: &ActorContext,
        id: Uuid,
        input: UpdateEventDefinitionInput,
    ) -> anyhow::Result<EventDefinitionRecord> {
        let mut record = self.get_definition(actor, id).await?;
        // Stopping an unchanged rule stays possible when its interface is unavailable.
        let unchanged_legacy_target = record.interface_event.as_ref().is_some_and(|previous| {
            previous.legacy_interface_id().is_some() && previous.same_target(&input.interface_event)
        });
        let payload_schema =
            if !input.enabled && record.interface_event.as_ref() == Some(&input.interface_event) {
                record.payload_schema.clone()
            } else if unchanged_legacy_target {
                // Validate activation against the current owner without rewriting existing
                // typed payloads or the Workflow mappings which consume those payloads.
                self.interface_schema(actor, &input.interface_event)?;
                record.payload_schema.clone()
            } else {
                self.interface_schema(actor, &input.interface_event)?
            };
        validate_fields(&input.name, &payload_schema)?;
        validate_identity(&record.contract_id)?;
        record.name = input.name;
        record.description = input.description;
        record.payload_schema = payload_schema;
        record.interface_event = Some(input.interface_event);
        record.enabled = input.enabled;
        record.updated_at = OffsetDateTime::now_utc();
        if !self.repository.update_event_definition(&record).await? {
            return Err(Error::NotFound("event_definition").into());
        }
        Ok(record)
    }

    fn interface_schema(
        &self,
        actor: &ActorContext,
        binding: &InterfaceEventBinding,
    ) -> anyhow::Result<Value> {
        if binding.target_id().trim().is_empty()
            || (binding.phase() == InterfaceEventPhase::Before && !binding.results().is_empty())
        {
            return Err(Error::InvalidInput("invalid_interface_event_binding").into());
        }
        if let Some(operation_id) = binding.operation_id() {
            if self.api_workspace_id != Some(actor.current_workspace_id) {
                return Err(Error::InvalidInput("api_event_unavailable").into());
            }
            let operation = self
                .api_operations
                .iter()
                .find(|entry| entry.operation.operation_id == operation_id)
                .ok_or(Error::InvalidInput("api_event_unavailable"))?;
            if operation.operation.scope == EventSourceScope::Unbound
                || !source_visible(actor, &operation.operation.scope)
            {
                return Err(Error::PermissionDenied("api_event_scope").into());
            }
            return operation
                .phase_schema(binding.phase())
                .filter(|schema| schema.is_object() || schema.is_boolean())
                .cloned()
                .ok_or_else(|| Error::InvalidInput("api_event_phase_unavailable").into());
        }
        if self.registered_workspace_id != Some(actor.current_workspace_id) {
            return Err(Error::InvalidInput("interface_event_unavailable").into());
        }
        let declared = self
            .registered_events
            .iter()
            .find(|entry| {
                entry.origin != EventDefinitionOrigin::Custom
                    && source_visible(actor, &entry.scope)
                    && entry
                        .interface_event
                        .as_ref()
                        .is_some_and(|source| source.same_target(binding))
                    && entry.payload_schema != Value::Null
                    && !entry.supported_phases.is_empty()
            })
            .ok_or(Error::InvalidInput("interface_event_unavailable"))?;
        Ok(declared.payload_schema.clone())
    }

    pub async fn get_workflow_trigger(
        &self,
        actor: &ActorContext,
        application_id: Uuid,
    ) -> anyhow::Result<Option<WorkflowEventTriggerRecord>> {
        self.repository
            .get_workflow_event_trigger(actor.current_workspace_id, application_id)
            .await
    }

    pub async fn validate_workflow_trigger(
        &self,
        actor: &ActorContext,
        input: &WorkflowEventTriggerInput,
    ) -> anyhow::Result<()> {
        if let Some(source) = &input.source {
            if source.source_id.trim().is_empty()
                || !source_visible(actor, &source.scope)
                || source.scope == EventSourceScope::Unbound
            {
                return Err(Error::PermissionDenied("event_source_scope").into());
            }
        }
        if input.contract_id.trim().is_empty() {
            return Err(Error::InvalidInput("event_contract_required").into());
        }
        if input
            .input_mapping
            .iter()
            .any(|(name, pointer)| name.trim().is_empty() || !is_json_pointer(pointer))
        {
            return Err(Error::InvalidInput("invalid_event_input_mapping").into());
        }
        // Disabling a subscription must remain possible after its source is disabled/removed.
        // Availability is an activation constraint, not a prerequisite for stopping work.
        if !input.enabled {
            return Ok(());
        }
        let event = self
            .catalog(actor)
            .await?
            .into_iter()
            .find(|entry| {
                entry.enabled != Some(false)
                    && match (&input.source, &entry.source) {
                        (None, None) => entry.origin == EventDefinitionOrigin::Custom,
                        (Some(selected), Some(source)) => selected == &source.selection,
                        _ => false,
                    }
                    && entry.contract_id == input.contract_id
            })
            .ok_or(Error::InvalidInput("event_unavailable"))?;
        if !event.supported_phases.contains(&input.trigger_phase) {
            return Err(Error::InvalidInput("event_phase_unavailable").into());
        }
        Ok(())
    }

    pub async fn save_workflow_trigger(
        &self,
        actor: &ActorContext,
        application_id: Uuid,
        input: WorkflowEventTriggerInput,
    ) -> anyhow::Result<WorkflowEventTriggerRecord> {
        self.validate_workflow_trigger(actor, &input).await?;
        let previous = self.get_workflow_trigger(actor, application_id).await?;
        let record = WorkflowEventTriggerRecord {
            id: previous
                .map(|record| record.id)
                .unwrap_or_else(Uuid::now_v7),
            workspace_id: actor.current_workspace_id,
            application_id,
            configuration: input,
            updated_by: actor.user_id,
            updated_at: OffsetDateTime::now_utc(),
        };
        if !self.repository.save_workflow_event_trigger(&record).await? {
            return Err(Error::NotFound("application").into());
        }
        // A concurrent first save can win the unique application key with another generated id.
        // Return the persisted identity/revision, never an uncommitted subscription identity.
        self.get_workflow_trigger(actor, application_id)
            .await?
            .ok_or_else(|| Error::NotFound("workflow_event_trigger").into())
    }
}

pub(super) fn validate_identity(contract_id: &str) -> anyhow::Result<()> {
    let slug = contract_id.strip_prefix("custom.").unwrap_or("");
    if slug.is_empty()
        || !slug.bytes().all(|ch| {
            ch.is_ascii_lowercase() || ch.is_ascii_digit() || matches!(ch, b'-' | b'_' | b'.')
        })
        || slug.split('.').any(str::is_empty)
    {
        return Err(Error::InvalidInput("invalid_custom_event_identity").into());
    }
    Ok(())
}

fn validate_fields(name: &str, schema: &Value) -> anyhow::Result<()> {
    if name.trim().is_empty() {
        return Err(Error::InvalidInput("event_name_required").into());
    }
    payload_validator(schema).map(|_| ())
}

pub(super) fn payload_validator(schema: &Value) -> anyhow::Result<jsonschema::Validator> {
    // Configuration and publication cannot trigger external schema resolution.
    fn local_references(value: &Value) -> bool {
        match value {
            Value::Object(object) => object.iter().all(|(key, value)| {
                (!matches!(key.as_str(), "$ref" | "$dynamicRef" | "$recursiveRef")
                    || value
                        .as_str()
                        .is_some_and(|reference| reference.starts_with('#')))
                    && local_references(value)
            }),
            Value::Array(values) => values.iter().all(local_references),
            _ => true,
        }
    }
    if !local_references(schema) {
        return Err(Error::InvalidInput("invalid_event_payload_schema").into());
    }
    jsonschema::options()
        .offline()
        .build(schema)
        .map_err(|_| Error::InvalidInput("invalid_event_payload_schema").into())
}

#[cfg(test)]
#[path = "_tests/configuration.rs"]
mod tests;

fn source_visible(actor: &ActorContext, scope: &EventSourceScope) -> bool {
    match scope {
        EventSourceScope::Workspace { workspace_id } => *workspace_id == actor.current_workspace_id,
        EventSourceScope::System | EventSourceScope::Unbound => actor.is_root,
    }
}
