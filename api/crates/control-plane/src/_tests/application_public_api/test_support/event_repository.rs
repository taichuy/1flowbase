use super::*;
use control_plane_contracts::ports::EventCenterRepository;
use domain::event_center::{EventDefinitionRecord, WorkflowEventTriggerRecord};

#[async_trait]
impl EventCenterRepository for ApplicationPublicApiTestRepository {
    async fn list_workflow_event_trigger_workspaces(&self) -> Result<Vec<Uuid>> {
        Ok(self
            .inner
            .lock()
            .unwrap()
            .workflow_event_triggers
            .values()
            .map(|row| row.workspace_id)
            .collect())
    }
    async fn list_event_definitions(
        &self,
        workspace_id: Uuid,
    ) -> Result<Vec<EventDefinitionRecord>> {
        Ok(self
            .inner
            .lock()
            .unwrap()
            .event_definitions
            .values()
            .filter(|row| row.workspace_id == workspace_id)
            .cloned()
            .collect())
    }
    async fn get_event_definition(
        &self,
        workspace_id: Uuid,
        id: Uuid,
    ) -> Result<Option<EventDefinitionRecord>> {
        Ok(self
            .inner
            .lock()
            .unwrap()
            .event_definitions
            .get(&id)
            .filter(|row| row.workspace_id == workspace_id)
            .cloned())
    }
    async fn create_event_definition(&self, record: &EventDefinitionRecord) -> Result<()> {
        let mut inner = self.inner.lock().unwrap();
        if inner.event_definitions.values().any(|row| {
            row.workspace_id == record.workspace_id && row.contract_id == record.contract_id
        }) {
            return Err(ControlPlaneError::Conflict("event_identity").into());
        }
        inner.event_definitions.insert(record.id, record.clone());
        Ok(())
    }
    async fn update_event_definition(&self, record: &EventDefinitionRecord) -> Result<bool> {
        let mut inner = self.inner.lock().unwrap();
        let Some(previous) = inner
            .event_definitions
            .get_mut(&record.id)
            .filter(|row| row.workspace_id == record.workspace_id)
        else {
            return Ok(false);
        };
        *previous = record.clone();
        Ok(true)
    }
    async fn get_workflow_event_trigger(
        &self,
        workspace_id: Uuid,
        application_id: Uuid,
    ) -> Result<Option<WorkflowEventTriggerRecord>> {
        Ok(self
            .inner
            .lock()
            .unwrap()
            .workflow_event_triggers
            .get(&application_id)
            .filter(|row| row.workspace_id == workspace_id)
            .cloned())
    }
    async fn save_workflow_event_trigger(
        &self,
        record: &WorkflowEventTriggerRecord,
    ) -> Result<bool> {
        let mut inner = self.inner.lock().unwrap();
        if !inner
            .applications
            .get(&record.application_id)
            .is_some_and(|app| {
                app.workspace_id == record.workspace_id
                    && app.workflow_trigger_type == Some(domain::WorkflowTriggerType::Event)
            })
        {
            return Ok(false);
        }
        inner
            .workflow_event_triggers
            .insert(record.application_id, record.clone());
        Ok(true)
    }
    async fn list_workflow_event_triggers(
        &self,
        workspace_id: Uuid,
    ) -> Result<Vec<WorkflowEventTriggerRecord>> {
        Ok(self
            .inner
            .lock()
            .unwrap()
            .workflow_event_triggers
            .values()
            .filter(|row| row.workspace_id == workspace_id)
            .cloned()
            .collect())
    }
}

#[async_trait]
impl control_plane_contracts::ports::EventCatalogRepository for ApplicationPublicApiTestRepository {
    async fn event_catalog_workspaces(&self) -> Result<Vec<domain::ScopeContext>> {
        Ok(Vec::new())
    }
    async fn synchronize_event_catalog(
        &self,
        _: &domain::event_center::EventCatalogSnapshot,
    ) -> Result<()> {
        anyhow::bail!("source reconciliation is outside this invocation fixture")
    }
    async fn list_event_catalog(
        &self,
        actor: &domain::ActorContext,
        query: domain::event_center::EventCatalogQuery,
    ) -> Result<Vec<domain::event_center::EventCatalogEntry>> {
        let mut entries = self
            .list_event_definitions(actor.current_workspace_id)
            .await?
            .into_iter()
            .map(domain::event_center::EventCatalogEntry::from)
            .collect::<Vec<_>>();
        query.sort(&mut entries);
        Ok(entries)
    }
}
