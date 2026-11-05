use crate::PgControlPlaneStore;
use async_trait::async_trait;
use control_plane_contracts::{ports::EventCenterRepository, ControlPlaneContractError as Error};
use domain::event_center::*;
use sqlx::{postgres::PgRow, Row};
use uuid::Uuid;

fn definition(row: PgRow) -> anyhow::Result<EventDefinitionRecord> {
    Ok(EventDefinitionRecord {
        id: row.try_get("id")?,
        workspace_id: row.try_get("workspace_id")?,
        contract_id: row.try_get("contract_id")?,
        contract_version: row.try_get("contract_version")?,
        name: row.try_get("name")?,
        description: row.try_get("description")?,
        payload_schema: row.try_get("payload_schema")?,
        enabled: row.try_get("enabled")?,
        created_by: row.try_get("created_by")?,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
    })
}
fn trigger(row: PgRow) -> anyhow::Result<WorkflowEventTriggerRecord> {
    let phase: String = row.try_get("trigger_phase")?;
    Ok(WorkflowEventTriggerRecord {
        id: row.try_get("id")?,
        workspace_id: row.try_get("workspace_id")?,
        application_id: row.try_get("application_id")?,
        configuration: WorkflowEventTriggerInput {
            enabled: row.try_get("enabled")?,
            contract_id: row.try_get("contract_id")?,
            contract_version: row.try_get("contract_version")?,
            trigger_phase: serde_json::from_value(serde_json::Value::String(phase))?,
            input_mapping: serde_json::from_value(row.try_get("input_mapping")?)?,
        },
        updated_by: row.try_get("updated_by")?,
        updated_at: row.try_get("updated_at")?,
    })
}
fn mutation_error(error: sqlx::Error) -> anyhow::Error {
    if error
        .as_database_error()
        .is_some_and(|error| error.is_unique_violation())
    {
        Error::Conflict("event_definition_identity").into()
    } else {
        error.into()
    }
}

#[async_trait]
impl EventCenterRepository for PgControlPlaneStore {
    async fn list_workflow_event_trigger_workspaces(&self) -> anyhow::Result<Vec<Uuid>> {
        Ok(sqlx::query_scalar(
            "SELECT DISTINCT workspace_id FROM workflow_event_triggers ORDER BY workspace_id",
        )
        .fetch_all(self.pool())
        .await?)
    }

    async fn list_event_definitions(
        &self,
        workspace_id: Uuid,
    ) -> anyhow::Result<Vec<EventDefinitionRecord>> {
        sqlx::query("SELECT * FROM event_definitions WHERE workspace_id = $1 ORDER BY contract_id, contract_version, id")
            .bind(workspace_id).fetch_all(self.pool()).await?.into_iter().map(definition).collect()
    }
    async fn get_event_definition(
        &self,
        workspace_id: Uuid,
        id: Uuid,
    ) -> anyhow::Result<Option<EventDefinitionRecord>> {
        sqlx::query("SELECT * FROM event_definitions WHERE workspace_id = $1 AND id = $2")
            .bind(workspace_id)
            .bind(id)
            .fetch_optional(self.pool())
            .await?
            .map(definition)
            .transpose()
    }
    async fn create_event_definition(&self, record: &EventDefinitionRecord) -> anyhow::Result<()> {
        sqlx::query("INSERT INTO event_definitions (id,workspace_id,contract_id,contract_version,name,description,payload_schema,enabled,created_by,created_at,updated_at) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)")
            .bind(record.id).bind(record.workspace_id).bind(&record.contract_id).bind(&record.contract_version)
            .bind(&record.name).bind(&record.description).bind(&record.payload_schema).bind(record.enabled)
            .bind(record.created_by).bind(record.created_at).bind(record.updated_at)
            .execute(self.pool()).await.map_err(mutation_error)?;
        Ok(())
    }
    async fn update_event_definition(
        &self,
        record: &EventDefinitionRecord,
    ) -> anyhow::Result<bool> {
        let result = sqlx::query("UPDATE event_definitions SET name=$3,description=$4,payload_schema=$5,enabled=$6,updated_at=$7 WHERE workspace_id=$1 AND id=$2")
            .bind(record.workspace_id).bind(record.id).bind(&record.name).bind(&record.description)
            .bind(&record.payload_schema).bind(record.enabled).bind(record.updated_at).execute(self.pool()).await?;
        Ok(result.rows_affected() == 1)
    }
    async fn get_workflow_event_trigger(
        &self,
        workspace_id: Uuid,
        application_id: Uuid,
    ) -> anyhow::Result<Option<WorkflowEventTriggerRecord>> {
        sqlx::query("SELECT t.* FROM workflow_event_triggers t JOIN applications a ON a.id=t.application_id AND a.workspace_id=t.workspace_id WHERE t.workspace_id=$1 AND t.application_id=$2")
            .bind(workspace_id).bind(application_id).fetch_optional(self.pool()).await?.map(trigger).transpose()
    }
    async fn save_workflow_event_trigger(
        &self,
        record: &WorkflowEventTriggerRecord,
    ) -> anyhow::Result<bool> {
        let phase = serde_json::to_value(record.configuration.trigger_phase)?;
        let phase = phase
            .as_str()
            .ok_or(Error::InvalidInput("invalid_event_phase"))?;
        let result = sqlx::query("INSERT INTO workflow_event_triggers (id,workspace_id,application_id,enabled,contract_id,contract_version,trigger_phase,input_mapping,updated_by,updated_at) SELECT $1,$2,$3,$4,$5,$6,$7,$8,$9,$10 FROM applications WHERE id=$3 AND workspace_id=$2 ON CONFLICT (workspace_id,application_id) DO UPDATE SET enabled=EXCLUDED.enabled,contract_id=EXCLUDED.contract_id,contract_version=EXCLUDED.contract_version,trigger_phase=EXCLUDED.trigger_phase,input_mapping=EXCLUDED.input_mapping,updated_by=EXCLUDED.updated_by,updated_at=EXCLUDED.updated_at")
            .bind(record.id).bind(record.workspace_id).bind(record.application_id).bind(record.configuration.enabled)
            .bind(&record.configuration.contract_id).bind(&record.configuration.contract_version).bind(phase)
            .bind(serde_json::to_value(&record.configuration.input_mapping)?).bind(record.updated_by).bind(record.updated_at)
            .execute(self.pool()).await?;
        Ok(result.rows_affected() == 1)
    }
    async fn list_enabled_workflow_event_triggers(
        &self,
        workspace_id: Uuid,
    ) -> anyhow::Result<Vec<WorkflowEventTriggerRecord>> {
        sqlx::query("SELECT t.* FROM workflow_event_triggers t JOIN applications a ON a.id=t.application_id AND a.workspace_id=t.workspace_id WHERE t.workspace_id=$1 AND t.enabled ORDER BY t.application_id")
            .bind(workspace_id).fetch_all(self.pool()).await?.into_iter().map(trigger).collect()
    }
}
