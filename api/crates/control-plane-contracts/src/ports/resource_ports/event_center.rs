use async_trait::async_trait;
use domain::event_center::{EventDefinitionRecord, WorkflowEventTriggerRecord};
use uuid::Uuid;

/// All row operations include workspace_id, including updates by UUID.
#[async_trait]
pub trait EventCenterRepository: Send + Sync {
    /// Background bootstrap only: scopes with durable trigger configuration.
    async fn list_workflow_event_trigger_workspaces(&self) -> anyhow::Result<Vec<Uuid>>;
    async fn list_event_definitions(
        &self,
        workspace_id: Uuid,
    ) -> anyhow::Result<Vec<EventDefinitionRecord>>;
    async fn get_event_definition(
        &self,
        workspace_id: Uuid,
        id: Uuid,
    ) -> anyhow::Result<Option<EventDefinitionRecord>>;
    async fn create_event_definition(&self, record: &EventDefinitionRecord) -> anyhow::Result<()>;
    async fn update_event_definition(&self, record: &EventDefinitionRecord)
        -> anyhow::Result<bool>;
    async fn get_workflow_event_trigger(
        &self,
        workspace_id: Uuid,
        application_id: Uuid,
    ) -> anyhow::Result<Option<WorkflowEventTriggerRecord>>;
    async fn save_workflow_event_trigger(
        &self,
        record: &WorkflowEventTriggerRecord,
    ) -> anyhow::Result<bool>;
    /// Configuration inventory, including legacy disabled rows. Publication state owns activation.
    async fn list_workflow_event_triggers(
        &self,
        workspace_id: Uuid,
    ) -> anyhow::Result<Vec<WorkflowEventTriggerRecord>>;
}
