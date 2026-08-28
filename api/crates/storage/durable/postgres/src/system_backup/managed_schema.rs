use sqlx::{PgPool, Row};

use super::PostgreSqlBackupError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagedSchemaBackupObject {
    pub ownership_key: String,
    pub physical_table: String,
    pub physical_column: Option<String>,
    pub active: bool,
}

pub async fn managed_schema_backup_inventory(
    pool: &PgPool,
) -> Result<Vec<ManagedSchemaBackupObject>, PostgreSqlBackupError> {
    let ledger_exists = sqlx::query_scalar::<_, Option<String>>(
        "select to_regclass('plugin_schema_ownership')::text",
    )
    .fetch_one(pool)
    .await?
    .is_some();
    if !ledger_exists {
        return Ok(Vec::new());
    }

    let rows = sqlx::query(
        "select ownership_key, physical_table, physical_column, active from plugin_schema_ownership order by ownership_key",
    )
    .fetch_all(pool)
    .await?;
    let mut inventory = Vec::with_capacity(rows.len());
    for row in rows {
        let object = ManagedSchemaBackupObject {
            ownership_key: row.try_get("ownership_key")?,
            physical_table: row.try_get("physical_table")?,
            physical_column: row.try_get("physical_column")?,
            active: row.try_get("active")?,
        };
        let table_exists = sqlx::query_scalar::<_, Option<String>>("select to_regclass($1)::text")
            .bind(&object.physical_table)
            .fetch_one(pool)
            .await?
            .is_some();
        let column_exists = match object.physical_column.as_deref() {
            Some(column) => {
                sqlx::query_scalar::<_, bool>(
                    "select exists(select 1 from information_schema.columns where table_schema = current_schema() and table_name = $1 and column_name = $2)",
                )
                .bind(&object.physical_table)
                .bind(column)
                .fetch_one(pool)
                .await?
            }
            None => true,
        };
        if !table_exists || !column_exists {
            return Err(PostgreSqlBackupError::ManagedSchemaInventoryInvalid);
        }
        inventory.push(object);
    }
    Ok(inventory)
}
