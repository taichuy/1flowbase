pub mod selective;
pub use selective::PgSelectiveBackupRepository;

mod dump;
mod managed_schema;
mod metadata;
mod toolchain;
mod verify;

pub(crate) use dump::PostgreSqlCommandConnection;
pub use dump::*;
pub use managed_schema::*;
pub use metadata::*;
pub use toolchain::*;
pub use verify::*;

#[cfg(test)]
mod _tests;

impl crate::PgControlPlaneStore {
    pub async fn verify_postgres_toolchain(
        &self,
        toolchain: &PostgreSqlToolchain,
    ) -> Result<u32, PostgreSqlBackupError> {
        toolchain.verify_server_compatibility(self.pool()).await
    }

    pub async fn migration_head(&self) -> Result<domain::MigrationHead, PostgreSqlBackupError> {
        migration_head(self.pool()).await
    }

    pub async fn managed_schema_backup_inventory(
        &self,
    ) -> Result<Vec<ManagedSchemaBackupObject>, PostgreSqlBackupError> {
        managed_schema_backup_inventory(self.pool()).await
    }

    pub async fn has_postgres_restore_privileges(&self) -> Result<bool, PostgreSqlBackupError> {
        let granted = sqlx::query_scalar::<_, bool>(
            "select rolsuper or (has_database_privilege(current_user, current_database(), 'CREATE') and pg_has_role(current_user, 'pg_write_all_data', 'MEMBER')) from pg_roles where rolname = current_user",
        )
        .fetch_optional(self.pool())
        .await?
        .unwrap_or(false);
        Ok(granted)
    }
}
