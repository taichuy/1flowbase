use anyhow::{anyhow, bail, Result};
use async_trait::async_trait;
use control_plane_contracts::ports::{
    ManagedSchemaApplyReceipt, ManagedSchemaFieldType, ManagedSchemaObjectKind,
    ManagedSchemaOperation, ManagedSchemaOwnershipRecord, ManagedSchemaPlan, ManagedSchemaPreview,
    ManagedSchemaPreviewAction, ManagedSchemaPreviewEntry, ManagedSchemaRepository,
};
use sqlx::{PgPool, Postgres, Row, Transaction};
use uuid::Uuid;

use crate::repositories::PgControlPlaneStore;

#[async_trait]
impl ManagedSchemaRepository for PgControlPlaneStore {
    async fn preview_managed_schema(
        &self,
        plan: &ManagedSchemaPlan,
    ) -> Result<ManagedSchemaPreview> {
        validate_plan(plan)?;
        let mut entries = Vec::with_capacity(plan.operations.len());
        for operation in &plan.operations {
            entries.push(preview_operation(self.pool(), plan, operation).await?);
        }
        Ok(ManagedSchemaPreview {
            owner_id: plan.owner_id.clone(),
            fingerprint: plan.fingerprint.clone(),
            entries,
        })
    }

    async fn apply_managed_schema(
        &self,
        plan: &ManagedSchemaPlan,
    ) -> Result<ManagedSchemaApplyReceipt> {
        validate_plan(plan)?;
        if let Some(receipt) = find_receipt(self.pool(), &plan.owner_id, &plan.fingerprint).await? {
            return Ok(receipt);
        }

        let mut transaction = self.pool().begin().await?;
        sqlx::query("select pg_advisory_xact_lock(hashtextextended($1, 0))")
            .bind(&plan.owner_id)
            .execute(&mut *transaction)
            .await?;
        sqlx::query("select set_config('lock_timeout', $1, true)")
            .bind(format!("{}ms", plan.lock_timeout_ms))
            .execute(&mut *transaction)
            .await?;

        let mut created_objects = 0_u32;
        let mut existing_objects = 0_u32;
        let mut retained_objects = 0_u32;
        for operation in &plan.operations {
            match apply_operation(&mut transaction, plan, operation).await? {
                ManagedSchemaPreviewAction::Create => created_objects += 1,
                ManagedSchemaPreviewAction::AlreadyPresent => existing_objects += 1,
                ManagedSchemaPreviewAction::Retain => retained_objects += 1,
            }
        }
        let receipt_id = Uuid::now_v7();
        let row = sqlx::query(
            r#"
            insert into plugin_schema_reconcile_receipts (
                receipt_id, owner_id, owner_version, plan_fingerprint,
                created_objects, existing_objects, retained_objects
            ) values ($1, $2, $3, $4, $5, $6, $7)
            on conflict (owner_id, plan_fingerprint) do update
            set owner_id = excluded.owner_id
            returning *
            "#,
        )
        .bind(receipt_id)
        .bind(&plan.owner_id)
        .bind(&plan.owner_version)
        .bind(&plan.fingerprint)
        .bind(i32::try_from(created_objects)?)
        .bind(i32::try_from(existing_objects)?)
        .bind(i32::try_from(retained_objects)?)
        .fetch_one(&mut *transaction)
        .await?;
        transaction.commit().await?;
        map_receipt(row)
    }

    async fn list_managed_schema_ownership(&self) -> Result<Vec<ManagedSchemaOwnershipRecord>> {
        sqlx::query("select * from plugin_schema_ownership order by owner_id, ownership_key")
            .fetch_all(self.pool())
            .await?
            .into_iter()
            .map(map_ownership)
            .collect()
    }
}

async fn preview_operation(
    pool: &PgPool,
    plan: &ManagedSchemaPlan,
    operation: &ManagedSchemaOperation,
) -> Result<ManagedSchemaPreviewEntry> {
    let key = ownership_key(operation);
    if let Some(owner) = sqlx::query_scalar::<_, String>(
        "select owner_id from plugin_schema_ownership where ownership_key = $1",
    )
    .bind(&key)
    .fetch_optional(pool)
    .await?
    {
        if owner != plan.owner_id {
            bail!("managed schema object {key} belongs to another owner");
        }
    }
    let action = match operation {
        ManagedSchemaOperation::EnsureOwnedCollection { physical_table, .. } => {
            if table_exists(pool, physical_table).await? {
                ManagedSchemaPreviewAction::AlreadyPresent
            } else {
                ManagedSchemaPreviewAction::Create
            }
        }
        ManagedSchemaOperation::EnsureOwnedField {
            physical_table,
            physical_column,
            field_type,
            nullable,
            ..
        } => {
            if table_exists(pool, physical_table).await? {
                ensure_owned_table(pool, &plan.owner_id, physical_table).await?;
                preview_column(
                    pool,
                    physical_table,
                    physical_column,
                    *field_type,
                    *nullable,
                )
                .await?
            } else if plan.operations.iter().any(|candidate| {
                matches!(
                    candidate,
                    ManagedSchemaOperation::EnsureOwnedCollection {
                        physical_table: planned_table,
                        ..
                    } if planned_table == physical_table
                )
            }) {
                ManagedSchemaPreviewAction::Create
            } else {
                bail!("managed schema owned collection is missing from the plan")
            }
        }
        ManagedSchemaOperation::EnsureExtensionField {
            target_table,
            physical_column,
            field_type,
            ..
        } => {
            ensure_registered_business_table(pool, target_table).await?;
            preview_column(pool, target_table, physical_column, *field_type, true).await?
        }
        ManagedSchemaOperation::RetainInactive { ownership_key } => {
            let owner = sqlx::query_scalar::<_, String>(
                "select owner_id from plugin_schema_ownership where ownership_key = $1",
            )
            .bind(ownership_key)
            .fetch_optional(pool)
            .await?
            .ok_or_else(|| anyhow!("retained managed schema ownership is missing"))?;
            if owner != plan.owner_id {
                bail!("retained managed schema ownership belongs to another owner");
            }
            ManagedSchemaPreviewAction::Retain
        }
    };
    Ok(ManagedSchemaPreviewEntry {
        ownership_key: key,
        action,
    })
}

async fn apply_operation(
    transaction: &mut Transaction<'_, Postgres>,
    plan: &ManagedSchemaPlan,
    operation: &ManagedSchemaOperation,
) -> Result<ManagedSchemaPreviewAction> {
    let key = ownership_key(operation);
    if let Some(owner) = sqlx::query_scalar::<_, String>(
        "select owner_id from plugin_schema_ownership where ownership_key = $1 for update",
    )
    .bind(&key)
    .fetch_optional(&mut **transaction)
    .await?
    {
        if owner != plan.owner_id {
            bail!("managed schema object {key} belongs to another owner");
        }
    }

    let (action, object_kind, logical_name, table, column, field_type, nullable) = match operation {
        ManagedSchemaOperation::EnsureOwnedCollection {
            logical_collection,
            physical_table,
        } => {
            let exists = table_exists_tx(transaction, physical_table).await?;
            if exists {
                let owner = table_owner_tx(transaction, physical_table).await?;
                if owner.as_deref() != Some(plan.owner_id.as_str()) {
                    bail!("owned table {physical_table} exists without matching ownership");
                }
            } else {
                let table = quote_identifier(physical_table)?;
                sqlx::query(&format!(
                    "create table {table} (id uuid primary key, scope_id uuid not null, created_at timestamptz not null default now(), updated_at timestamptz not null default now())"
                ))
                .execute(&mut **transaction)
                .await?;
            }
            (
                if exists {
                    ManagedSchemaPreviewAction::AlreadyPresent
                } else {
                    ManagedSchemaPreviewAction::Create
                },
                "owned_collection",
                logical_collection.clone(),
                physical_table.clone(),
                None,
                None,
                None,
            )
        }
        ManagedSchemaOperation::EnsureOwnedField {
            logical_collection,
            logical_field,
            physical_table,
            physical_column,
            field_type,
            nullable,
        } => {
            ensure_owned_table_tx(transaction, &plan.owner_id, physical_table).await?;
            let action = ensure_column(
                transaction,
                plan,
                physical_table,
                physical_column,
                *field_type,
                *nullable,
            )
            .await?;
            (
                action,
                "owned_field",
                format!("{logical_collection}.{logical_field}"),
                physical_table.clone(),
                Some(physical_column.clone()),
                Some(*field_type),
                Some(*nullable),
            )
        }
        ManagedSchemaOperation::EnsureExtensionField {
            target_table,
            logical_field,
            physical_column,
            field_type,
        } => {
            ensure_registered_business_table_tx(transaction, target_table).await?;
            let action = ensure_column(
                transaction,
                plan,
                target_table,
                physical_column,
                *field_type,
                true,
            )
            .await?;
            (
                action,
                "extension_field",
                logical_field.clone(),
                target_table.clone(),
                Some(physical_column.clone()),
                Some(*field_type),
                Some(true),
            )
        }
        ManagedSchemaOperation::RetainInactive { ownership_key } => {
            let updated = sqlx::query(
                "update plugin_schema_ownership set active = false, owner_version = $3, plan_fingerprint = $4, updated_at = now() where ownership_key = $1 and owner_id = $2",
            )
            .bind(ownership_key)
            .bind(&plan.owner_id)
            .bind(&plan.owner_version)
            .bind(&plan.fingerprint)
            .execute(&mut **transaction)
            .await?;
            if updated.rows_affected() != 1 {
                bail!("retained managed schema ownership is missing or belongs to another owner");
            }
            return Ok(ManagedSchemaPreviewAction::Retain);
        }
    };

    sqlx::query(
        r#"
        insert into plugin_schema_ownership (
            ownership_key, owner_id, owner_version, object_kind, logical_name,
            physical_table, physical_column, field_type, nullable, active, plan_fingerprint
        ) values ($1, $2, $3, $4, $5, $6, $7, $8, $9, true, $10)
        on conflict (ownership_key) do update
        set owner_version = excluded.owner_version, active = true,
            plan_fingerprint = excluded.plan_fingerprint, updated_at = now()
        where plugin_schema_ownership.owner_id = excluded.owner_id
        "#,
    )
    .bind(key)
    .bind(&plan.owner_id)
    .bind(&plan.owner_version)
    .bind(object_kind)
    .bind(logical_name)
    .bind(table)
    .bind(column)
    .bind(field_type.map(ManagedSchemaFieldType::as_str))
    .bind(nullable)
    .bind(&plan.fingerprint)
    .execute(&mut **transaction)
    .await?;
    Ok(action)
}

async fn ensure_column(
    transaction: &mut Transaction<'_, Postgres>,
    plan: &ManagedSchemaPlan,
    table: &str,
    column: &str,
    field_type: ManagedSchemaFieldType,
    nullable: bool,
) -> Result<ManagedSchemaPreviewAction> {
    if let Some((actual_type, actual_nullable)) =
        column_contract_tx(transaction, table, column).await?
    {
        ensure_column_contract(
            table,
            column,
            field_type,
            nullable,
            &actual_type,
            actual_nullable,
        )?;
        return Ok(ManagedSchemaPreviewAction::AlreadyPresent);
    }
    let size = sqlx::query_scalar::<_, i64>("select pg_total_relation_size($1::regclass)")
        .bind(table)
        .fetch_one(&mut **transaction)
        .await?;
    if u64::try_from(size)? > plan.max_target_table_bytes {
        bail!("managed schema target table exceeds configured capacity preflight");
    }
    let table = quote_identifier(table)?;
    let column = quote_identifier(column)?;
    let nullability = if nullable { "" } else { " not null" };
    sqlx::query(&format!(
        "alter table {table} add column {column} {}{nullability}",
        postgres_type(field_type)
    ))
    .execute(&mut **transaction)
    .await?;
    Ok(ManagedSchemaPreviewAction::Create)
}

fn validate_plan(plan: &ManagedSchemaPlan) -> Result<()> {
    if plan.owner_id.trim().is_empty()
        || plan.owner_version.trim().is_empty()
        || plan.fingerprint.trim().is_empty()
        || plan.max_target_table_bytes == 0
        || plan.lock_timeout_ms == 0
    {
        bail!("managed schema plan identity and preconditions must be explicit");
    }
    Ok(())
}

fn ownership_key(operation: &ManagedSchemaOperation) -> String {
    match operation {
        ManagedSchemaOperation::EnsureOwnedCollection { physical_table, .. } => {
            format!("table:{physical_table}")
        }
        ManagedSchemaOperation::EnsureOwnedField {
            physical_table,
            physical_column,
            ..
        } => format!("column:{physical_table}.{physical_column}"),
        ManagedSchemaOperation::EnsureExtensionField {
            target_table,
            physical_column,
            ..
        } => format!("column:{target_table}.{physical_column}"),
        ManagedSchemaOperation::RetainInactive { ownership_key } => ownership_key.clone(),
    }
}

fn quote_identifier(value: &str) -> Result<String> {
    if value.is_empty()
        || value.len() > 63
        || !value.chars().all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
        })
    {
        bail!("managed schema physical identifier is invalid");
    }
    Ok(format!("\"{value}\""))
}

fn postgres_type(field_type: ManagedSchemaFieldType) -> &'static str {
    match field_type {
        ManagedSchemaFieldType::String | ManagedSchemaFieldType::Text => "text",
        ManagedSchemaFieldType::Number => "numeric",
        ManagedSchemaFieldType::Boolean => "boolean",
        ManagedSchemaFieldType::Datetime => "timestamptz",
        ManagedSchemaFieldType::Json => "jsonb",
        ManagedSchemaFieldType::Uuid => "uuid",
    }
}

fn expected_catalog_type(field_type: ManagedSchemaFieldType) -> &'static str {
    match field_type {
        ManagedSchemaFieldType::String | ManagedSchemaFieldType::Text => "text",
        ManagedSchemaFieldType::Number => "numeric",
        ManagedSchemaFieldType::Boolean => "boolean",
        ManagedSchemaFieldType::Datetime => "timestamp with time zone",
        ManagedSchemaFieldType::Json => "jsonb",
        ManagedSchemaFieldType::Uuid => "uuid",
    }
}

fn ensure_column_contract(
    table: &str,
    column: &str,
    expected_type: ManagedSchemaFieldType,
    expected_nullable: bool,
    actual_type: &str,
    actual_nullable: bool,
) -> Result<()> {
    if actual_type != expected_catalog_type(expected_type) || actual_nullable != expected_nullable {
        bail!("managed schema drift at {table}.{column}");
    }
    Ok(())
}

async fn table_exists(pool: &PgPool, table: &str) -> Result<bool> {
    Ok(
        sqlx::query_scalar::<_, Option<String>>("select to_regclass($1)::text")
            .bind(table)
            .fetch_one(pool)
            .await?
            .is_some(),
    )
}

async fn table_exists_tx(transaction: &mut Transaction<'_, Postgres>, table: &str) -> Result<bool> {
    Ok(
        sqlx::query_scalar::<_, Option<String>>("select to_regclass($1)::text")
            .bind(table)
            .fetch_one(&mut **transaction)
            .await?
            .is_some(),
    )
}

async fn ensure_registered_business_table(pool: &PgPool, table: &str) -> Result<()> {
    let registered = sqlx::query_scalar::<_, bool>(
        "select exists(select 1 from model_definitions where physical_table_name = $1)",
    )
    .bind(table)
    .fetch_one(pool)
    .await?;
    if !registered || !table_exists(pool, table).await? {
        bail!("managed schema target is not a registered business table");
    }
    Ok(())
}

async fn ensure_registered_business_table_tx(
    transaction: &mut Transaction<'_, Postgres>,
    table: &str,
) -> Result<()> {
    let registered = sqlx::query_scalar::<_, bool>(
        "select exists(select 1 from model_definitions where physical_table_name = $1)",
    )
    .bind(table)
    .fetch_one(&mut **transaction)
    .await?;
    if !registered || !table_exists_tx(transaction, table).await? {
        bail!("managed schema target is not a registered business table");
    }
    Ok(())
}

async fn ensure_owned_table(pool: &PgPool, owner_id: &str, table: &str) -> Result<()> {
    let owner = sqlx::query_scalar::<_, String>(
        "select owner_id from plugin_schema_ownership where physical_table = $1 and object_kind = 'owned_collection'",
    )
    .bind(table)
    .fetch_optional(pool)
    .await?;
    if owner.as_deref() != Some(owner_id) || !table_exists(pool, table).await? {
        bail!("managed schema owned collection is missing or has another owner");
    }
    Ok(())
}

async fn ensure_owned_table_tx(
    transaction: &mut Transaction<'_, Postgres>,
    owner_id: &str,
    table: &str,
) -> Result<()> {
    let owner = table_owner_tx(transaction, table).await?;
    if owner.as_deref() != Some(owner_id) || !table_exists_tx(transaction, table).await? {
        bail!("managed schema owned collection is missing or has another owner");
    }
    Ok(())
}

async fn table_owner_tx(
    transaction: &mut Transaction<'_, Postgres>,
    table: &str,
) -> Result<Option<String>> {
    Ok(sqlx::query_scalar::<_, String>(
        "select owner_id from plugin_schema_ownership where physical_table = $1 and object_kind = 'owned_collection'",
    )
    .bind(table)
    .fetch_optional(&mut **transaction)
    .await?)
}

async fn preview_column(
    pool: &PgPool,
    table: &str,
    column: &str,
    field_type: ManagedSchemaFieldType,
    nullable: bool,
) -> Result<ManagedSchemaPreviewAction> {
    let row = sqlx::query(
        "select data_type, is_nullable from information_schema.columns where table_schema = current_schema() and table_name = $1 and column_name = $2",
    )
    .bind(table)
    .bind(column)
    .fetch_optional(pool)
    .await?;
    let Some(row) = row else {
        return Ok(ManagedSchemaPreviewAction::Create);
    };
    let actual_type: String = row.try_get("data_type")?;
    let actual_nullable = row.try_get::<String, _>("is_nullable")? == "YES";
    ensure_column_contract(
        table,
        column,
        field_type,
        nullable,
        &actual_type,
        actual_nullable,
    )?;
    Ok(ManagedSchemaPreviewAction::AlreadyPresent)
}

async fn column_contract_tx(
    transaction: &mut Transaction<'_, Postgres>,
    table: &str,
    column: &str,
) -> Result<Option<(String, bool)>> {
    let row = sqlx::query(
        "select data_type, is_nullable from information_schema.columns where table_schema = current_schema() and table_name = $1 and column_name = $2",
    )
    .bind(table)
    .bind(column)
    .fetch_optional(&mut **transaction)
    .await?;
    row.map(|row| {
        Ok((
            row.try_get("data_type")?,
            row.try_get::<String, _>("is_nullable")? == "YES",
        ))
    })
    .transpose()
}

async fn find_receipt(
    pool: &PgPool,
    owner_id: &str,
    fingerprint: &str,
) -> Result<Option<ManagedSchemaApplyReceipt>> {
    sqlx::query(
        "select * from plugin_schema_reconcile_receipts where owner_id = $1 and plan_fingerprint = $2",
    )
    .bind(owner_id)
    .bind(fingerprint)
    .fetch_optional(pool)
    .await?
    .map(map_receipt)
    .transpose()
}

fn map_receipt(row: sqlx::postgres::PgRow) -> Result<ManagedSchemaApplyReceipt> {
    Ok(ManagedSchemaApplyReceipt {
        receipt_id: row.try_get("receipt_id")?,
        owner_id: row.try_get("owner_id")?,
        owner_version: row.try_get("owner_version")?,
        fingerprint: row.try_get("plan_fingerprint")?,
        created_objects: u32::try_from(row.try_get::<i32, _>("created_objects")?)?,
        existing_objects: u32::try_from(row.try_get::<i32, _>("existing_objects")?)?,
        retained_objects: u32::try_from(row.try_get::<i32, _>("retained_objects")?)?,
        applied_at: row.try_get("applied_at")?,
    })
}

fn map_ownership(row: sqlx::postgres::PgRow) -> Result<ManagedSchemaOwnershipRecord> {
    let object_kind = match row.try_get::<String, _>("object_kind")?.as_str() {
        "owned_collection" => ManagedSchemaObjectKind::OwnedCollection,
        "owned_field" => ManagedSchemaObjectKind::OwnedField,
        "extension_field" => ManagedSchemaObjectKind::ExtensionField,
        _ => return Err(anyhow!("invalid managed schema object kind")),
    };
    let field_type = row
        .try_get::<Option<String>, _>("field_type")?
        .map(|value| match value.as_str() {
            "string" => Ok(ManagedSchemaFieldType::String),
            "text" => Ok(ManagedSchemaFieldType::Text),
            "number" => Ok(ManagedSchemaFieldType::Number),
            "boolean" => Ok(ManagedSchemaFieldType::Boolean),
            "datetime" => Ok(ManagedSchemaFieldType::Datetime),
            "json" => Ok(ManagedSchemaFieldType::Json),
            "uuid" => Ok(ManagedSchemaFieldType::Uuid),
            _ => Err(anyhow!("invalid managed schema field type")),
        })
        .transpose()?;
    Ok(ManagedSchemaOwnershipRecord {
        ownership_key: row.try_get("ownership_key")?,
        owner_id: row.try_get("owner_id")?,
        owner_version: row.try_get("owner_version")?,
        object_kind,
        logical_name: row.try_get("logical_name")?,
        physical_table: row.try_get("physical_table")?,
        physical_column: row.try_get("physical_column")?,
        field_type,
        nullable: row.try_get("nullable")?,
        active: row.try_get("active")?,
        plan_fingerprint: row.try_get("plan_fingerprint")?,
    })
}
