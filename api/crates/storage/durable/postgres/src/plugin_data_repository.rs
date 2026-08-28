use std::collections::BTreeMap;

use extension_contracts::{
    PluginDataBinding, PluginDataError, PluginDataErrorKind, PluginDataFilter,
    PluginDataFilterOperator, PluginDataOperation, PluginDataOperationResult, PluginDataOrder,
    PluginDataOrderDirection, PluginDataPort, PluginDataRequest, PluginDataResponse, PluginDataRow,
    PluginDataTarget, PluginDataValue,
};
use sha2::{Digest, Sha256};
use sqlx::{postgres::PgRow, Postgres, QueryBuilder, Row, Transaction, ValueRef};
use time::{format_description::well_known::Rfc3339, OffsetDateTime};
use uuid::Uuid;

use crate::repositories::PgControlPlaneStore;

const OWNED_COLLECTION: &str = "owned_collection";
const OWNED_FIELD: &str = "owned_field";
const EXTENSION_FIELD: &str = "extension_field";

#[derive(Clone)]
struct ResolvedField {
    physical: String,
    field_type: String,
}

struct ResolvedTarget {
    table: String,
    owned_collection: bool,
    fields: BTreeMap<String, ResolvedField>,
}

impl PluginDataPort for PgControlPlaneStore {
    fn execute<'a>(
        &'a self,
        binding: &'a PluginDataBinding,
        request: &'a PluginDataRequest,
    ) -> extension_contracts::PluginDataFuture<'a> {
        Box::pin(async move { execute_request(self, binding, request).await })
    }
}

async fn execute_request(
    store: &PgControlPlaneStore,
    binding: &PluginDataBinding,
    request: &PluginDataRequest,
) -> Result<PluginDataResponse, PluginDataError> {
    validate_binding(binding)?;
    request.validate()?;
    for operation in &request.operations {
        if !binding.permissions.contains(&operation.permission()) {
            return Err(error(
                PluginDataErrorKind::PermissionDenied,
                "plugin_data_permission",
                false,
            ));
        }
    }

    let workspace_id = Uuid::parse_str(&binding.workspace_id)
        .map_err(|_| PluginDataError::invalid("plugin_data_workspace"))?;
    let owner_id = format!("{}/{}", binding.publisher_namespace, binding.plugin_code);
    let request_hash = hex_digest(
        &serde_json::to_vec(request)
            .map_err(|_| PluginDataError::invalid("plugin_data_request_encoding"))?,
    );
    let mut transaction = store.pool().begin().await.map_err(storage_error)?;

    if let Some(key) = request.idempotency_key.as_deref() {
        if let Some((stored_hash, response)) = load_receipt(
            &mut transaction,
            &owner_id,
            workspace_id,
            &binding.provider_instance_id,
            key,
        )
        .await?
        {
            if stored_hash != request_hash {
                return Err(error(
                    PluginDataErrorKind::Conflict,
                    "plugin_data_idempotency_conflict",
                    false,
                ));
            }
            transaction.commit().await.map_err(storage_error)?;
            return Ok(PluginDataResponse {
                replayed: true,
                ..response
            });
        }
    }

    let mut results = Vec::with_capacity(request.operations.len());
    for operation in &request.operations {
        results.push(
            execute_operation(
                &mut transaction,
                &owner_id,
                &binding.plugin_version,
                workspace_id,
                operation,
            )
            .await?,
        );
    }
    let response = PluginDataResponse {
        results,
        replayed: false,
    };
    if let Some(key) = request.idempotency_key.as_deref() {
        store_receipt(
            &mut transaction,
            &owner_id,
            workspace_id,
            &binding.provider_instance_id,
            key,
            &request_hash,
            &response,
        )
        .await?;
    }
    transaction.commit().await.map_err(storage_error)?;
    Ok(response)
}

fn validate_binding(binding: &PluginDataBinding) -> Result<(), PluginDataError> {
    if binding.publisher_namespace.trim().is_empty()
        || binding.plugin_code.trim().is_empty()
        || binding.plugin_version.trim().is_empty()
        || binding.provider_instance_id.trim().is_empty()
        || binding.storage_binding != "main"
    {
        return Err(PluginDataError::invalid("plugin_data_binding"));
    }
    let now_ms = (OffsetDateTime::now_utc().unix_timestamp_nanos() / 1_000_000) as i64;
    if binding.deadline_unix_ms <= now_ms {
        return Err(error(
            PluginDataErrorKind::DeadlineExceeded,
            "plugin_data_deadline",
            false,
        ));
    }
    Ok(())
}

async fn execute_operation(
    transaction: &mut Transaction<'_, Postgres>,
    owner_id: &str,
    owner_version: &str,
    workspace_id: Uuid,
    operation: &PluginDataOperation,
) -> Result<PluginDataOperationResult, PluginDataError> {
    match operation {
        PluginDataOperation::Find {
            target,
            fields,
            filters,
            order,
            page,
        } => {
            let resolved = resolve_target(transaction, owner_id, owner_version, target).await?;
            let rows = select_rows(
                transaction,
                workspace_id,
                &resolved,
                fields,
                filters,
                order,
                Some((page.limit, page.offset)),
            )
            .await?;
            Ok(PluginDataOperationResult::Rows { rows })
        }
        PluginDataOperation::FindOne {
            target,
            fields,
            filters,
        } => {
            let resolved = resolve_target(transaction, owner_id, owner_version, target).await?;
            let row = select_rows(
                transaction,
                workspace_id,
                &resolved,
                fields,
                filters,
                &[],
                Some((1, 0)),
            )
            .await?
            .into_iter()
            .next();
            Ok(PluginDataOperationResult::OptionalRow { row })
        }
        PluginDataOperation::Count { target, filters } => {
            let resolved = resolve_target(transaction, owner_id, owner_version, target).await?;
            let count = count_rows(transaction, workspace_id, &resolved, filters).await?;
            Ok(PluginDataOperationResult::Count { count })
        }
        PluginDataOperation::Insert { target, values } => {
            let resolved = resolve_target(transaction, owner_id, owner_version, target).await?;
            require_owned(&resolved, "plugin_data_insert_target")?;
            let affected = insert_owned(transaction, workspace_id, &resolved, values).await?;
            Ok(PluginDataOperationResult::Mutation { affected })
        }
        PluginDataOperation::Update {
            target,
            filters,
            values,
        } => {
            if filters.is_empty() {
                return Err(PluginDataError::invalid("plugin_data_update_filters"));
            }
            let resolved = resolve_target(transaction, owner_id, owner_version, target).await?;
            let affected =
                update_rows(transaction, workspace_id, &resolved, filters, values).await?;
            Ok(PluginDataOperationResult::Mutation { affected })
        }
        PluginDataOperation::Delete { target, filters } => {
            if filters.is_empty() {
                return Err(PluginDataError::invalid("plugin_data_delete_filters"));
            }
            let resolved = resolve_target(transaction, owner_id, owner_version, target).await?;
            require_owned(&resolved, "plugin_data_delete_target")?;
            let affected = delete_rows(transaction, workspace_id, &resolved, filters).await?;
            Ok(PluginDataOperationResult::Mutation { affected })
        }
        PluginDataOperation::Upsert {
            target,
            identity,
            values,
        } => {
            let resolved = resolve_target(transaction, owner_id, owner_version, target).await?;
            require_owned(&resolved, "plugin_data_upsert_target")?;
            let affected =
                upsert_owned(transaction, workspace_id, &resolved, identity, values).await?;
            Ok(PluginDataOperationResult::Mutation { affected })
        }
    }
}

async fn resolve_target(
    transaction: &mut Transaction<'_, Postgres>,
    owner_id: &str,
    owner_version: &str,
    target: &PluginDataTarget,
) -> Result<ResolvedTarget, PluginDataError> {
    let (table, owned_collection) = match target {
        PluginDataTarget::OwnedCollection { collection_code } => {
            let row = sqlx::query(
                "select physical_table, owner_version from plugin_schema_ownership where owner_id = $1 and object_kind = $2 and logical_name = $3 and active = true",
            )
            .bind(owner_id)
            .bind(OWNED_COLLECTION)
            .bind(collection_code)
            .fetch_optional(&mut **transaction)
            .await
            .map_err(storage_error)?
            .ok_or_else(|| ownership_error("plugin_data_collection_ownership"))?;
            require_version(&row, owner_version)?;
            (row.try_get("physical_table").map_err(storage_error)?, true)
        }
        PluginDataTarget::ExtensionProjection { target_table } => {
            let exists = sqlx::query_scalar::<_, bool>(
                "select exists(select 1 from plugin_schema_ownership where owner_id = $1 and object_kind = $2 and physical_table = $3 and active = true)",
            )
            .bind(owner_id)
            .bind(EXTENSION_FIELD)
            .bind(target_table)
            .fetch_one(&mut **transaction)
            .await
            .map_err(storage_error)?;
            if !exists {
                return Err(ownership_error("plugin_data_projection_ownership"));
            }
            (target_table.clone(), false)
        }
    };

    let rows = sqlx::query(
        "select logical_name, physical_column, field_type, owner_version from plugin_schema_ownership where owner_id = $1 and physical_table = $2 and object_kind = $3 and active = true order by logical_name",
    )
    .bind(owner_id)
    .bind(&table)
    .bind(if owned_collection { OWNED_FIELD } else { EXTENSION_FIELD })
    .fetch_all(&mut **transaction)
    .await
    .map_err(storage_error)?;
    let mut fields = BTreeMap::new();
    fields.insert(
        "id".to_string(),
        ResolvedField {
            physical: "id".to_string(),
            field_type: "uuid".to_string(),
        },
    );
    if owned_collection {
        fields.insert(
            "created_at".to_string(),
            ResolvedField {
                physical: "created_at".to_string(),
                field_type: "datetime".to_string(),
            },
        );
        fields.insert(
            "updated_at".to_string(),
            ResolvedField {
                physical: "updated_at".to_string(),
                field_type: "datetime".to_string(),
            },
        );
    }
    for row in rows {
        require_version(&row, owner_version)?;
        let stored_name: String = row.try_get("logical_name").map_err(storage_error)?;
        let logical = if owned_collection {
            stored_name
                .split_once('.')
                .map(|(_, field)| field.to_string())
                .ok_or_else(|| ownership_error("plugin_data_owned_field_identity"))?
        } else {
            stored_name
        };
        fields.insert(
            logical,
            ResolvedField {
                physical: row.try_get("physical_column").map_err(storage_error)?,
                field_type: row.try_get("field_type").map_err(storage_error)?,
            },
        );
    }
    Ok(ResolvedTarget {
        table,
        owned_collection,
        fields,
    })
}

fn require_version(row: &PgRow, expected: &str) -> Result<(), PluginDataError> {
    let actual: String = row.try_get("owner_version").map_err(storage_error)?;
    if actual != expected {
        return Err(ownership_error("plugin_data_owner_version"));
    }
    Ok(())
}

fn require_owned(target: &ResolvedTarget, code: &'static str) -> Result<(), PluginDataError> {
    if target.owned_collection {
        Ok(())
    } else {
        Err(ownership_error(code))
    }
}

async fn select_rows(
    transaction: &mut Transaction<'_, Postgres>,
    workspace_id: Uuid,
    target: &ResolvedTarget,
    requested_fields: &[String],
    filters: &[PluginDataFilter],
    order: &[PluginDataOrder],
    page: Option<(u32, u64)>,
) -> Result<Vec<PluginDataRow>, PluginDataError> {
    let fields = resolve_fields(target, requested_fields, false)?;
    let mut query = QueryBuilder::<Postgres>::new("select ");
    {
        let mut separated = query.separated(", ");
        for (_, field) in &fields {
            let physical = quoted(&field.physical)?;
            if field.field_type == "number" {
                separated.push(format!("({physical})::text as {physical}"));
            } else {
                separated.push(physical);
            }
        }
    }
    query
        .push(" from ")
        .push(quoted(&target.table)?)
        .push(" where ");
    push_scope(&mut query, workspace_id);
    push_filters(&mut query, target, filters)?;
    if !order.is_empty() {
        query.push(" order by ");
        let mut separated = query.separated(", ");
        for item in order {
            let field = target
                .fields
                .get(&item.field)
                .ok_or_else(|| ownership_error("plugin_data_order_field"))?;
            separated
                .push(quoted(&field.physical)?)
                .push_unseparated(match item.direction {
                    PluginDataOrderDirection::Ascending => " asc",
                    PluginDataOrderDirection::Descending => " desc",
                });
        }
    }
    if let Some((limit, offset)) = page {
        query
            .push(" limit ")
            .push_bind(i64::from(limit))
            .push(" offset ")
            .push_bind(
                i64::try_from(offset).map_err(|_| PluginDataError::invalid("plugin_data_page"))?,
            );
    }
    let rows = query
        .build()
        .fetch_all(&mut **transaction)
        .await
        .map_err(storage_error)?;
    rows.into_iter().map(|row| map_row(row, &fields)).collect()
}

async fn count_rows(
    transaction: &mut Transaction<'_, Postgres>,
    workspace_id: Uuid,
    target: &ResolvedTarget,
    filters: &[PluginDataFilter],
) -> Result<u64, PluginDataError> {
    let mut query = QueryBuilder::<Postgres>::new("select count(*)::bigint from ");
    query.push(quoted(&target.table)?).push(" where ");
    push_scope(&mut query, workspace_id);
    push_filters(&mut query, target, filters)?;
    let count: i64 = query
        .build_query_scalar()
        .fetch_one(&mut **transaction)
        .await
        .map_err(storage_error)?;
    u64::try_from(count).map_err(|_| storage_error(anyhow::anyhow!("negative count")))
}

async fn insert_owned(
    transaction: &mut Transaction<'_, Postgres>,
    workspace_id: Uuid,
    target: &ResolvedTarget,
    values: &BTreeMap<String, PluginDataValue>,
) -> Result<u64, PluginDataError> {
    let values = resolve_values(target, values, false)?;
    let mut query = QueryBuilder::<Postgres>::new("insert into ");
    query.push(quoted(&target.table)?).push(" (id, scope_id");
    for (_, field, _) in &values {
        query.push(", ").push(quoted(&field.physical)?);
    }
    query
        .push(") values (")
        .push_bind(Uuid::now_v7())
        .push(", ")
        .push_bind(workspace_id);
    for (_, _, value) in &values {
        query.push(", ");
        push_value(&mut query, value)?;
    }
    query.push(")");
    Ok(query
        .build()
        .execute(&mut **transaction)
        .await
        .map_err(storage_error)?
        .rows_affected())
}

async fn update_rows(
    transaction: &mut Transaction<'_, Postgres>,
    workspace_id: Uuid,
    target: &ResolvedTarget,
    filters: &[PluginDataFilter],
    values: &BTreeMap<String, PluginDataValue>,
) -> Result<u64, PluginDataError> {
    let values = resolve_values(target, values, false)?;
    let mut query = QueryBuilder::<Postgres>::new("update ");
    query.push(quoted(&target.table)?).push(" set ");
    {
        let mut separated = query.separated(", ");
        for (_, field, value) in &values {
            separated
                .push(quoted(&field.physical)?)
                .push_unseparated(" = ");
            push_value_separated(&mut separated, value)?;
        }
        if target.owned_collection {
            separated.push("updated_at = now()");
        }
    }
    query.push(" where ");
    push_scope(&mut query, workspace_id);
    push_filters(&mut query, target, filters)?;
    Ok(query
        .build()
        .execute(&mut **transaction)
        .await
        .map_err(storage_error)?
        .rows_affected())
}

async fn delete_rows(
    transaction: &mut Transaction<'_, Postgres>,
    workspace_id: Uuid,
    target: &ResolvedTarget,
    filters: &[PluginDataFilter],
) -> Result<u64, PluginDataError> {
    let mut query = QueryBuilder::<Postgres>::new("delete from ");
    query.push(quoted(&target.table)?).push(" where ");
    push_scope(&mut query, workspace_id);
    push_filters(&mut query, target, filters)?;
    Ok(query
        .build()
        .execute(&mut **transaction)
        .await
        .map_err(storage_error)?
        .rows_affected())
}

async fn upsert_owned(
    transaction: &mut Transaction<'_, Postgres>,
    workspace_id: Uuid,
    target: &ResolvedTarget,
    identity: &BTreeMap<String, PluginDataValue>,
    values: &BTreeMap<String, PluginDataValue>,
) -> Result<u64, PluginDataError> {
    let lock_material = serde_json::to_string(&(target.table.as_str(), workspace_id, identity))
        .map_err(|_| PluginDataError::invalid("plugin_data_identity"))?;
    sqlx::query("select pg_advisory_xact_lock(hashtextextended($1, 0))")
        .bind(lock_material)
        .execute(&mut **transaction)
        .await
        .map_err(storage_error)?;
    let filters = identity
        .iter()
        .map(|(field, value)| PluginDataFilter {
            field: field.clone(),
            operator: PluginDataFilterOperator::Equal,
            value: Some(value.clone()),
        })
        .collect::<Vec<_>>();
    let affected = update_rows(transaction, workspace_id, target, &filters, values).await?;
    if affected > 0 {
        return Ok(affected);
    }
    let mut merged = identity.clone();
    for (field, value) in values {
        merged.insert(field.clone(), value.clone());
    }
    insert_owned(transaction, workspace_id, target, &merged).await
}

fn resolve_fields<'a>(
    target: &'a ResolvedTarget,
    requested: &'a [String],
    mutable: bool,
) -> Result<Vec<(&'a str, &'a ResolvedField)>, PluginDataError> {
    requested
        .iter()
        .map(|logical| {
            let field = target
                .fields
                .get(logical)
                .ok_or_else(|| ownership_error("plugin_data_field_ownership"))?;
            if mutable && matches!(logical.as_str(), "id" | "created_at" | "updated_at") {
                return Err(ownership_error("plugin_data_immutable_field"));
            }
            Ok((logical.as_str(), field))
        })
        .collect()
}

fn resolve_values<'a>(
    target: &'a ResolvedTarget,
    values: &'a BTreeMap<String, PluginDataValue>,
    allow_identity: bool,
) -> Result<Vec<(&'a str, &'a ResolvedField, &'a PluginDataValue)>, PluginDataError> {
    values
        .iter()
        .map(|(logical, value)| {
            let field = target
                .fields
                .get(logical)
                .ok_or_else(|| ownership_error("plugin_data_field_ownership"))?;
            if !allow_identity && matches!(logical.as_str(), "id" | "created_at" | "updated_at") {
                return Err(ownership_error("plugin_data_immutable_field"));
            }
            validate_value_type(value, &field.field_type)?;
            Ok((logical.as_str(), field, value))
        })
        .collect()
}

fn push_scope(query: &mut QueryBuilder<'_, Postgres>, workspace_id: Uuid) {
    query.push("scope_id = ").push_bind(workspace_id);
}

fn push_filters(
    query: &mut QueryBuilder<'_, Postgres>,
    target: &ResolvedTarget,
    filters: &[PluginDataFilter],
) -> Result<(), PluginDataError> {
    for filter in filters {
        let field = target
            .fields
            .get(&filter.field)
            .ok_or_else(|| ownership_error("plugin_data_filter_field"))?;
        query.push(" and ").push(quoted(&field.physical)?);
        match filter.operator {
            PluginDataFilterOperator::IsNull => {
                query.push(" is null");
            }
            PluginDataFilterOperator::IsNotNull => {
                query.push(" is not null");
            }
            operator => {
                query.push(match operator {
                    PluginDataFilterOperator::Equal => " = ",
                    PluginDataFilterOperator::NotEqual => " <> ",
                    PluginDataFilterOperator::LessThan => " < ",
                    PluginDataFilterOperator::LessThanOrEqual => " <= ",
                    PluginDataFilterOperator::GreaterThan => " > ",
                    PluginDataFilterOperator::GreaterThanOrEqual => " >= ",
                    _ => unreachable!(),
                });
                let value = filter
                    .value
                    .as_ref()
                    .ok_or_else(|| PluginDataError::invalid("plugin_data_filter_value"))?;
                validate_value_type(value, &field.field_type)?;
                push_value(query, value)?;
            }
        }
    }
    Ok(())
}

fn push_value(
    query: &mut QueryBuilder<'_, Postgres>,
    value: &PluginDataValue,
) -> Result<(), PluginDataError> {
    match value {
        PluginDataValue::Null => {
            query.push("null");
        }
        PluginDataValue::String(value) => {
            query.push_bind(value.clone());
        }
        PluginDataValue::Number(value) => {
            value
                .parse::<f64>()
                .map_err(|_| PluginDataError::invalid("plugin_data_number"))?;
            query.push_bind(value.clone()).push("::numeric");
        }
        PluginDataValue::Boolean(value) => {
            query.push_bind(*value);
        }
        PluginDataValue::Datetime(value) => {
            query.push_bind(
                OffsetDateTime::parse(value, &Rfc3339)
                    .map_err(|_| PluginDataError::invalid("plugin_data_datetime"))?,
            );
        }
        PluginDataValue::Json(value) => {
            query.push_bind(value.clone());
        }
        PluginDataValue::Uuid(value) => {
            query.push_bind(
                Uuid::parse_str(value).map_err(|_| PluginDataError::invalid("plugin_data_uuid"))?,
            );
        }
    };
    Ok(())
}

fn push_value_separated(
    separated: &mut sqlx::query_builder::Separated<'_, '_, Postgres, &str>,
    value: &PluginDataValue,
) -> Result<(), PluginDataError> {
    match value {
        PluginDataValue::Null => {
            separated.push_unseparated("null");
        }
        PluginDataValue::String(value) => {
            separated.push_bind_unseparated(value.clone());
        }
        PluginDataValue::Number(value) => {
            value
                .parse::<f64>()
                .map_err(|_| PluginDataError::invalid("plugin_data_number"))?;
            separated
                .push_bind_unseparated(value.clone())
                .push_unseparated("::numeric");
        }
        PluginDataValue::Boolean(value) => {
            separated.push_bind_unseparated(*value);
        }
        PluginDataValue::Datetime(value) => {
            separated.push_bind_unseparated(
                OffsetDateTime::parse(value, &Rfc3339)
                    .map_err(|_| PluginDataError::invalid("plugin_data_datetime"))?,
            );
        }
        PluginDataValue::Json(value) => {
            separated.push_bind_unseparated(value.clone());
        }
        PluginDataValue::Uuid(value) => {
            separated.push_bind_unseparated(
                Uuid::parse_str(value).map_err(|_| PluginDataError::invalid("plugin_data_uuid"))?,
            );
        }
    };
    Ok(())
}

fn map_row(
    row: PgRow,
    fields: &[(&str, &ResolvedField)],
) -> Result<PluginDataRow, PluginDataError> {
    let mut values = BTreeMap::new();
    for (logical, field) in fields {
        let raw = row
            .try_get_raw(field.physical.as_str())
            .map_err(storage_error)?;
        let value = if raw.is_null() {
            PluginDataValue::Null
        } else {
            match field.field_type.as_str() {
                "string" | "text" => PluginDataValue::String(
                    row.try_get(field.physical.as_str())
                        .map_err(storage_error)?,
                ),
                "number" => PluginDataValue::Number(
                    row.try_get::<String, _>(field.physical.as_str())
                        .map_err(storage_error)?,
                ),
                "boolean" => PluginDataValue::Boolean(
                    row.try_get(field.physical.as_str())
                        .map_err(storage_error)?,
                ),
                "datetime" => PluginDataValue::Datetime(
                    row.try_get::<OffsetDateTime, _>(field.physical.as_str())
                        .map_err(storage_error)?
                        .format(&Rfc3339)
                        .map_err(storage_error)?,
                ),
                "json" => PluginDataValue::Json(
                    row.try_get(field.physical.as_str())
                        .map_err(storage_error)?,
                ),
                "uuid" => PluginDataValue::Uuid(
                    row.try_get::<Uuid, _>(field.physical.as_str())
                        .map_err(storage_error)?
                        .to_string(),
                ),
                _ => return Err(ownership_error("plugin_data_field_type")),
            }
        };
        values.insert((*logical).to_string(), value);
    }
    Ok(PluginDataRow { values })
}

fn validate_value_type(value: &PluginDataValue, field_type: &str) -> Result<(), PluginDataError> {
    if matches!(value, PluginDataValue::Null) {
        return Ok(());
    }
    let matches = matches!(
        (value, field_type),
        (PluginDataValue::String(_), "string" | "text")
            | (PluginDataValue::Number(_), "number")
            | (PluginDataValue::Boolean(_), "boolean")
            | (PluginDataValue::Datetime(_), "datetime")
            | (PluginDataValue::Json(_), "json")
            | (PluginDataValue::Uuid(_), "uuid")
    );
    if matches {
        Ok(())
    } else {
        Err(PluginDataError::invalid("plugin_data_value_type"))
    }
}

async fn load_receipt(
    transaction: &mut Transaction<'_, Postgres>,
    owner_id: &str,
    workspace_id: Uuid,
    provider_instance_id: &str,
    key: &str,
) -> Result<Option<(String, PluginDataResponse)>, PluginDataError> {
    let row = sqlx::query(
        "select request_hash, response from plugin_data_idempotency_receipts where owner_id = $1 and workspace_id = $2 and provider_instance_id = $3 and idempotency_key = $4 for update",
    )
    .bind(owner_id).bind(workspace_id).bind(provider_instance_id).bind(key)
    .fetch_optional(&mut **transaction).await.map_err(storage_error)?;
    row.map(|row| {
        let hash = row.try_get("request_hash").map_err(storage_error)?;
        let value: serde_json::Value = row.try_get("response").map_err(storage_error)?;
        let response = serde_json::from_value(value).map_err(storage_error)?;
        Ok((hash, response))
    })
    .transpose()
}

async fn store_receipt(
    transaction: &mut Transaction<'_, Postgres>,
    owner_id: &str,
    workspace_id: Uuid,
    provider_instance_id: &str,
    key: &str,
    request_hash: &str,
    response: &PluginDataResponse,
) -> Result<(), PluginDataError> {
    let response = serde_json::to_value(response).map_err(storage_error)?;
    sqlx::query(
        "insert into plugin_data_idempotency_receipts (owner_id, workspace_id, provider_instance_id, idempotency_key, request_hash, response) values ($1, $2, $3, $4, $5, $6)",
    )
    .bind(owner_id).bind(workspace_id).bind(provider_instance_id).bind(key).bind(request_hash).bind(response)
    .execute(&mut **transaction).await.map_err(|database_error| {
        if database_error.as_database_error().is_some_and(|error| error.is_unique_violation()) {
            error(PluginDataErrorKind::Conflict, "plugin_data_idempotency_race", true)
        } else { storage_error(database_error) }
    })?;
    Ok(())
}

fn quoted(identifier: &str) -> Result<String, PluginDataError> {
    if identifier.is_empty()
        || identifier.len() > 63
        || !identifier.chars().all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
        })
    {
        return Err(ownership_error("plugin_data_physical_identifier"));
    }
    Ok(format!("\"{identifier}\""))
}

fn hex_digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn ownership_error(code: &'static str) -> PluginDataError {
    error(PluginDataErrorKind::OwnershipDenied, code, false)
}

fn storage_error(source: impl std::fmt::Display) -> PluginDataError {
    tracing::warn!(error = %source, "plugin data storage operation failed");
    error(
        PluginDataErrorKind::StorageUnavailable,
        "plugin_data_storage",
        true,
    )
}

fn error(kind: PluginDataErrorKind, code: &'static str, retryable: bool) -> PluginDataError {
    PluginDataError {
        kind,
        code: code.to_string(),
        retryable,
    }
}
