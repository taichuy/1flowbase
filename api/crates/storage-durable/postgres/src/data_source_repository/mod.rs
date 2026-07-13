use anyhow::Result;
use async_trait::async_trait;
use control_plane::{
    errors::ControlPlaneError,
    ports::{
        CreateDataSourceInstanceInput, CreateDataSourcePreviewSessionInput, DataSourceRepository,
        RotateDataSourceSecretInput, RotateDataSourceSecretOutput,
        UpdateDataSourceInstanceConfigInput, UpdateDataSourceInstanceStatusInput,
        UpsertDataSourceCatalogCacheInput, UpsertDataSourceSecretInput,
    },
};
use sqlx::Row;
use uuid::Uuid;

use crate::{
    repositories::PgControlPlaneStore,
    secret_crypto::{decrypt_secret_json, encrypt_secret_json},
};

fn decrypt_data_source_secret(
    stored: &serde_json::Value,
    master_key: &str,
) -> Result<serde_json::Value> {
    if stored.get("algorithm").is_some() {
        decrypt_secret_json(stored, master_key)
    } else {
        // Historical data source rows were stored as plaintext JSON. Read them in place so
        // existing instances keep running; only explicit create/rotate writes use AEAD.
        Ok(stored.clone())
    }
}

fn parse_instance_status(value: &str) -> Result<domain::DataSourceInstanceStatus> {
    match value {
        "draft" => Ok(domain::DataSourceInstanceStatus::Draft),
        "ready" => Ok(domain::DataSourceInstanceStatus::Ready),
        "invalid" => Ok(domain::DataSourceInstanceStatus::Invalid),
        "disabled" => Ok(domain::DataSourceInstanceStatus::Disabled),
        _ => Err(ControlPlaneError::InvalidInput("data_source_instance.status").into()),
    }
}

fn parse_refresh_status(value: &str) -> Result<domain::DataSourceCatalogRefreshStatus> {
    match value {
        "idle" => Ok(domain::DataSourceCatalogRefreshStatus::Idle),
        "ready" => Ok(domain::DataSourceCatalogRefreshStatus::Ready),
        "failed" => Ok(domain::DataSourceCatalogRefreshStatus::Failed),
        _ => Err(ControlPlaneError::InvalidInput("data_source_catalog.refresh_status").into()),
    }
}

fn map_instance(row: sqlx::postgres::PgRow) -> Result<domain::DataSourceInstanceRecord> {
    let id = row.get("id");
    let secret_version: Option<i32> = row.get("secret_version");
    Ok(domain::DataSourceInstanceRecord {
        id,
        workspace_id: row.get("workspace_id"),
        installation_id: row.get("installation_id"),
        source_code: row.get("source_code"),
        display_name: row.get("display_name"),
        status: parse_instance_status(row.get::<String, _>("status").as_str())?,
        config_json: row.get("config_json"),
        metadata_json: row.get("metadata_json"),
        secret_ref: secret_version.map(|_| domain::data_source_secret_ref(id)),
        secret_version,
        defaults: domain::DataSourceDefaults {
            data_model_status: domain::DataModelStatus::from_db(
                row.get::<String, _>("default_data_model_status").as_str(),
            ),
        },
        created_by: row.get("created_by"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

fn map_secret(row: sqlx::postgres::PgRow) -> Result<domain::DataSourceSecretRecord> {
    Ok(domain::DataSourceSecretRecord {
        data_source_instance_id: row.get("data_source_instance_id"),
        secret_ref: domain::data_source_secret_ref(row.get("data_source_instance_id")),
        encrypted_secret_json: row.get("encrypted_secret_json"),
        secret_version: row.get("secret_version"),
        updated_at: row.get("updated_at"),
    })
}

fn map_catalog_cache(row: sqlx::postgres::PgRow) -> Result<domain::DataSourceCatalogCacheRecord> {
    Ok(domain::DataSourceCatalogCacheRecord {
        data_source_instance_id: row.get("data_source_instance_id"),
        refresh_status: parse_refresh_status(row.get::<String, _>("refresh_status").as_str())?,
        catalog_json: row.get("catalog_json"),
        last_error_message: row.get("last_error_message"),
        refreshed_at: row.get("refreshed_at"),
        updated_at: row.get("updated_at"),
    })
}

fn map_preview_session(
    row: sqlx::postgres::PgRow,
) -> Result<domain::DataSourcePreviewSessionRecord> {
    Ok(domain::DataSourcePreviewSessionRecord {
        id: row.get("id"),
        workspace_id: row.get("workspace_id"),
        actor_user_id: row.get("actor_user_id"),
        data_source_instance_id: row.get("data_source_instance_id"),
        config_fingerprint: row.get("config_fingerprint"),
        preview_json: row.get("preview_json"),
        expires_at: row.get("expires_at"),
        created_at: row.get("created_at"),
    })
}

fn refresh_secret_reference_versions(
    value: &serde_json::Value,
    secret_ref: &str,
    secret_version: i32,
) -> serde_json::Value {
    match value {
        serde_json::Value::Object(object) if is_secret_reference_marker(value) => {
            let mut updated = object.clone();
            if updated
                .get("secret_ref")
                .and_then(serde_json::Value::as_str)
                .map(|value| value == secret_ref)
                .unwrap_or(false)
            {
                updated.insert(
                    "secret_version".to_string(),
                    serde_json::json!(secret_version),
                );
            }
            serde_json::Value::Object(updated)
        }
        serde_json::Value::Object(object) => serde_json::Value::Object(
            object
                .iter()
                .map(|(key, child)| {
                    (
                        key.clone(),
                        refresh_secret_reference_versions(child, secret_ref, secret_version),
                    )
                })
                .collect(),
        ),
        serde_json::Value::Array(items) => serde_json::Value::Array(
            items
                .iter()
                .map(|item| refresh_secret_reference_versions(item, secret_ref, secret_version))
                .collect(),
        ),
        _ => value.clone(),
    }
}

fn is_secret_reference_marker(value: &serde_json::Value) -> bool {
    value
        .as_object()
        .map(|object| object.contains_key("secret_ref") && object.contains_key("secret_version"))
        .unwrap_or(false)
}

// Partial rotation overwrites submitted top-level credentials while retaining the rest. The
// host-owned config marker map is merged separately because each entry represents one path.
fn merge_rotated_secret_values(
    existing: Option<&serde_json::Value>,
    incoming: &serde_json::Value,
) -> serde_json::Value {
    let Some(incoming_object) = incoming.as_object() else {
        return incoming.clone();
    };
    let mut merged_object = existing
        .and_then(serde_json::Value::as_object)
        .cloned()
        .unwrap_or_default();
    for (key, value) in incoming_object {
        if key != "__config_secret_values" {
            merged_object.insert(key.clone(), value.clone());
        }
    }

    let mut marker_values = existing
        .and_then(|value| value.get("__config_secret_values"))
        .and_then(serde_json::Value::as_object)
        .cloned()
        .unwrap_or_default();
    if let Some(incoming_marker_values) = incoming_object
        .get("__config_secret_values")
        .and_then(serde_json::Value::as_object)
    {
        for (key, value) in incoming_marker_values {
            marker_values.insert(key.clone(), value.clone());
        }
    }
    if !marker_values.is_empty() {
        merged_object.insert(
            "__config_secret_values".to_string(),
            serde_json::Value::Object(marker_values),
        );
    }

    serde_json::Value::Object(merged_object)
}

#[async_trait]
impl DataSourceRepository for PgControlPlaneStore {
    async fn list_instances(
        &self,
        workspace_id: Uuid,
    ) -> Result<Vec<domain::DataSourceInstanceRecord>> {
        let rows = sqlx::query(
            r#"
            select
                data_source_instances.id,
                data_source_instances.workspace_id,
                data_source_instances.installation_id,
                data_source_instances.source_code,
                data_source_instances.display_name,
                data_source_instances.status,
                data_source_instances.config_json,
                data_source_instances.metadata_json,
                data_source_instances.default_data_model_status,
                data_source_instances.created_by,
                data_source_instances.created_at,
                data_source_instances.updated_at,
                secrets.secret_version
            from data_source_instances
            left join data_source_secrets secrets
              on secrets.data_source_instance_id = data_source_instances.id
            where data_source_instances.workspace_id = $1
            order by data_source_instances.display_name asc, data_source_instances.created_at asc
            "#,
        )
        .bind(workspace_id)
        .fetch_all(self.pool())
        .await?;

        rows.into_iter().map(map_instance).collect()
    }

    async fn create_instance(
        &self,
        input: &CreateDataSourceInstanceInput,
    ) -> Result<domain::DataSourceInstanceRecord> {
        let row = sqlx::query(
            r#"
            insert into data_source_instances (
                id,
                workspace_id,
                installation_id,
                source_code,
                display_name,
                status,
                config_json,
                metadata_json,
                default_data_model_status,
                created_by
            ) values (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10
            )
            returning
                id,
                workspace_id,
                installation_id,
                source_code,
                display_name,
                status,
                config_json,
                metadata_json,
                default_data_model_status,
                created_by,
                created_at,
                updated_at,
                null::integer as secret_version
            "#,
        )
        .bind(input.instance_id)
        .bind(input.workspace_id)
        .bind(input.installation_id)
        .bind(&input.source_code)
        .bind(&input.display_name)
        .bind(input.status.as_str())
        .bind(&input.config_json)
        .bind(&input.metadata_json)
        .bind(input.defaults.data_model_status.as_str())
        .bind(input.created_by)
        .fetch_one(self.pool())
        .await?;

        map_instance(row)
    }

    async fn update_instance_status(
        &self,
        input: &UpdateDataSourceInstanceStatusInput,
    ) -> Result<domain::DataSourceInstanceRecord> {
        let row = sqlx::query(
            r#"
            with updated as (
                update data_source_instances
                set
                    status = $3,
                    metadata_json = $4,
                    updated_at = now()
                where workspace_id = $1
                  and id = $2
                returning
                    id,
                    workspace_id,
                    installation_id,
                    source_code,
                    display_name,
                    status,
                    config_json,
                    metadata_json,
                    default_data_model_status,
                    created_by,
                    created_at,
                    updated_at
            )
            select
                updated.id,
                updated.workspace_id,
                updated.installation_id,
                updated.source_code,
                updated.display_name,
                updated.status,
                updated.config_json,
                updated.metadata_json,
                updated.default_data_model_status,
                updated.created_by,
                updated.created_at,
                updated.updated_at,
                secrets.secret_version
            from updated
            left join data_source_secrets secrets
              on secrets.data_source_instance_id = updated.id
            "#,
        )
        .bind(input.workspace_id)
        .bind(input.instance_id)
        .bind(input.status.as_str())
        .bind(&input.metadata_json)
        .fetch_one(self.pool())
        .await?;

        map_instance(row)
    }

    async fn update_instance_defaults(
        &self,
        input: &control_plane::ports::UpdateDataSourceDefaultsInput,
    ) -> Result<domain::DataSourceInstanceRecord> {
        let row = sqlx::query(
            r#"
            with updated as (
                update data_source_instances
                set
                    default_data_model_status = $3,
                    updated_at = now()
                where workspace_id = $1
                  and id = $2
                returning
                    id,
                    workspace_id,
                    installation_id,
                    source_code,
                    display_name,
                    status,
                    config_json,
                    metadata_json,
                    default_data_model_status,
                    created_by,
                    created_at,
                    updated_at
            )
            select
                updated.id,
                updated.workspace_id,
                updated.installation_id,
                updated.source_code,
                updated.display_name,
                updated.status,
                updated.config_json,
                updated.metadata_json,
                updated.default_data_model_status,
                updated.created_by,
                updated.created_at,
                updated.updated_at,
                secrets.secret_version
            from updated
            left join data_source_secrets secrets
              on secrets.data_source_instance_id = updated.id
            "#,
        )
        .bind(input.workspace_id)
        .bind(input.instance_id)
        .bind(input.defaults.data_model_status.as_str())
        .fetch_one(self.pool())
        .await?;

        map_instance(row)
    }

    async fn get_main_source_defaults(
        &self,
        workspace_id: Uuid,
    ) -> Result<domain::DataSourceDefaults> {
        let row = sqlx::query(
            r#"
            select default_data_model_status
            from main_source_defaults
            where workspace_id = $1
            "#,
        )
        .bind(workspace_id)
        .fetch_optional(self.pool())
        .await?;

        Ok(row
            .map(|row| domain::DataSourceDefaults {
                data_model_status: domain::DataModelStatus::from_db(
                    row.get::<String, _>("default_data_model_status").as_str(),
                ),
            })
            .unwrap_or_default())
    }

    async fn update_main_source_defaults(
        &self,
        input: &control_plane::ports::UpdateMainSourceDefaultsInput,
    ) -> Result<domain::DataSourceDefaults> {
        let row = sqlx::query(
            r#"
            insert into main_source_defaults (
                id,
                workspace_id,
                default_data_model_status,
                created_by,
                updated_by
            ) values (
                $1, $2, $3, $4, $4
            )
            on conflict (workspace_id) do update
            set
                default_data_model_status = excluded.default_data_model_status,
                updated_by = excluded.updated_by,
                updated_at = now()
            returning default_data_model_status
            "#,
        )
        .bind(Uuid::now_v7())
        .bind(input.workspace_id)
        .bind(input.defaults.data_model_status.as_str())
        .bind(input.updated_by)
        .fetch_one(self.pool())
        .await?;

        Ok(domain::DataSourceDefaults {
            data_model_status: domain::DataModelStatus::from_db(
                row.get::<String, _>("default_data_model_status").as_str(),
            ),
        })
    }

    async fn update_instance_config(
        &self,
        input: &UpdateDataSourceInstanceConfigInput,
    ) -> Result<domain::DataSourceInstanceRecord> {
        let row = sqlx::query(
            r#"
            with updated as (
                update data_source_instances
                set
                    config_json = $3,
                    updated_at = now()
                where workspace_id = $1
                  and id = $2
                returning
                    id,
                    workspace_id,
                    installation_id,
                    source_code,
                    display_name,
                    status,
                    config_json,
                    metadata_json,
                    default_data_model_status,
                    created_by,
                    created_at,
                    updated_at
            )
            select
                updated.id,
                updated.workspace_id,
                updated.installation_id,
                updated.source_code,
                updated.display_name,
                updated.status,
                updated.config_json,
                updated.metadata_json,
                updated.default_data_model_status,
                updated.created_by,
                updated.created_at,
                updated.updated_at,
                secrets.secret_version
            from updated
            left join data_source_secrets secrets
              on secrets.data_source_instance_id = updated.id
            "#,
        )
        .bind(input.workspace_id)
        .bind(input.instance_id)
        .bind(&input.config_json)
        .fetch_one(self.pool())
        .await?;

        map_instance(row)
    }

    async fn get_instance(
        &self,
        workspace_id: Uuid,
        instance_id: Uuid,
    ) -> Result<Option<domain::DataSourceInstanceRecord>> {
        let row = sqlx::query(
            r#"
            select
                data_source_instances.id,
                data_source_instances.workspace_id,
                data_source_instances.installation_id,
                data_source_instances.source_code,
                data_source_instances.display_name,
                data_source_instances.status,
                data_source_instances.config_json,
                data_source_instances.metadata_json,
                data_source_instances.default_data_model_status,
                data_source_instances.created_by,
                data_source_instances.created_at,
                data_source_instances.updated_at,
                secrets.secret_version
            from data_source_instances
            left join data_source_secrets secrets
              on secrets.data_source_instance_id = data_source_instances.id
            where data_source_instances.workspace_id = $1
              and data_source_instances.id = $2
            "#,
        )
        .bind(workspace_id)
        .bind(instance_id)
        .fetch_optional(self.pool())
        .await?;

        row.map(map_instance).transpose()
    }

    async fn upsert_secret(
        &self,
        input: &UpsertDataSourceSecretInput,
    ) -> Result<domain::DataSourceSecretRecord> {
        let encrypted_secret_json =
            encrypt_secret_json(&input.plaintext_secret_json, &input.master_key)?;
        let row = sqlx::query(
            r#"
            with parent as (
                select scope_id, created_by
                from data_source_instances
                where id = $1
            )
            insert into data_source_secrets (
                id,
                data_source_instance_id,
                scope_id,
                encrypted_secret_json,
                secret_version,
                created_by,
                updated_by
            )
            select
                $2,
                $1,
                parent.scope_id,
                $3,
                $4,
                parent.created_by,
                parent.created_by
            from parent
            on conflict (data_source_instance_id) do update
            set
                scope_id = excluded.scope_id,
                encrypted_secret_json = excluded.encrypted_secret_json,
                secret_version = excluded.secret_version,
                updated_by = excluded.updated_by,
                updated_at = now()
            returning
                data_source_instance_id,
                encrypted_secret_json,
                secret_version,
                updated_at
            "#,
        )
        .bind(input.data_source_instance_id)
        .bind(Uuid::now_v7())
        .bind(&encrypted_secret_json)
        .bind(input.secret_version)
        .fetch_one(self.pool())
        .await?;

        map_secret(row)
    }

    async fn rotate_secret(
        &self,
        input: &RotateDataSourceSecretInput,
    ) -> Result<RotateDataSourceSecretOutput> {
        let mut transaction = self.pool().begin().await?;

        let instance_row = sqlx::query(
            r#"
            select config_json
            from data_source_instances
            where workspace_id = $1
              and id = $2
            for update
            "#,
        )
        .bind(input.workspace_id)
        .bind(input.data_source_instance_id)
        .fetch_optional(&mut *transaction)
        .await?
        .ok_or(ControlPlaneError::NotFound("data_source_instance"))?;
        let config_json: serde_json::Value = instance_row.get("config_json");
        let stored_secret_json: Option<serde_json::Value> = sqlx::query(
            r#"
            select encrypted_secret_json
            from data_source_secrets
            where data_source_instance_id = $1
            for update
            "#,
        )
        .bind(input.data_source_instance_id)
        .fetch_optional(&mut *transaction)
        .await?
        .map(|row| row.get("encrypted_secret_json"));
        let existing_secret_json = stored_secret_json
            .as_ref()
            .map(|stored| decrypt_data_source_secret(stored, &input.master_key))
            .transpose()?;
        let plaintext_secret_json = merge_rotated_secret_values(
            existing_secret_json.as_ref(),
            &input.plaintext_secret_json,
        );
        let encrypted_secret_json = encrypt_secret_json(&plaintext_secret_json, &input.master_key)?;

        let secret_row = sqlx::query(
            r#"
            with parent as (
                select scope_id, created_by
                from data_source_instances
                where id = $1
            )
            insert into data_source_secrets (
                id,
                data_source_instance_id,
                scope_id,
                encrypted_secret_json,
                secret_version,
                created_by,
                updated_by
            )
            select
                $2,
                $1,
                parent.scope_id,
                $3,
                1,
                parent.created_by,
                parent.created_by
            from parent
            on conflict (data_source_instance_id) do update
            set
                scope_id = excluded.scope_id,
                encrypted_secret_json = excluded.encrypted_secret_json,
                secret_version = data_source_secrets.secret_version + 1,
                updated_by = excluded.updated_by,
                updated_at = now()
            returning
                data_source_instance_id,
                encrypted_secret_json,
                secret_version,
                updated_at
        "#,
        )
        .bind(input.data_source_instance_id)
        .bind(Uuid::now_v7())
        .bind(&encrypted_secret_json)
        .fetch_one(&mut *transaction)
        .await?;
        let secret = map_secret(secret_row)?;

        let config_json = refresh_secret_reference_versions(
            &config_json,
            &input.secret_ref,
            secret.secret_version,
        );
        let updated_row = sqlx::query(
            r#"
            with updated as (
                update data_source_instances
                set
                    config_json = $3,
                    updated_at = now()
                where workspace_id = $1
                  and id = $2
                returning
                    id,
                    workspace_id,
                    installation_id,
                    source_code,
                    display_name,
                    status,
                    config_json,
                    metadata_json,
                    default_data_model_status,
                    created_by,
                    created_at,
                    updated_at
            )
            select
                updated.id,
                updated.workspace_id,
                updated.installation_id,
                updated.source_code,
                updated.display_name,
                updated.status,
                updated.config_json,
                updated.metadata_json,
                updated.default_data_model_status,
                updated.created_by,
                updated.created_at,
                updated.updated_at,
                secrets.secret_version
            from updated
            left join data_source_secrets secrets
              on secrets.data_source_instance_id = updated.id
            "#,
        )
        .bind(input.workspace_id)
        .bind(input.data_source_instance_id)
        .bind(&config_json)
        .fetch_one(&mut *transaction)
        .await?;
        let instance = map_instance(updated_row)?;

        transaction.commit().await?;

        Ok(RotateDataSourceSecretOutput { secret, instance })
    }

    async fn get_secret_record(
        &self,
        instance_id: Uuid,
    ) -> Result<Option<domain::DataSourceSecretRecord>> {
        let row = sqlx::query(
            r#"
            select
                data_source_instance_id,
                encrypted_secret_json,
                secret_version,
                updated_at
            from data_source_secrets
            where data_source_instance_id = $1
            "#,
        )
        .bind(instance_id)
        .fetch_optional(self.pool())
        .await?;

        row.map(map_secret).transpose()
    }

    async fn get_secret_json(
        &self,
        instance_id: Uuid,
        master_key: &str,
    ) -> Result<Option<serde_json::Value>> {
        let row = sqlx::query(
            r#"
            select encrypted_secret_json
            from data_source_secrets
            where data_source_instance_id = $1
            "#,
        )
        .bind(instance_id)
        .fetch_optional(self.pool())
        .await?;

        row.map(|row| {
            let stored: serde_json::Value = row.get("encrypted_secret_json");
            decrypt_data_source_secret(&stored, master_key)
        })
        .transpose()
    }

    async fn upsert_catalog_cache(
        &self,
        input: &UpsertDataSourceCatalogCacheInput,
    ) -> Result<domain::DataSourceCatalogCacheRecord> {
        let row = sqlx::query(
            r#"
            with parent as (
                select scope_id, created_by
                from data_source_instances
                where id = $1
            )
            insert into data_source_catalog_caches (
                id,
                data_source_instance_id,
                scope_id,
                refresh_status,
                catalog_json,
                last_error_message,
                refreshed_at,
                created_by,
                updated_by
            )
            select
                $2,
                $1,
                parent.scope_id,
                $3,
                $4,
                $5,
                $6,
                parent.created_by,
                parent.created_by
            from parent
            on conflict (data_source_instance_id) do update
            set
                scope_id = excluded.scope_id,
                refresh_status = excluded.refresh_status,
                catalog_json = excluded.catalog_json,
                last_error_message = excluded.last_error_message,
                refreshed_at = excluded.refreshed_at,
                updated_by = excluded.updated_by,
                updated_at = now()
            returning
                data_source_instance_id,
                refresh_status,
                catalog_json,
                last_error_message,
                refreshed_at,
                updated_at
            "#,
        )
        .bind(input.data_source_instance_id)
        .bind(Uuid::now_v7())
        .bind(input.refresh_status.as_str())
        .bind(&input.catalog_json)
        .bind(input.last_error_message.as_deref())
        .bind(input.refreshed_at)
        .fetch_one(self.pool())
        .await?;

        map_catalog_cache(row)
    }

    async fn get_catalog_cache(
        &self,
        workspace_id: Uuid,
        instance_id: Uuid,
    ) -> Result<Option<domain::DataSourceCatalogCacheRecord>> {
        let row = sqlx::query(
            r#"
            select
                cache.data_source_instance_id,
                cache.refresh_status,
                cache.catalog_json,
                cache.last_error_message,
                cache.refreshed_at,
                cache.updated_at
            from data_source_catalog_caches cache
            inner join data_source_instances instance
                on instance.id = cache.data_source_instance_id
            where cache.data_source_instance_id = $1
              and instance.scope_id = $2
            "#,
        )
        .bind(instance_id)
        .bind(workspace_id)
        .fetch_optional(self.pool())
        .await?;

        row.map(map_catalog_cache).transpose()
    }

    async fn create_preview_session(
        &self,
        input: &CreateDataSourcePreviewSessionInput,
    ) -> Result<domain::DataSourcePreviewSessionRecord> {
        let row = sqlx::query(
            r#"
            insert into data_source_preview_sessions (
                id,
                workspace_id,
                actor_user_id,
                data_source_instance_id,
                config_fingerprint,
                preview_json,
                expires_at
            ) values (
                $1, $2, $3, $4, $5, $6, $7
            )
            returning
                id,
                workspace_id,
                actor_user_id,
                data_source_instance_id,
                config_fingerprint,
                preview_json,
                expires_at,
                created_at
            "#,
        )
        .bind(input.session_id)
        .bind(input.workspace_id)
        .bind(input.actor_user_id)
        .bind(input.data_source_instance_id)
        .bind(&input.config_fingerprint)
        .bind(&input.preview_json)
        .bind(input.expires_at)
        .fetch_one(self.pool())
        .await?;

        map_preview_session(row)
    }
}
