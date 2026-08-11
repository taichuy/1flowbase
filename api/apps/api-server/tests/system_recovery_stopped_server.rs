use std::{fs, process::Command};

use serde_json::Value;
use uuid::Uuid;

#[test]
fn status_reads_the_external_journal_without_contacting_the_stopped_api_database() {
    let root = std::env::temp_dir().join(format!("system-recovery-cli-{}", Uuid::now_v7()));
    let repository = root.join("repository");
    let output = Command::new(env!("CARGO_BIN_EXE_system_recovery"))
        .args(["status", "--recovery-job-id", &Uuid::now_v7().to_string()])
        .env("API_ENV", "development")
        .env(
            "API_DATABASE_URL",
            "postgres://stopped-api.invalid:1/server_is_stopped",
        )
        .env("API_DATABASE_POOL_MAX_CONNECTIONS", "1")
        .env("BOOTSTRAP_WORKSPACE_NAME", "offline-recovery-fixture")
        .env("BOOTSTRAP_ROOT_ACCOUNT", "root")
        .env("BOOTSTRAP_ROOT_EMAIL", "root@example.com")
        .env("BOOTSTRAP_ROOT_PASSWORD", "unused-offline")
        .env("BOOTSTRAP_ROOT_NAME", "Root")
        .env("BOOTSTRAP_ROOT_NICKNAME", "Root")
        .env("API_SYSTEM_BACKUP_REPOSITORY_ROOT", &repository)
        .env("API_BUSINESS_FILE_LOCAL_ROOT", root.join("objects"))
        .env("API_PROVIDER_INSTALL_ROOT", root.join("providers"))
        .env("API_MCP_TEMPLATE_LIBRARY_ROOT", root.join("mcp"))
        .env(
            "API_HOST_EXTENSION_DROPIN_ROOT",
            root.join("providers/host-extension/dropins"),
        )
        .output()
        .expect("the recovery binary must be executable without an API server");

    assert!(
        output.status.success(),
        "offline status failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let report: Value = serde_json::from_slice(&output.stdout).expect("status must emit JSON");
    assert_eq!(report["journal_location"], "external_backup_repository");
    assert_eq!(report["journal_event_count"], 0);
    assert_eq!(report["status"], Value::Null);

    fs::remove_dir_all(root).expect("the stopped-server fixture must clean its roots");
}
