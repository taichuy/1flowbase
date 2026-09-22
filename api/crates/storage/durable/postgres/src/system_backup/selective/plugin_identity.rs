//! Logical foreign-key identities are portable; installation policy and trust are target-owned.
use super::{archive::Column, schema};
use anyhow::{Context, Result};
use serde_json::Value;
use sqlx::PgConnection;

const FIELDS: &[&str] = &[
    "id",
    "scope_id",
    "category",
    "organization",
    "artifact_id",
    "artifact_version",
    "plugin_id",
    "contract_version",
    "protocol",
    "display_name",
    "created_by",
    "created_at",
];
const IDENTITY: &[&str] = &[
    "id",
    "scope_id",
    "category",
    "organization",
    "artifact_id",
    "artifact_version",
    "plugin_id",
    "contract_version",
    "protocol",
];
pub(super) fn columns(columns: Vec<Column>) -> Vec<Column> {
    columns
        .into_iter()
        .filter(|column| FIELDS.contains(&column.name.as_str()))
        .collect()
}
pub(super) fn row(mut row: Value) -> Result<Value> {
    row.as_object_mut()
        .context("invalid plugin identity")?
        .retain(|key, _| FIELDS.contains(&key.as_str()));
    Ok(row)
}
/// Returns the current logical identity only; no policy values enter the archive/preview.
pub(super) async fn existing(
    connection: &mut PgConnection,
    incoming: &Value,
) -> Result<Option<Value>> {
    let row = sqlx::query_scalar::<_, Value>(
        "select to_jsonb(t) from extension_installations t where id=($1->>'id')::uuid",
    )
    .bind(incoming)
    .fetch_optional(connection)
    .await?;
    row.map(self::row).transpose()
}
pub(super) fn compatible(existing: &Value, incoming: &Value) -> bool {
    IDENTITY
        .iter()
        .all(|field| existing.get(*field) == incoming.get(*field))
}
pub(super) fn insert_sql() -> Result<String> {
    let fields = FIELDS
        .iter()
        .map(|field| schema::quote(field))
        .collect::<Result<Vec<_>>>()?;
    let values = fields
        .iter()
        .map(|field| format!("r.{field}"))
        .collect::<Vec<_>>()
        .join(",");
    // A previously absent identity never claims the source's local verification or activation.
    // Existing identities remain entirely unchanged, including display, policy, and attribution.
    Ok(format!("insert into extension_installations ({},source_kind,trust_level,verification_status,desired_state,signature_status) select {values},'uploaded','unverified',case when r.plugin_id is null then null else 'pending' end,case when r.plugin_id is null then null else 'disabled' end,'missing' from jsonb_populate_record(null::extension_installations,$1) r where true on conflict (id) do nothing",fields.join(",")))
}
