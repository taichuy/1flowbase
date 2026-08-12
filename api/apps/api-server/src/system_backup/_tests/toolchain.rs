use std::{ffi::OsString, path::PathBuf};

use super::super::{
    resolve_postgres_toolchain_source, PostgreSqlToolchainDiscoveryError, PostgreSqlToolchainSource,
};

#[test]
fn explicit_postgres_tool_paths_are_resolved_as_one_pair() {
    let source = resolve_postgres_toolchain_source(
        Some(OsString::from("/opt/postgresql/bin/pg_dump")),
        Some(OsString::from("/opt/postgresql/bin/pg_restore")),
    )
    .unwrap();

    assert_eq!(
        source,
        PostgreSqlToolchainSource::Explicit {
            pg_dump: PathBuf::from("/opt/postgresql/bin/pg_dump"),
            pg_restore: PathBuf::from("/opt/postgresql/bin/pg_restore"),
        }
    );
}

#[test]
fn absent_postgres_tool_paths_fall_back_to_process_path() {
    assert_eq!(
        resolve_postgres_toolchain_source(None, None).unwrap(),
        PostgreSqlToolchainSource::Path
    );
}

#[test]
fn partial_or_empty_postgres_tool_paths_are_rejected() {
    for values in [
        (Some(OsString::from("pg_dump")), None),
        (None, Some(OsString::from("pg_restore"))),
        (Some(OsString::new()), Some(OsString::from("pg_restore"))),
        (Some(OsString::from("pg_dump")), Some(OsString::new())),
    ] {
        assert!(matches!(
            resolve_postgres_toolchain_source(values.0, values.1),
            Err(PostgreSqlToolchainDiscoveryError::IncompleteExplicitConfiguration)
        ));
    }
}
