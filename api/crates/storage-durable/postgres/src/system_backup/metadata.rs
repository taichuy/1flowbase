use std::collections::BTreeSet;

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
    migration_head_from_parts(
        rows.iter()
            .map(|(version, checksum)| (*version, checksum.as_slice())),
    )
}

/// Returns every forward-only migration prefix embedded in this binary.
/// A BackupSet may only be restored when its recorded source head is one of these prefixes.
pub fn supported_migration_heads() -> Result<BTreeSet<MigrationHead>, PostgreSqlBackupError> {
    let mut heads = BTreeSet::new();
    let migrations = sqlx::migrate!("./migrations")
        .iter()
        .filter(|migration| !migration.migration_type.is_down_migration())
        .collect::<Vec<_>>();
    for prefix_len in 1..=migrations.iter().count() {
        heads.insert(migration_head_from_parts(
            migrations
                .iter()
                .take(prefix_len)
                .map(|migration| (migration.version, migration.checksum.as_ref())),
        )?);
    }
    Ok(heads)
}

pub fn current_migration_head() -> Result<MigrationHead, PostgreSqlBackupError> {
    migration_head_from_parts(
        sqlx::migrate!("./migrations")
            .iter()
            .filter(|migration| !migration.migration_type.is_down_migration())
            .map(|migration| (migration.version, migration.checksum.as_ref())),
    )
}

fn migration_head_from_parts<'a>(
    parts: impl IntoIterator<Item = (i64, &'a [u8])>,
) -> Result<MigrationHead, PostgreSqlBackupError> {
    let mut hasher = Sha256::new();
    for (version, checksum) in parts {
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

#[cfg(test)]
mod tests {
    use super::{current_migration_head, supported_migration_heads};

    #[test]
    fn compiled_migration_path_contains_the_current_head() {
        let heads = supported_migration_heads().unwrap();
        let current = current_migration_head().unwrap();

        assert!(!heads.is_empty());
        assert!(heads.contains(&current));
    }
}
