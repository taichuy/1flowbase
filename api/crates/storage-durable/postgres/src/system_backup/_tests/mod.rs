use super::{parse_major_version, PostgreSqlBackupError};

#[test]
fn parses_supported_postgresql_tool_versions() {
    assert_eq!(
        parse_major_version("pg_dump (PostgreSQL) 17.4").unwrap(),
        17
    );
    assert_eq!(
        parse_major_version("pg_restore (PostgreSQL) 16.9").unwrap(),
        16
    );
}

#[test]
fn rejects_missing_or_invalid_postgresql_tool_version() {
    assert!(matches!(
        parse_major_version("pg_dump unknown"),
        Err(PostgreSqlBackupError::InvalidToolVersion)
    ));
}
