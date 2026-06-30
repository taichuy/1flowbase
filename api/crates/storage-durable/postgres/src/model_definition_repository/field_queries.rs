use std::collections::{HashMap, HashSet};

use anyhow::Result;
use sqlx::{Postgres, Row, Transaction};
use uuid::Uuid;

use crate::{
    mappers::model_field_mapper::{PgModelFieldMapper, StoredModelFieldRow},
    physical_schema_repository::join_table_name,
};

pub(super) async fn insert_model_field(
    tx: &mut Transaction<'_, Postgres>,
    field: &domain::ModelFieldRecord,
    actor_user_id: Option<Uuid>,
    availability_status: domain::MetadataAvailabilityStatus,
) -> Result<()> {
    sqlx::query(
        r#"
        insert into model_fields (
            id,
            data_model_id,
            code,
            title,
            description,
            physical_column_name,
            external_field_key,
            field_kind,
            is_system,
            is_writable,
            is_required,
            api_required,
            is_unique,
            default_value,
            display_interface,
            display_options,
            relation_target_model_id,
            relation_options,
            sort_order,
            availability_status,
            scope_id,
            created_by,
            updated_by
        )
        values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, (select scope_id from model_definitions where id = $2), $21, $21)
        "#,
    )
    .bind(field.id)
    .bind(field.data_model_id)
    .bind(&field.code)
    .bind(&field.title)
    .bind(&field.description)
    .bind(&field.physical_column_name)
    .bind(&field.external_field_key)
    .bind(field.field_kind.as_str())
    .bind(field.is_system)
    .bind(field.is_writable)
    .bind(field.is_required)
    .bind(field.api_required)
    .bind(field.is_unique)
    .bind(&field.default_value)
    .bind(&field.display_interface)
    .bind(&field.display_options)
    .bind(field.relation_target_model_id)
    .bind(&field.relation_options)
    .bind(field.sort_order)
    .bind(availability_status.as_str())
    .bind(actor_user_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub(super) async fn load_fields_by_model_id(
    pool: &sqlx::PgPool,
) -> Result<HashMap<Uuid, Vec<domain::ModelFieldRecord>>> {
    let rows = sqlx::query(
        r#"
        select
            id,
            data_model_id,
            code,
            title,
            description,
            physical_column_name,
            external_field_key,
            field_kind,
            is_system,
            is_writable,
            is_required,
            api_required,
            is_unique,
            default_value,
            display_interface,
            display_options,
            relation_target_model_id,
            relation_options,
            sort_order,
            availability_status
        from model_fields
        order by data_model_id asc, sort_order asc, created_at asc
        "#,
    )
    .fetch_all(pool)
    .await?;

    Ok(group_field_rows(rows))
}

pub(super) async fn load_fields_for_model(
    tx: &mut Transaction<'_, Postgres>,
    model_id: Uuid,
) -> Result<Vec<domain::ModelFieldRecord>> {
    let rows = sqlx::query(
        r#"
        select
            id,
            data_model_id,
            code,
            title,
            description,
            physical_column_name,
            external_field_key,
            field_kind,
            is_system,
            is_writable,
            is_required,
            api_required,
            is_unique,
            default_value,
            display_interface,
            display_options,
            relation_target_model_id,
            relation_options,
            sort_order,
            availability_status
        from model_fields
        where data_model_id = $1
        order by sort_order asc, created_at asc
        "#,
    )
    .bind(model_id)
    .fetch_all(&mut **tx)
    .await?;

    Ok(group_field_rows(rows).remove(&model_id).unwrap_or_default())
}

pub(super) async fn load_model_field_for_update(
    tx: &mut Transaction<'_, Postgres>,
    model_id: Uuid,
    field_id: Uuid,
) -> Result<Option<domain::ModelFieldRecord>> {
    let row = sqlx::query(
        r#"
        select
            id,
            data_model_id,
            code,
            title,
            description,
            physical_column_name,
            external_field_key,
            field_kind,
            is_system,
            is_writable,
            is_required,
            api_required,
            is_unique,
            default_value,
            display_interface,
            display_options,
            relation_target_model_id,
            relation_options,
            sort_order,
            availability_status
        from model_fields
        where data_model_id = $1
          and id = $2
        for update
        "#,
    )
    .bind(model_id)
    .bind(field_id)
    .fetch_optional(&mut **tx)
    .await?;

    Ok(row.map(to_model_field_record))
}

pub(super) async fn load_join_tables_for_model(
    tx: &mut Transaction<'_, Postgres>,
    model_id: Uuid,
) -> Result<Vec<String>> {
    let rows = sqlx::query(
        r#"
        select
            owner.id as owner_id,
            owner.code as owner_code,
            target.id as target_id,
            target.code as target_code
        from model_fields fields
        join model_definitions owner on owner.id = fields.data_model_id
        join model_definitions target on target.id = fields.relation_target_model_id
        where fields.field_kind = 'many_to_many'
          and (fields.data_model_id = $1 or fields.relation_target_model_id = $1)
        "#,
    )
    .bind(model_id)
    .fetch_all(&mut **tx)
    .await?;

    let mut table_names = HashSet::new();
    for row in rows {
        table_names.insert(join_table_name(
            row.get::<String, _>("owner_code").as_str(),
            row.get("owner_id"),
            row.get::<String, _>("target_code").as_str(),
            row.get("target_id"),
        ));
    }

    Ok(table_names.into_iter().collect())
}

pub(super) async fn insert_model_field_after_failure(
    pool: &sqlx::PgPool,
    field: &domain::ModelFieldRecord,
    actor_user_id: Option<Uuid>,
    availability_status: domain::MetadataAvailabilityStatus,
) -> Result<()> {
    sqlx::query(
        r#"
        insert into model_fields (
            id,
            data_model_id,
            code,
            title,
            description,
            physical_column_name,
            external_field_key,
            field_kind,
            is_system,
            is_writable,
            is_required,
            api_required,
            is_unique,
            default_value,
            display_interface,
            display_options,
            relation_target_model_id,
            relation_options,
            sort_order,
            availability_status,
            scope_id,
            created_by,
            updated_by
        )
        values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, (select scope_id from model_definitions where id = $2), $21, $21)
        on conflict (id) do update
        set availability_status = excluded.availability_status,
            updated_by = excluded.updated_by,
            updated_at = now()
        "#,
    )
    .bind(field.id)
    .bind(field.data_model_id)
    .bind(&field.code)
    .bind(&field.title)
    .bind(&field.description)
    .bind(&field.physical_column_name)
    .bind(&field.external_field_key)
    .bind(field.field_kind.as_str())
    .bind(field.is_system)
    .bind(field.is_writable)
    .bind(field.is_required)
    .bind(field.api_required)
    .bind(field.is_unique)
    .bind(&field.default_value)
    .bind(&field.display_interface)
    .bind(&field.display_options)
    .bind(field.relation_target_model_id)
    .bind(&field.relation_options)
    .bind(field.sort_order)
    .bind(availability_status.as_str())
    .bind(actor_user_id)
    .execute(pool)
    .await?;
    Ok(())
}

fn group_field_rows(
    rows: Vec<sqlx::postgres::PgRow>,
) -> HashMap<Uuid, Vec<domain::ModelFieldRecord>> {
    let mut fields_by_model_id = HashMap::new();
    for row in rows {
        let field = to_model_field_record(row);
        fields_by_model_id
            .entry(field.data_model_id)
            .or_insert_with(Vec::new)
            .push(field);
    }
    fields_by_model_id
}

fn to_model_field_record(row: sqlx::postgres::PgRow) -> domain::ModelFieldRecord {
    PgModelFieldMapper::to_model_field_record(StoredModelFieldRow {
        id: row.get("id"),
        data_model_id: row.get("data_model_id"),
        code: row.get("code"),
        title: row.get("title"),
        description: row.get("description"),
        physical_column_name: row.get("physical_column_name"),
        external_field_key: row.get("external_field_key"),
        field_kind: row.get("field_kind"),
        is_system: row.get("is_system"),
        is_writable: row.get("is_writable"),
        is_required: row.get("is_required"),
        api_required: row.get("api_required"),
        is_unique: row.get("is_unique"),
        default_value: row.get("default_value"),
        display_interface: row.get("display_interface"),
        display_options: row.get("display_options"),
        relation_target_model_id: row.get("relation_target_model_id"),
        relation_options: row.get("relation_options"),
        sort_order: row.get("sort_order"),
        availability_status: row.get("availability_status"),
    })
}
