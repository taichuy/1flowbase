use anyhow::{anyhow, Result};
use sqlx::{PgPool, Row};
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordAppliedExtensionMigrationInput {
    pub extension_id: String,
    pub plugin_version: String,
    pub migration_id: String,
    pub checksum: String,
    pub package_fingerprint: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppliedExtensionMigrationRecord {
    pub id: Uuid,
    pub extension_id: String,
    pub plugin_version: String,
    pub migration_id: String,
    pub checksum: String,
    pub package_fingerprint: String,
    pub applied_at: OffsetDateTime,
}

#[derive(Debug, Clone)]
pub struct HostExtensionMigrationRepository {
    pool: PgPool,
}

impl HostExtensionMigrationRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn record_applied_extension_migration(
        &self,
        input: &RecordAppliedExtensionMigrationInput,
    ) -> Result<AppliedExtensionMigrationRecord> {
        validate_non_empty(&input.extension_id, "extension_id")?;
        validate_non_empty(&input.plugin_version, "plugin_version")?;
        validate_non_empty(&input.migration_id, "migration_id")?;
        validate_non_empty(&input.checksum, "checksum")?;
        validate_non_empty(&input.package_fingerprint, "package_fingerprint")?;

        if let Some(existing) = self
            .get_applied_extension_migration(&input.extension_id, &input.migration_id)
            .await?
        {
            if existing.checksum != input.checksum {
                return Err(anyhow!(
                    "checksum mismatch for host extension migration {}:{}",
                    input.extension_id,
                    input.migration_id
                ));
            }
            return Ok(existing);
        }

        let row = sqlx::query(
            r#"
            insert into host_extension_migrations (
                id,
                scope_id,
                extension_id,
                plugin_version,
                migration_id,
                checksum,
                package_fingerprint
            ) values (
                $1, $2, $3, $4, $5, $6, $7
            )
            returning
                id,
                extension_id,
                plugin_version,
                migration_id,
                checksum,
                package_fingerprint,
                applied_at
            "#,
        )
        .bind(Uuid::now_v7())
        .bind(domain::SYSTEM_SCOPE_ID)
        .bind(&input.extension_id)
        .bind(&input.plugin_version)
        .bind(&input.migration_id)
        .bind(&input.checksum)
        .bind(&input.package_fingerprint)
        .fetch_one(&self.pool)
        .await?;

        applied_extension_migration_from_row(row)
    }

    pub async fn get_applied_extension_migration(
        &self,
        extension_id: &str,
        migration_id: &str,
    ) -> Result<Option<AppliedExtensionMigrationRecord>> {
        let row = sqlx::query(
            r#"
            select
                id,
                extension_id,
                plugin_version,
                migration_id,
                checksum,
                package_fingerprint,
                applied_at
            from host_extension_migrations
            where extension_id = $1
              and migration_id = $2
            "#,
        )
        .bind(extension_id)
        .bind(migration_id)
        .fetch_optional(&self.pool)
        .await?;

        row.map(applied_extension_migration_from_row).transpose()
    }

    pub fn ensure_extension_table_name(extension_id: &str, table_name: &str) -> Result<()> {
        validate_non_empty(extension_id, "extension_id")?;
        validate_non_empty(table_name, "table_name")?;
        let expected_prefix = format!(
            "ext_{}__",
            normalize_extension_id_for_table_namespace(extension_id)
        );
        if !table_name.starts_with(&expected_prefix) {
            return Err(anyhow!(
                "host extension table name must start with {expected_prefix}"
            ));
        }
        Ok(())
    }
}

fn applied_extension_migration_from_row(
    row: sqlx::postgres::PgRow,
) -> Result<AppliedExtensionMigrationRecord> {
    Ok(AppliedExtensionMigrationRecord {
        id: row.get("id"),
        extension_id: row.get("extension_id"),
        plugin_version: row.get("plugin_version"),
        migration_id: row.get("migration_id"),
        checksum: row.get("checksum"),
        package_fingerprint: row.get("package_fingerprint"),
        applied_at: row.get("applied_at"),
    })
}

fn normalize_extension_id_for_table_namespace(extension_id: &str) -> String {
    extension_id
        .to_ascii_lowercase()
        .chars()
        .map(|character| {
            if character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_' {
                character
            } else {
                '_'
            }
        })
        .collect()
}

fn validate_non_empty(value: &str, field: &str) -> Result<()> {
    if value.trim().is_empty() {
        return Err(anyhow!("{field} must not be empty"));
    }
    Ok(())
}
