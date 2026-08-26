use anyhow::Result;
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

pub(super) struct ChangeLogEntry<'a> {
    pub(super) data_model_id: Option<Uuid>,
    pub(super) action: &'a str,
    pub(super) target_type: &'a str,
    pub(super) target_id: Option<Uuid>,
    pub(super) actor_user_id: Option<Uuid>,
    pub(super) before_snapshot: serde_json::Value,
    pub(super) after_snapshot: serde_json::Value,
    pub(super) execution_status: &'a str,
    pub(super) error_message: Option<String>,
}

pub(super) async fn append_change_log_tx(
    tx: &mut Transaction<'_, Postgres>,
    entry: &ChangeLogEntry<'_>,
) -> Result<()> {
    sqlx::query(
        r#"
        insert into model_change_logs (
            id,
            data_model_id,
            action,
            target_type,
            target_id,
            actor_user_id,
            before_snapshot,
            after_snapshot,
            execution_status,
            error_message
        )
        values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
        "#,
    )
    .bind(Uuid::now_v7())
    .bind(entry.data_model_id)
    .bind(entry.action)
    .bind(entry.target_type)
    .bind(entry.target_id)
    .bind(entry.actor_user_id)
    .bind(&entry.before_snapshot)
    .bind(&entry.after_snapshot)
    .bind(entry.execution_status)
    .bind(&entry.error_message)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub(super) async fn append_change_log(
    pool: &sqlx::PgPool,
    entry: &ChangeLogEntry<'_>,
) -> Result<()> {
    sqlx::query(
        r#"
        insert into model_change_logs (
            id,
            data_model_id,
            action,
            target_type,
            target_id,
            actor_user_id,
            before_snapshot,
            after_snapshot,
            execution_status,
            error_message
        )
        values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
        "#,
    )
    .bind(Uuid::now_v7())
    .bind(entry.data_model_id)
    .bind(entry.action)
    .bind(entry.target_type)
    .bind(entry.target_id)
    .bind(entry.actor_user_id)
    .bind(&entry.before_snapshot)
    .bind(&entry.after_snapshot)
    .bind(entry.execution_status)
    .bind(&entry.error_message)
    .execute(pool)
    .await?;
    Ok(())
}
