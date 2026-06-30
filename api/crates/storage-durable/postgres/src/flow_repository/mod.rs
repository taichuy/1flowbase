use anyhow::Result;
use async_trait::async_trait;
use control_plane::{errors::ControlPlaneError, ports::FlowRepository};
use sqlx::{Postgres, Row, Transaction};
use uuid::Uuid;

use crate::{
    mappers::flow_mapper::{PgFlowMapper, StoredFlowDraftRow, StoredFlowRow, StoredFlowVersionRow},
    repositories::PgControlPlaneStore,
};

#[async_trait]
impl FlowRepository for PgControlPlaneStore {
    async fn get_or_create_editor_state(
        &self,
        workspace_id: Uuid,
        application_id: Uuid,
        actor_user_id: Uuid,
    ) -> Result<domain::FlowEditorState> {
        let mut tx = self.pool().begin().await?;
        let application_type =
            ensure_application_exists(&mut tx, workspace_id, application_id).await?;
        let state =
            ensure_editor_state(&mut tx, application_id, actor_user_id, application_type).await?;
        tx.commit().await?;
        Ok(state)
    }

    async fn save_draft(
        &self,
        workspace_id: Uuid,
        application_id: Uuid,
        actor_user_id: Uuid,
        document: serde_json::Value,
        change_kind: domain::FlowChangeKind,
        summary: &str,
    ) -> Result<domain::FlowEditorState> {
        let mut tx = self.pool().begin().await?;
        let application_type =
            ensure_application_exists(&mut tx, workspace_id, application_id).await?;
        let state =
            ensure_editor_state(&mut tx, application_id, actor_user_id, application_type).await?;

        sqlx::query(
            r#"
            update flows
            set updated_by = $2,
                updated_at = now()
            where id = $1
            "#,
        )
        .bind(state.flow.id)
        .bind(actor_user_id)
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            r#"
            update flow_drafts
            set schema_version = $2,
                document = $3,
                updated_by = $4,
                updated_at = now()
            where flow_id = $1
            "#,
        )
        .bind(state.flow.id)
        .bind(document_schema_version(&document))
        .bind(&document)
        .bind(actor_user_id)
        .execute(&mut *tx)
        .await?;

        if matches!(change_kind, domain::FlowChangeKind::Logical) {
            insert_version(
                &mut tx,
                state.flow.id,
                actor_user_id,
                domain::FlowVersionTrigger::Autosave,
                summary,
                &document,
            )
            .await?;
            trim_versions(&mut tx, state.flow.id).await?;
        }

        let updated = fetch_editor_state(&mut tx, state.flow.id).await?;
        tx.commit().await?;
        Ok(updated)
    }

    async fn restore_version(
        &self,
        workspace_id: Uuid,
        application_id: Uuid,
        actor_user_id: Uuid,
        version_id: Uuid,
    ) -> Result<domain::FlowEditorState> {
        let mut tx = self.pool().begin().await?;
        let application_type =
            ensure_application_exists(&mut tx, workspace_id, application_id).await?;
        let state =
            ensure_editor_state(&mut tx, application_id, actor_user_id, application_type).await?;
        let restored = fetch_version(&mut tx, state.flow.id, version_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("flow_version"))?;

        sqlx::query(
            r#"
            update flows
            set updated_by = $2,
                updated_at = now()
            where id = $1
            "#,
        )
        .bind(state.flow.id)
        .bind(actor_user_id)
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            r#"
            update flow_drafts
            set schema_version = $2,
                document = $3,
                updated_by = $4,
                updated_at = now()
            where flow_id = $1
            "#,
        )
        .bind(state.flow.id)
        .bind(document_schema_version(&restored.document))
        .bind(&restored.document)
        .bind(actor_user_id)
        .execute(&mut *tx)
        .await?;

        insert_version(
            &mut tx,
            state.flow.id,
            actor_user_id,
            domain::FlowVersionTrigger::Restore,
            &format!("恢复版本 {}", restored.sequence),
            &restored.document,
        )
        .await?;
        trim_versions(&mut tx, state.flow.id).await?;

        let updated = fetch_editor_state(&mut tx, state.flow.id).await?;
        tx.commit().await?;
        Ok(updated)
    }

    async fn update_version_metadata(
        &self,
        workspace_id: Uuid,
        application_id: Uuid,
        actor_user_id: Uuid,
        version_id: Uuid,
        summary: Option<String>,
        summary_is_custom: Option<bool>,
        is_protected: Option<bool>,
    ) -> Result<domain::FlowEditorState> {
        let mut tx = self.pool().begin().await?;
        let application_type =
            ensure_application_exists(&mut tx, workspace_id, application_id).await?;
        let state =
            ensure_editor_state(&mut tx, application_id, actor_user_id, application_type).await?;
        let updated = sqlx::query_scalar::<_, Uuid>(
            r#"
            update flow_versions
            set summary = coalesce($3, summary),
                summary_is_custom = coalesce($4, summary_is_custom),
                is_protected = coalesce($5, is_protected),
                updated_by = $6,
                updated_at = now()
            where flow_id = $1 and id = $2
            returning id
            "#,
        )
        .bind(state.flow.id)
        .bind(version_id)
        .bind(summary)
        .bind(summary_is_custom)
        .bind(is_protected)
        .bind(actor_user_id)
        .fetch_optional(&mut *tx)
        .await?;

        if updated.is_none() {
            return Err(ControlPlaneError::NotFound("flow_version").into());
        }

        let updated = fetch_editor_state(&mut tx, state.flow.id).await?;
        tx.commit().await?;
        Ok(updated)
    }
}

async fn ensure_application_exists(
    tx: &mut Transaction<'_, Postgres>,
    workspace_id: Uuid,
    application_id: Uuid,
) -> Result<domain::ApplicationType> {
    let application_type = sqlx::query_scalar::<_, String>(
        "select application_type from applications where workspace_id = $1 and id = $2",
    )
    .bind(workspace_id)
    .bind(application_id)
    .fetch_optional(&mut **tx)
    .await?;

    let Some(application_type) = application_type else {
        return Err(ControlPlaneError::NotFound("application").into());
    };

    match application_type.as_str() {
        "agent_flow" => Ok(domain::ApplicationType::AgentFlow),
        "workflow" => Ok(domain::ApplicationType::Workflow),
        _ => Err(ControlPlaneError::InvalidInput("application_type").into()),
    }
}

async fn ensure_editor_state(
    tx: &mut Transaction<'_, Postgres>,
    application_id: Uuid,
    actor_user_id: Uuid,
    application_type: domain::ApplicationType,
) -> Result<domain::FlowEditorState> {
    let flow = match find_flow(tx, application_id).await? {
        Some(flow) => flow,
        None => insert_flow(tx, application_id, actor_user_id).await?,
    };

    if find_draft(tx, flow.id).await?.is_none() {
        let document = latest_version_document(tx, flow.id)
            .await?
            .unwrap_or_else(|| {
                domain::default_flow_document_for_application(application_type, flow.id)
            });
        insert_draft(tx, flow.id, &document, actor_user_id).await?;
    }

    if list_versions(tx, flow.id).await?.is_empty() {
        let draft = find_draft(tx, flow.id)
            .await?
            .ok_or(ControlPlaneError::NotFound("flow_draft"))?;
        insert_version(
            tx,
            flow.id,
            actor_user_id,
            domain::FlowVersionTrigger::Autosave,
            "初始化默认草稿",
            &draft.document,
        )
        .await?;
    }

    fetch_editor_state(tx, flow.id).await
}

async fn fetch_editor_state(
    tx: &mut Transaction<'_, Postgres>,
    flow_id: Uuid,
) -> Result<domain::FlowEditorState> {
    let flow = find_flow_by_id(tx, flow_id)
        .await?
        .ok_or(ControlPlaneError::NotFound("flow"))?;
    let draft = find_draft(tx, flow_id)
        .await?
        .ok_or(ControlPlaneError::NotFound("flow_draft"))?;
    let versions = list_versions(tx, flow_id).await?;

    Ok(domain::FlowEditorState {
        flow,
        draft,
        versions,
        autosave_interval_seconds: domain::FLOW_AUTOSAVE_INTERVAL_SECONDS,
    })
}

async fn find_flow(
    tx: &mut Transaction<'_, Postgres>,
    application_id: Uuid,
) -> Result<Option<domain::FlowRecord>> {
    let row = sqlx::query(
        r#"
        select id, application_id, created_by, updated_at
        from flows
        where application_id = $1
        "#,
    )
    .bind(application_id)
    .fetch_optional(&mut **tx)
    .await?;

    row.map(map_flow_row).transpose()
}

async fn find_flow_by_id(
    tx: &mut Transaction<'_, Postgres>,
    flow_id: Uuid,
) -> Result<Option<domain::FlowRecord>> {
    let row = sqlx::query(
        r#"
        select id, application_id, created_by, updated_at
        from flows
        where id = $1
        "#,
    )
    .bind(flow_id)
    .fetch_optional(&mut **tx)
    .await?;

    row.map(map_flow_row).transpose()
}

async fn insert_flow(
    tx: &mut Transaction<'_, Postgres>,
    application_id: Uuid,
    actor_user_id: Uuid,
) -> Result<domain::FlowRecord> {
    let row = sqlx::query(
        r#"
        insert into flows (id, application_id, scope_id, created_by, updated_by)
        values ($1, $2, (select scope_id from applications where id = $2), $3, $3)
        returning id, application_id, created_by, updated_at
        "#,
    )
    .bind(Uuid::now_v7())
    .bind(application_id)
    .bind(actor_user_id)
    .fetch_one(&mut **tx)
    .await?;

    map_flow_row(row)
}

async fn find_draft(
    tx: &mut Transaction<'_, Postgres>,
    flow_id: Uuid,
) -> Result<Option<domain::FlowDraftRecord>> {
    let row = sqlx::query(
        r#"
        select id, flow_id, schema_version, document, updated_at
        from flow_drafts
        where flow_id = $1
        "#,
    )
    .bind(flow_id)
    .fetch_optional(&mut **tx)
    .await?;

    row.map(map_draft_row).transpose()
}

async fn insert_draft(
    tx: &mut Transaction<'_, Postgres>,
    flow_id: Uuid,
    document: &serde_json::Value,
    actor_user_id: Uuid,
) -> Result<domain::FlowDraftRecord> {
    let row = sqlx::query(
        r#"
        insert into flow_drafts (id, flow_id, scope_id, schema_version, document, created_by, updated_by)
        select $1, flows.id, flows.scope_id, $3, $4, $5, $5
        from flows
        where flows.id = $2
        returning id, flow_id, schema_version, document, updated_at
        "#,
    )
    .bind(Uuid::now_v7())
    .bind(flow_id)
    .bind(document_schema_version(document))
    .bind(document)
    .bind(actor_user_id)
    .fetch_one(&mut **tx)
    .await?;

    map_draft_row(row)
}

async fn latest_version_document(
    tx: &mut Transaction<'_, Postgres>,
    flow_id: Uuid,
) -> Result<Option<serde_json::Value>> {
    sqlx::query_scalar(
        "select document from flow_versions where flow_id = $1 order by sequence desc limit 1",
    )
    .bind(flow_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(Into::into)
}

async fn list_versions(
    tx: &mut Transaction<'_, Postgres>,
    flow_id: Uuid,
) -> Result<Vec<domain::FlowVersionRecord>> {
    let rows = sqlx::query(
        r#"
        select id, flow_id, sequence, trigger, change_kind, summary, summary_is_custom, is_protected, document, created_at
        from flow_versions
        where flow_id = $1
        order by is_protected desc, sequence asc
        "#,
    )
    .bind(flow_id)
    .fetch_all(&mut **tx)
    .await?;

    rows.into_iter().map(map_version_row).collect()
}

async fn fetch_version(
    tx: &mut Transaction<'_, Postgres>,
    flow_id: Uuid,
    version_id: Uuid,
) -> Result<Option<domain::FlowVersionRecord>> {
    let row = sqlx::query(
        r#"
        select id, flow_id, sequence, trigger, change_kind, summary, summary_is_custom, is_protected, document, created_at
        from flow_versions
        where flow_id = $1 and id = $2
        "#,
    )
    .bind(flow_id)
    .bind(version_id)
    .fetch_optional(&mut **tx)
    .await?;

    row.map(map_version_row).transpose()
}

async fn insert_version(
    tx: &mut Transaction<'_, Postgres>,
    flow_id: Uuid,
    actor_user_id: Uuid,
    trigger: domain::FlowVersionTrigger,
    summary: &str,
    document: &serde_json::Value,
) -> Result<()> {
    let next_sequence: i64 = sqlx::query_scalar(
        "select coalesce(max(sequence), 0) + 1 from flow_versions where flow_id = $1",
    )
    .bind(flow_id)
    .fetch_one(&mut **tx)
    .await?;

    sqlx::query(
        r#"
        insert into flow_versions (
            id,
            flow_id,
            scope_id,
            sequence,
            trigger,
            change_kind,
            summary,
            summary_is_custom,
            is_protected,
            document,
            created_by,
            updated_by
        ) values ($1, $2, (select scope_id from flows where id = $2), $3, $4, 'logical', $5, false, false, $6, $7, $7)
        "#,
    )
    .bind(Uuid::now_v7())
    .bind(flow_id)
    .bind(next_sequence)
    .bind(trigger.as_str())
    .bind(summary)
    .bind(document)
    .bind(actor_user_id)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

async fn trim_versions(tx: &mut Transaction<'_, Postgres>, flow_id: Uuid) -> Result<()> {
    sqlx::query(
        r#"
        delete from flow_versions
        where id in (
            select id
            from flow_versions
            where flow_id = $1
              and is_protected = false
              and not exists (
                  select 1
                  from application_publication_versions publication
                  where publication.flow_version_id = flow_versions.id
              )
            order by sequence desc
            offset $2
        )
        "#,
    )
    .bind(flow_id)
    .bind(domain::FLOW_HISTORY_LIMIT as i64)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

fn map_flow_row(row: sqlx::postgres::PgRow) -> Result<domain::FlowRecord> {
    Ok(PgFlowMapper::to_flow_record(StoredFlowRow {
        id: row.get("id"),
        application_id: row.get("application_id"),
        created_by: row.get("created_by"),
        updated_at: row.get("updated_at"),
    }))
}

fn map_draft_row(row: sqlx::postgres::PgRow) -> Result<domain::FlowDraftRecord> {
    Ok(PgFlowMapper::to_flow_draft_record(StoredFlowDraftRow {
        id: row.get("id"),
        flow_id: row.get("flow_id"),
        schema_version: row.get("schema_version"),
        document: row.get("document"),
        updated_at: row.get("updated_at"),
    }))
}

fn map_version_row(row: sqlx::postgres::PgRow) -> Result<domain::FlowVersionRecord> {
    PgFlowMapper::to_flow_version_record(StoredFlowVersionRow {
        id: row.get("id"),
        flow_id: row.get("flow_id"),
        sequence: row.get("sequence"),
        trigger: row.get("trigger"),
        change_kind: row.get("change_kind"),
        summary: row.get("summary"),
        summary_is_custom: row.get("summary_is_custom"),
        is_protected: row.get("is_protected"),
        document: row.get("document"),
        created_at: row.get("created_at"),
    })
}

fn document_schema_version(document: &serde_json::Value) -> &str {
    document
        .get("schemaVersion")
        .and_then(serde_json::Value::as_str)
        .unwrap_or(domain::FLOW_SCHEMA_VERSION)
}
