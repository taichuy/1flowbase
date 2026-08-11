use domain::MigrationHead;
use sha2::{Digest, Sha256};
use sqlx::PgPool;

use super::PostgreSqlBackupError;

pub async fn migration_head(pool: &PgPool) -> Result<MigrationHead, PostgreSqlBackupError> {
    let rows = sqlx::query_as::<_, (i64, Vec<u8>)>(
        "select version, checksum from _sqlx_migrations where success = true order by version",
    )
    .fetch_all(pool)
    .await?;
    let mut hasher = Sha256::new();
    for (version, checksum) in rows {
        hasher.update(version.to_le_bytes());
        hasher.update((checksum.len() as u64).to_le_bytes());
        hasher.update(checksum);
    }
    let fingerprint = hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    MigrationHead::try_from(fingerprint).map_err(|_| PostgreSqlBackupError::InvalidToolVersion)
}
