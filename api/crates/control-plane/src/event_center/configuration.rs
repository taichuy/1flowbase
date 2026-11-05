use control_plane_contracts::{ports::EventCenterRepository, ControlPlaneContractError as Error};
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
}

impl<R: EventCenterRepository> EventCenterConfigurationService<R> {
    pub fn new(repository: R) -> Self {
        Self {
            repository,
            registered_events: Vec::new(),
            registered_workspace_id: None,
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
        }
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

    pub async fn catalog(&self, actor: &ActorContext) -> anyhow::Result<Vec<EventCatalogEntry>> {
        let mut entries = if self.registered_workspace_id == Some(actor.current_workspace_id) {
            self.registered_events.clone()
        } else {
            Vec::new()
        };
        entries.extend(
            self.list_definitions(actor)
                .await?
                .into_iter()
                .map(EventCatalogEntry::from),
        );
        Ok(entries)
    }

    pub async fn create_definition(
        &self,
        actor: &ActorContext,
        input: CreateEventDefinitionInput,
    ) -> anyhow::Result<EventDefinitionRecord> {
        validate_identity(&input.contract_id, &input.contract_version)?;
        validate_fields(&input.name, &input.payload_schema)?;
        if self
            .registered_events
            .iter()
            .any(|entry| entry.contract_id == input.contract_id)
        {
            return Err(Error::Conflict("registered_event_identity").into());
        }
        let now = OffsetDateTime::now_utc();
        let record = EventDefinitionRecord {
            id: Uuid::now_v7(),
            workspace_id: actor.current_workspace_id,
            contract_id: input.contract_id,
            contract_version: input.contract_version,
            name: input.name,
            description: input.description,
            payload_schema: input.payload_schema,
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
        validate_fields(&input.name, &input.payload_schema)?;
        let mut record = self.get_definition(actor, id).await?;
        validate_identity(&record.contract_id, &record.contract_version)?;
        record.name = input.name;
        record.description = input.description;
        record.payload_schema = input.payload_schema;
        record.enabled = input.enabled;
        record.updated_at = OffsetDateTime::now_utc();
        if !self.repository.update_event_definition(&record).await? {
            return Err(Error::NotFound("event_definition").into());
        }
        Ok(record)
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
        if input
            .input_mapping
            .iter()
            .any(|(name, pointer)| name.trim().is_empty() || !is_json_pointer(pointer))
        {
            return Err(Error::InvalidInput("invalid_event_input_mapping").into());
        }
        let event = self
            .catalog(actor)
            .await?
            .into_iter()
            .find(|entry| {
                entry.enabled
                    && entry.contract_id == input.contract_id
                    && entry.contract_version == input.contract_version
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
        Ok(record)
    }
}

pub(super) fn validate_identity(contract_id: &str, contract_version: &str) -> anyhow::Result<()> {
    let slug = contract_id.strip_prefix("custom.").unwrap_or("");
    if slug.is_empty()
        || !slug.bytes().all(|ch| {
            ch.is_ascii_lowercase() || ch.is_ascii_digit() || matches!(ch, b'-' | b'_' | b'.')
        })
        || slug.split('.').any(str::is_empty)
        || contract_version.trim().is_empty()
        || contract_version.trim() != contract_version
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
