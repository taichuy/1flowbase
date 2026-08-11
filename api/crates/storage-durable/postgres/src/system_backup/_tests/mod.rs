use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
};

use tokio::process::Command;
use uuid::Uuid;

use super::{
    parse_major_version, PostgreSqlBackupError, PostgreSqlCommandConnection,
    PostgreSqlCommandConnectionError,
};

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

#[tokio::test]
async fn postgresql_tool_connection_keeps_password_out_of_argv_and_parent_environment() {
    let parent_password = std::env::var_os("PGPASSWORD");
    let capture = capture_connection(
        "postgres://operator:s3cr%25et%3Awith%2Fchars%2Bplus@127.0.0.1:5432/app?sslmode=disable&statement-cache-capacity=100&application_name=recovery",
    )
    .await
    .unwrap();

    assert_eq!(
        capture.arguments,
        vec![
            "--dbname",
            "postgres://operator@127.0.0.1:5432/app?sslmode=disable&application_name=recovery",
        ]
    );
    assert_eq!(capture.password.as_deref(), Some("s3cr%et:with/chars+plus"));
    assert!(!capture.arguments.join(" ").contains("s3cr"));
    assert_eq!(std::env::var_os("PGPASSWORD"), parent_password);
}

#[tokio::test]
async fn postgresql_tool_connection_preserves_passwordless_url_and_query_exactly() {
    let database_url =
        "postgresql://operator@db.internal:5433/app?application_name=a%2Bb&sslmode=require";
    let capture = capture_connection(database_url).await.unwrap();

    assert_eq!(capture.arguments, vec!["--dbname", database_url]);
    assert_eq!(capture.password, None);
}

struct CapturedConnection {
    arguments: Vec<String>,
    password: Option<String>,
}

async fn capture_connection(
    database_url: &str,
) -> Result<CapturedConnection, PostgreSqlCommandConnectionError> {
    let root = TemporaryDirectory::new();
    let tool = root.path().join("capture-postgresql-tool");
    fs::write(
        &tool,
        r#"#!/bin/sh
printf '%s\n' "$@" > "$0.args"
if [ "${PGPASSWORD+x}" = x ]; then
  printf '%s' "$PGPASSWORD" > "$0.password"
fi
"#,
    )
    .expect("capture tool must be writable");
    let mut permissions = fs::metadata(&tool)
        .expect("capture tool metadata must exist")
        .permissions();
    permissions.set_mode(0o700);
    fs::set_permissions(&tool, permissions).expect("capture tool must be executable");

    let connection = PostgreSqlCommandConnection::parse(database_url)?;
    let mut command = Command::new(&tool);
    command.env_clear();
    connection.apply(&mut command);
    let status = command
        .status()
        .await
        .expect("capture tool must start successfully");
    assert!(status.success(), "capture tool must exit successfully");

    let arguments = fs::read_to_string(tool.with_extension("args"))
        .expect("capture tool must record argv")
        .lines()
        .map(str::to_owned)
        .collect();
    let password = fs::read_to_string(tool.with_extension("password")).ok();
    Ok(CapturedConnection {
        arguments,
        password,
    })
}

struct TemporaryDirectory {
    path: PathBuf,
}

impl TemporaryDirectory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "postgresql-tool-connection-{}",
            Uuid::now_v7().simple()
        ));
        fs::create_dir(&path).expect("temporary capture root must be created");
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TemporaryDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}
