use std::{fs, path::PathBuf, time::Duration};

use control_plane::ports::FrontstageExecutableUpgradeCompiler;
use serde_json::json;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::*;

fn fixture_source() -> domain::LegacyFrontstageExecutableSnapshotRow {
    domain::LegacyFrontstageExecutableSnapshotRow {
        row_id: Uuid::now_v7(),
        workspace_id: Uuid::now_v7(),
        page_id: Uuid::now_v7(),
        code_ref: "fixture".into(),
        source_code: "export default () => null;".into(),
        source_sha256: "fd1122b0fb60184867dbbc2c2c13731750291b3e66966b90a267750423f99130".into(),
        catalog_locator: domain::FrontstageExecutableCatalogLocator {
            installation_id: Uuid::now_v7(),
            provider_code: "1flowbase".into(),
            plugin_id: "1flowbase@1.0.0".into(),
            plugin_version: "1.0.0".into(),
            contribution_code: "frontstage.js-ui-block".into(),
        },
        runtime_descriptor: json!({}),
        dependency_lock: json!([]),
    }
}

fn compiler(script: &str, timeout: Duration) -> (NodeFrontstageExecutableCompiler, PathBuf) {
    let root = std::env::temp_dir().join(format!("frontstage-compiler-{}", Uuid::now_v7()));
    fs::create_dir(&root).unwrap();
    let entry = root.join("compiler.sh");
    fs::write(&entry, script).unwrap();
    let entry_sha256 = format!("{:x}", Sha256::digest(script.as_bytes()));
    (
        NodeFrontstageExecutableCompiler {
            process: CompilerProcess {
                program: "/bin/sh".into(),
                entry,
                current_dir: root.clone(),
                entry_sha256,
                timeout,
            },
        },
        root,
    )
}

#[tokio::test]
async fn malformed_output_fails_closed() {
    let (compiler, root) = compiler(
        "cat >/dev/null\nprintf 'not-json\\n'\n",
        Duration::from_secs(1),
    );
    let failure = compiler
        .compile_frontstage_executable(&target(), &fixture_source())
        .await
        .unwrap_err();
    assert_eq!(failure.error_code, "malformed_output");
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn nonzero_process_fails_closed() {
    let (compiler, root) = compiler("cat >/dev/null\nexit 9\n", Duration::from_secs(1));
    let failure = compiler
        .compile_frontstage_executable(&target(), &fixture_source())
        .await
        .unwrap_err();
    assert_eq!(failure.error_code, "process_nonzero");
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn artifact_identity_mismatch_fails_closed() {
    let response = json!({
        "ok": true,
        "generated_css": "",
        "generated_css_sha256": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        "source_sha256": fixture_source().source_sha256,
        "dependency_lock": [],
        "compiler_identity": target().compiler_identity,
        "toolchain_lock": target().toolchain_lock,
        "artifact_identity": { "name": "wrong" },
        "artifact_sha256": "2005c459882fcaeb283ff36706b327efebf8783414ecaa111f92f628c4ba0af8"
    });
    let script = format!("cat >/dev/null\nprintf '%s\\n' '{}'\n", response);
    let (compiler, root) = compiler(&script, Duration::from_secs(1));
    let failure = compiler
        .compile_frontstage_executable(&target(), &fixture_source())
        .await
        .unwrap_err();
    assert_eq!(failure.error_code, "artifact_identity_mismatch");
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn timeout_fails_closed() {
    let (compiler, root) = compiler("cat >/dev/null\nsleep 5\n", Duration::from_millis(10));
    let failure = compiler
        .compile_frontstage_executable(&target(), &fixture_source())
        .await
        .unwrap_err();
    assert_eq!(failure.error_code, "process_timeout");
    fs::remove_dir_all(root).unwrap();
}
