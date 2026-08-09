use anyhow::Result;
use sqlx::{Postgres, Transaction};

const TEMPLATE_PROVIDER: &str = "core";
const TEMPLATE_CODE: &str = "ordered_tree";
const TEMPLATE_VERSION: &str = "v1";

pub(crate) fn matches(model: &domain::ModelDefinitionRecord) -> bool {
    model.template_provider == TEMPLATE_PROVIDER
        && model.template_code == TEMPLATE_CODE
        && model.template_version == TEMPLATE_VERSION
}

pub(crate) fn system_field_records(
    model: &domain::ModelDefinitionRecord,
) -> Vec<domain::ModelFieldRecord> {
    if !matches(model) {
        return Vec::new();
    }

    [
        ("parent_id", domain::ModelFieldKind::ManyToOne, false),
        ("sibling_rank", domain::ModelFieldKind::String, true),
    ]
    .into_iter()
    .enumerate()
    .map(
        |(offset, (code, field_kind, is_required))| domain::ModelFieldRecord {
            id: uuid::Uuid::now_v7(),
            data_model_id: model.id,
            code: code.to_owned(),
            title: code.to_owned(),
            description: None,
            physical_column_name: code.to_owned(),
            external_field_key: None,
            field_kind,
            is_system: true,
            is_writable: false,
            is_required,
            api_required: false,
            is_unique: false,
            default_value: None,
            display_interface: None,
            display_options: serde_json::json!({}),
            relation_target_model_id: None,
            relation_options: serde_json::json!({}),
            sort_order: 6 + offset as i32,
            availability_status: domain::MetadataAvailabilityStatus::Available,
        },
    )
    .collect()
}

pub(crate) async fn create_table(
    tx: &mut Transaction<'_, Postgres>,
    model: &domain::ModelDefinitionRecord,
) -> Result<()> {
    let table_name = quoted_identifier(&model.physical_table_name)?;
    let primary_key = quoted_identifier(&object_name("pk_ot", model.id))?;
    let scope_id_unique = quoted_identifier(&object_name("uq_ot_scope_id", model.id))?;
    let parent_not_self = quoted_identifier(&object_name("ck_ot_parent_self", model.id))?;
    let parent_foreign_key = quoted_identifier(&object_name("fk_ot_parent", model.id))?;
    let sibling_index = quoted_identifier(&object_name("idx_ot_siblings", model.id))?;
    let sibling_unique = quoted_identifier(&object_name("uq_ot_sibling", model.id))?;
    let root_unique = quoted_identifier(&object_name("uq_ot_root_rank", model.id))?;

    let statement = format!(
        r#"
        create table {table_name} (
          id uuid constraint {primary_key} primary key,
          created_at timestamptz not null default now(),
          updated_at timestamptz not null default now(),
          created_by uuid,
          updated_by uuid,
          scope_id uuid not null,
          parent_id uuid,
          sibling_rank text collate "C" not null,
          constraint {scope_id_unique} unique (scope_id, id),
          constraint {parent_not_self} check (parent_id is null or parent_id <> id),
          constraint {parent_foreign_key} foreign key (scope_id, parent_id)
            references {table_name} (scope_id, id) on delete restrict
        )
        "#
    );
    sqlx::query(&statement).execute(&mut **tx).await?;

    sqlx::query(&format!(
        "create index {sibling_index} on {table_name} (scope_id, parent_id, sibling_rank, id)"
    ))
    .execute(&mut **tx)
    .await?;
    sqlx::query(&format!(
        "create unique index {sibling_unique} on {table_name} (scope_id, parent_id, sibling_rank) where parent_id is not null"
    ))
    .execute(&mut **tx)
    .await?;
    sqlx::query(&format!(
        "create unique index {root_unique} on {table_name} (scope_id, sibling_rank) where parent_id is null"
    ))
    .execute(&mut **tx)
    .await?;
    Ok(())
}

fn object_name(prefix: &str, model_id: uuid::Uuid) -> String {
    format!("{prefix}_{}", model_id.simple())
}

fn quoted_identifier(value: &str) -> Result<String> {
    if value.len() > 63
        || !value
            .chars()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_')
    {
        return Err(anyhow::anyhow!("invalid ordered-tree SQL identifier"));
    }
    Ok(format!("\"{value}\""))
}
