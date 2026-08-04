use std::{fs, path::PathBuf};

use plugin_framework::artifact_reconcile::{
    compute_manifest_fingerprint, reconcile_provider_artifact, ArtifactReconcileInput,
    ArtifactReconcileOutcome,
};
use sha2::{Digest, Sha256};
use uuid::Uuid;

struct TempArtifactFixture {
    root: PathBuf,
    package_path: PathBuf,
    installed_path: PathBuf,
    package_sha256: String,
}

impl TempArtifactFixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "plugin-framework-artifact-reconcile-{}",
            Uuid::now_v7()
        ));
        let package_path = root.join("packages").join("fixture_provider.1flowbasepkg");
        let installed_path = root.join("installed").join("fixture_provider");
        fs::create_dir_all(&installed_path).unwrap();
        fs::create_dir_all(package_path.parent().unwrap()).unwrap();

        fs::write(
            installed_path.join("manifest.yaml"),
            r#"manifest_version: 1
plugin_id: fixture_provider@0.1.0
version: 0.1.0
publisher_namespace: fixture
vendor: fixture
display_name: Fixture Provider
description: Fixture provider
source_kind: uploaded
trust_level: unverified
consumption_kind: runtime_extension
execution_mode: process_per_call
slot_codes:
  - model_provider
binding_targets:
  - workspace
selection_mode: assignment_then_select
minimum_host_version: 0.1.0
contract_version: 1flowbase.provider/v2
schema_version: 1flowbase.plugin.manifest/v1
permissions:
  network: outbound_only
  secrets: provider_instance_only
  storage: none
  mcp: none
  subprocess: deny
runtime:
  protocol: stdio_json
  entry: bin/fixture-provider
node_contributions: []
"#,
        )
        .unwrap();
        fs::write(&package_path, b"fixture package bytes").unwrap();

        let package_sha256 = format!("{:x}", Sha256::digest(b"fixture package bytes"));

        Self {
            root,
            package_path,
            installed_path,
            package_sha256,
        }
    }
}

impl Drop for TempArtifactFixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[tokio::test]
async fn reconcile_provider_artifact_reports_ready_when_manifest_and_checksums_match() {
    let fixture = TempArtifactFixture::new();
    let manifest_fingerprint =
        compute_manifest_fingerprint(&fixture.installed_path.join("manifest.yaml"))
            .await
            .unwrap();

    let result = reconcile_provider_artifact(ArtifactReconcileInput {
        package_path: Some(fixture.package_path.as_path()),
        installed_path: fixture.installed_path.as_path(),
        expected_artifact_sha256: Some(fixture.package_sha256.as_str()),
        expected_manifest_fingerprint: Some(manifest_fingerprint.as_str()),
    })
    .await
    .unwrap();

    assert_eq!(result.outcome, ArtifactReconcileOutcome::Ready);
    assert_eq!(
        result.manifest_fingerprint.as_deref(),
        Some(manifest_fingerprint.as_str())
    );
    assert!(result.last_error.is_none());
}

#[tokio::test]
async fn reconcile_provider_artifact_reports_missing_when_installed_path_is_absent() {
    let temp = std::env::temp_dir().join(format!(
        "plugin-framework-artifact-reconcile-missing-{}",
        Uuid::now_v7()
    ));
    let missing_installed_path = temp.join("missing-installed");

    let result = reconcile_provider_artifact(ArtifactReconcileInput {
        package_path: None,
        installed_path: missing_installed_path.as_path(),
        expected_artifact_sha256: None,
        expected_manifest_fingerprint: None,
    })
    .await
    .unwrap();

    assert_eq!(result.outcome, ArtifactReconcileOutcome::Missing);
    assert!(result.manifest_fingerprint.is_none());
    assert_eq!(result.last_error.as_deref(), Some("installed_path_missing"));
}

#[tokio::test]
async fn reconcile_provider_artifact_reports_corrupted_when_manifest_fingerprint_drifts() {
    let fixture = TempArtifactFixture::new();
    let original_fingerprint =
        compute_manifest_fingerprint(&fixture.installed_path.join("manifest.yaml"))
            .await
            .unwrap();

    fs::write(
        fixture.installed_path.join("manifest.yaml"),
        r#"manifest_version: 1
plugin_id: tampered_provider@0.1.0
version: 0.1.0
publisher_namespace: fixture
vendor: fixture
display_name: Fixture Provider
description: Fixture provider
source_kind: uploaded
trust_level: unverified
consumption_kind: runtime_extension
execution_mode: process_per_call
slot_codes:
  - model_provider
binding_targets:
  - workspace
selection_mode: assignment_then_select
minimum_host_version: 0.1.0
contract_version: 1flowbase.provider/v2
schema_version: 1flowbase.plugin.manifest/v1
permissions:
  network: outbound_only
  secrets: provider_instance_only
  storage: none
  mcp: none
  subprocess: deny
runtime:
  protocol: stdio_json
  entry: bin/fixture-provider
node_contributions: []
"#,
    )
    .unwrap();

    let result = reconcile_provider_artifact(ArtifactReconcileInput {
        package_path: Some(fixture.package_path.as_path()),
        installed_path: fixture.installed_path.as_path(),
        expected_artifact_sha256: Some(fixture.package_sha256.as_str()),
        expected_manifest_fingerprint: Some(original_fingerprint.as_str()),
    })
    .await
    .unwrap();

    assert_eq!(result.outcome, ArtifactReconcileOutcome::Corrupted);
    assert!(result
        .last_error
        .as_deref()
        .is_some_and(|value| value.contains("manifest_fingerprint_mismatch")));
}
