mod backup_export;

use anyhow::Result;
use async_trait::async_trait;
use control_plane_contracts::ports::{
    AuthRepository, CreateFileStorageInput, CreateFileTableRegistrationInput,
    DeleteFileStorageInput, DeleteFileTableInput, FileManagementRepository,
    UpdateFileStorageBindingInput, UpdateFileStorageInput,
};
use sqlx::Row;
use uuid::Uuid;

use crate::repositories::PgControlPlaneStore;

fn parse_health_status(value: &str) -> Result<domain::FileStorageHealthStatus> {
    match value {
        "unknown" => Ok(domain::FileStorageHealthStatus::Unknown),
        "ready" => Ok(domain::FileStorageHealthStatus::Ready),
        "failed" => Ok(domain::FileStorageHealthStatus::Failed),
        _ => anyhow::bail!("invalid file storage health status"),
    }
}

fn parse_scope_kind(value: &str) -> Result<domain::FileTableScopeKind> {
    match value {
        "system" => Ok(domain::FileTableScopeKind::System),
        "workspace" => Ok(domain::FileTableScopeKind::Workspace),
        _ => anyhow::bail!("invalid file table scope kind"),
    }
}

fn map_file_storage(row: sqlx::postgres::PgRow) -> Result<domain::FileStorageRecord> {
    Ok(domain::FileStorageRecord {
        id: row.get("id"),
        code: row.get("code"),
        title: row.get("title"),
        driver_type: row.get("driver_type"),
        enabled: row.get("enabled"),
        is_default: row.get("is_default"),
        config_json: row.get("config_json"),
        rule_json: row.get("rule_json"),
        health_status: parse_health_status(row.get::<String, _>("health_status").as_str())?,
        last_health_error: row.get("last_health_error"),
        created_by: row.get("created_by"),
        updated_by: row.get("updated_by"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

fn map_file_table(row: sqlx::postgres::PgRow) -> Result<domain::FileTableRecord> {
    Ok(domain::FileTableRecord {
        id: row.get("id"),
        code: row.get("code"),
        title: row.get("title"),
        scope_kind: parse_scope_kind(row.get::<String, _>("scope_kind").as_str())?,
        scope_id: row.get("scope_id"),
        model_definition_id: row.get("model_definition_id"),
        bound_storage_id: row.get("bound_storage_id"),
        is_builtin: row.get("is_builtin"),
        is_default: row.get("is_default"),
        status: row.get("status"),
        created_by: row.get("created_by"),
        updated_by: row.get("updated_by"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

#[async_trait]
impl FileManagementRepository for PgControlPlaneStore {
    async fn load_actor_context_for_user(
        &self,
        actor_user_id: Uuid,
    ) -> Result<domain::ActorContext> {
        AuthRepository::load_actor_context_for_user(self, actor_user_id).await
    }

    async fn find_file_table_by_code(&self, code: &str) -> Result<Option<domain::FileTableRecord>> {
        let row = sqlx::query("select * from file_tables where code = $1")
            .bind(code)
            .fetch_optional(self.pool())
            .await?;

        row.map(map_file_table).transpose()
    }

    async fn get_file_table(&self, file_table_id: Uuid) -> Result<Option<domain::FileTableRecord>> {
        let row = sqlx::query("select * from file_tables where id = $1")
            .bind(file_table_id)
            .fetch_optional(self.pool())
            .await?;

        row.map(map_file_table).transpose()
    }

    async fn create_file_storage(
        &self,
        input: &CreateFileStorageInput,
    ) -> Result<domain::FileStorageRecord> {
        let row = sqlx::query(
            r#"
            insert into file_storages (
                id,
                scope_id,
                code,
                title,
                driver_type,
                enabled,
                is_default,
                config_json,
                rule_json,
                health_status,
                created_by,
                updated_by
            ) values (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, 'unknown', $10, $10
            )
            returning
                id,
                code,
                title,
                driver_type,
                enabled,
                is_default,
                config_json,
                rule_json,
                health_status,
                last_health_error,
                created_by,
                updated_by,
                created_at,
                updated_at
            "#,
        )
        .bind(input.storage_id)
        .bind(domain::SYSTEM_SCOPE_ID)
        .bind(&input.code)
        .bind(&input.title)
        .bind(&input.driver_type)
        .bind(input.enabled)
        .bind(input.is_default)
        .bind(&input.config_json)
        .bind(&input.rule_json)
        .bind(input.actor_user_id)
        .fetch_one(self.pool())
        .await?;

        map_file_storage(row)
    }

    async fn create_file_table_registration(
        &self,
        input: &CreateFileTableRegistrationInput,
    ) -> Result<domain::FileTableRecord> {
        let row = sqlx::query(
            r#"
            insert into file_tables (
                id,
                code,
                title,
                scope_kind,
                scope_id,
                model_definition_id,
                bound_storage_id,
                is_builtin,
                is_default,
                status,
                created_by,
                updated_by
            ) values (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, 'active', $10, $10
            )
            returning *
            "#,
        )
        .bind(input.file_table_id)
        .bind(&input.code)
        .bind(&input.title)
        .bind(match input.scope_kind {
            domain::FileTableScopeKind::System => "system",
            domain::FileTableScopeKind::Workspace => "workspace",
        })
        .bind(input.scope_id)
        .bind(input.model_definition_id)
        .bind(input.bound_storage_id)
        .bind(input.is_builtin)
        .bind(input.is_default)
        .bind(input.actor_user_id)
        .fetch_one(self.pool())
        .await?;

        map_file_table(row)
    }

    async fn list_file_storages(&self) -> Result<Vec<domain::FileStorageRecord>> {
        let rows = sqlx::query(
            r#"
            select
                id,
                code,
                title,
                driver_type,
                enabled,
                is_default,
                config_json,
                rule_json,
                health_status,
                last_health_error,
                created_by,
                updated_by,
                created_at,
                updated_at
            from file_storages
            order by is_default desc, created_at asc
            "#,
        )
        .fetch_all(self.pool())
        .await?;

        rows.into_iter().map(map_file_storage).collect()
    }

    async fn get_default_file_storage(&self) -> Result<Option<domain::FileStorageRecord>> {
        let row = sqlx::query(
            r#"
            select
                id,
                code,
                title,
                driver_type,
                enabled,
                is_default,
                config_json,
                rule_json,
                health_status,
                last_health_error,
                created_by,
                updated_by,
                created_at,
                updated_at
            from file_storages
            where is_default = true
            order by created_at asc
            limit 1
            "#,
        )
        .fetch_optional(self.pool())
        .await?;

        row.map(map_file_storage).transpose()
    }

    async fn get_file_storage(
        &self,
        storage_id: Uuid,
    ) -> Result<Option<domain::FileStorageRecord>> {
        let row = sqlx::query(
            r#"
            select
                id,
                code,
                title,
                driver_type,
                enabled,
                is_default,
                config_json,
                rule_json,
                health_status,
                last_health_error,
                created_by,
                updated_by,
                created_at,
                updated_at
            from file_storages
            where id = $1
            "#,
        )
        .bind(storage_id)
        .fetch_optional(self.pool())
        .await?;

        row.map(map_file_storage).transpose()
    }

    async fn list_visible_file_tables(
        &self,
        workspace_id: Uuid,
    ) -> Result<Vec<domain::FileTableRecord>> {
        let rows = sqlx::query(
            r#"
            select
                id,
                code,
                title,
                scope_kind,
                scope_id,
                model_definition_id,
                bound_storage_id,
                is_builtin,
                is_default,
                status,
                created_by,
                updated_by,
                created_at,
                updated_at
            from file_tables
            where (scope_kind = 'system' and scope_id = $1)
               or (scope_kind = 'workspace' and scope_id = $2)
            order by is_default desc, created_at asc
            "#,
        )
        .bind(domain::SYSTEM_SCOPE_ID)
        .bind(workspace_id)
        .fetch_all(self.pool())
        .await?;

        rows.into_iter().map(map_file_table).collect()
    }

    async fn update_file_table_binding(
        &self,
        input: &UpdateFileStorageBindingInput,
    ) -> Result<domain::FileTableRecord> {
        let row = sqlx::query(
            r#"
            update file_tables
            set
                bound_storage_id = $3,
                updated_by = $1,
                updated_at = now()
            where id = $2
            returning
                id,
                code,
                title,
                scope_kind,
                scope_id,
                model_definition_id,
                bound_storage_id,
                is_builtin,
                is_default,
                status,
                created_by,
                updated_by,
                created_at,
                updated_at
            "#,
        )
        .bind(input.actor_user_id)
        .bind(input.file_table_id)
        .bind(input.bound_storage_id)
        .fetch_one(self.pool())
        .await?;

        map_file_table(row)
    }

    async fn update_file_storage(
        &self,
        input: &UpdateFileStorageInput,
    ) -> Result<domain::FileStorageRecord> {
        let mut tx = self.pool().begin().await?;

        if input.is_default {
            sqlx::query(
                r#"
                update file_storages
                set
                    is_default = false,
                    updated_by = $1,
                    updated_at = now()
                where id <> $2
                  and is_default = true
                "#,
            )
            .bind(input.actor_user_id)
            .bind(input.file_storage_id)
            .execute(&mut *tx)
            .await?;
        }

        let row = sqlx::query(
            r#"
            update file_storages
            set
                title = $3,
                enabled = $4,
                is_default = $5,
                config_json = $6,
                rule_json = $7,
                updated_by = $1,
                updated_at = now()
            where id = $2
            returning
                id,
                code,
                title,
                driver_type,
                enabled,
                is_default,
                config_json,
                rule_json,
                health_status,
                last_health_error,
                created_by,
                updated_by,
                created_at,
                updated_at
            "#,
        )
        .bind(input.actor_user_id)
        .bind(input.file_storage_id)
        .bind(&input.title)
        .bind(input.enabled)
        .bind(input.is_default)
        .bind(&input.config_json)
        .bind(&input.rule_json)
        .fetch_one(&mut *tx)
        .await?;

        tx.commit().await?;

        map_file_storage(row)
    }

    async fn delete_file_storage(&self, input: &DeleteFileStorageInput) -> Result<()> {
        sqlx::query("delete from file_storages where id = $1")
            .bind(input.file_storage_id)
            .execute(self.pool())
            .await?;

        Ok(())
    }

    async fn delete_file_table(&self, input: &DeleteFileTableInput) -> Result<()> {
        sqlx::query("delete from file_tables where id = $1")
            .bind(input.file_table_id)
            .execute(self.pool())
            .await?;

        Ok(())
    }
}
