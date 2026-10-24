//! Host-managed relation sets share the source record's write transaction and row lock.
use anyhow::{anyhow, Result};
use domain::{ModelFieldKind, ModelFieldRecord};
use serde_json::{Map, Value};
use sqlx::{PgConnection, Postgres, QueryBuilder};
use storage_durable::{
    model_metadata::ModelMetadata, runtime_model_availability::RuntimeModelError,
};
use uuid::Uuid;

use super::quote_identifier;
use crate::physical_schema_repository::join_table_name;

fn invalid(field: &ModelFieldRecord) -> anyhow::Error {
    RuntimeModelError::InvalidOperationField(field.code.clone()).into()
}

async fn relation_tables(
    conn: &mut PgConnection,
    metadata: &ModelMetadata,
    field: &ModelFieldRecord,
) -> Result<(String, String)> {
    let target_id = field
        .relation_target_model_id
        .ok_or_else(|| invalid(field))?;
    // Raw IDs follow the same contract as many_to_one: publication gates expansion,
    // not the source record's stored links (or an empty relation set).
    let target: Option<(String, String)> =
        sqlx::query_as("select code, physical_table_name from model_definitions where id = $1")
            .bind(target_id)
            .fetch_optional(&mut *conn)
            .await?;
    let (code, table) = target.ok_or_else(|| invalid(field))?;
    Ok((
        quote_identifier(&join_table_name(
            &metadata.model_code,
            metadata.model_id,
            &code,
            target_id,
        ))?,
        quote_identifier(&table)?,
    ))
}

/// Omitted fields remain unchanged; supplied arrays replace the set. Never write a virtual column.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn replace_relations(
    conn: &mut PgConnection,
    metadata: &ModelMetadata,
    source_id: Uuid,
    scope_id: Uuid,
    owner_user_id: Option<Uuid>,
    actor_user_id: Option<Uuid>,
    payload: &Map<String, Value>,
) -> Result<()> {
    for field in metadata
        .fields
        .iter()
        .filter(|f| f.field_kind == ModelFieldKind::ManyToMany)
    {
        let Some(value) = payload.get(&field.code) else {
            continue;
        };
        let values = value.as_array().ok_or_else(|| invalid(field))?;
        let mut ids = values
            .iter()
            .map(|v| {
                v.as_str()
                    .and_then(|v| Uuid::parse_str(v).ok())
                    .ok_or_else(|| invalid(field))
            })
            .collect::<Result<Vec<_>>>()?;
        ids.sort_unstable();
        ids.dedup();
        let (relation, target) = relation_tables(conn, metadata, field).await?;
        // Lock targets against deletion until the FK inserts commit. Scope and owner restrictions
        // match runtime record lookup; missing and inaccessible IDs have the same error contract.
        let mut query =
            QueryBuilder::<Postgres>::new(format!("select id from {target} where scope_id = "));
        query
            .push_bind(scope_id)
            .push(" and id = any(")
            .push_bind(ids.clone())
            .push(")");
        if let Some(owner) = owner_user_id {
            query.push(" and created_by = ").push_bind(owner);
        }
        query.push(" order by id for key share");
        let found = query
            .build_query_scalar::<Uuid>()
            .fetch_all(&mut *conn)
            .await?;
        if found.len() != ids.len() {
            return Err(invalid(field));
        }
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "delete from {relation} where left_model_id = $1 and scope_id = $2"
        )))
        .bind(source_id)
        .bind(scope_id)
        .execute(&mut *conn)
        .await?;
        for id in ids {
            sqlx::query(sqlx::AssertSqlSafe(format!("insert into {relation} (id, left_model_id, right_model_id, scope_id, created_by, updated_by) values ($1,$2,$3,$4,$5,$5)")))
                .bind(Uuid::now_v7()).bind(source_id).bind(id).bind(scope_id).bind(actor_user_id).execute(&mut *conn).await?;
        }
    }
    Ok(())
}

/// Batch enrichment keeps tree projections and generic CRUD reads on the same relation contract.
pub(crate) async fn hydrate_relations(
    conn: &mut PgConnection,
    metadata: &ModelMetadata,
    owner_user_id: Option<Uuid>,
    records: &mut [Value],
) -> Result<()> {
    if records.is_empty()
        || !metadata
            .fields
            .iter()
            .any(|f| f.field_kind == ModelFieldKind::ManyToMany)
    {
        return Ok(());
    }
    let ids = records
        .iter()
        .map(|r| {
            r.get("id")
                .and_then(Value::as_str)
                .and_then(|id| Uuid::parse_str(id).ok())
                .ok_or_else(|| anyhow!("runtime record missing id"))
        })
        .collect::<Result<Vec<_>>>()?;
    for field in metadata
        .fields
        .iter()
        .filter(|f| f.field_kind == ModelFieldKind::ManyToMany)
    {
        let (relation, target) = relation_tables(conn, metadata, field).await?;
        let source = quote_identifier(&metadata.physical_table_name)?;
        let mut query = QueryBuilder::<Postgres>::new(format!("select rel.left_model_id, rel.right_model_id from {relation} rel join {source} source on source.id = rel.left_model_id and source.scope_id = rel.scope_id join {target} target on target.id = rel.right_model_id and target.scope_id = rel.scope_id where rel.left_model_id = any("));
        query.push_bind(ids.clone()).push(")");
        if let Some(owner) = owner_user_id {
            query.push(" and target.created_by = ").push_bind(owner);
        }
        query.push(" order by rel.right_model_id");
        let pairs = query
            .build_query_as::<(Uuid, Uuid)>()
            .fetch_all(&mut *conn)
            .await?;
        let mut grouped = std::collections::HashMap::<Uuid, Vec<Value>>::new();
        for (source, target) in pairs {
            grouped
                .entry(source)
                .or_default()
                .push(Value::String(target.to_string()));
        }
        for (record, id) in records.iter_mut().zip(&ids) {
            record
                .as_object_mut()
                .ok_or_else(|| anyhow!("runtime record must be object"))?
                .insert(
                    field.code.clone(),
                    Value::Array(grouped.get(id).cloned().unwrap_or_default()),
                );
        }
    }
    Ok(())
}
