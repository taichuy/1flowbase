use std::sync::Mutex;

use async_trait::async_trait;
use serde_json::json;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::*;

struct FixtureRepository {
    start: domain::FrontstageExecutableUpgradeStart,
    rows: Vec<domain::LegacyFrontstageExecutableSnapshotRow>,
    commits: Mutex<usize>,
    failures: Mutex<Vec<domain::FrontstageExecutableUpgradeFailure>>,
    cutover_checks: Mutex<usize>,
}

#[async_trait]
impl FrontstageExecutableUpgradeRepository for FixtureRepository {
    async fn begin_frontstage_executable_upgrade(
        &self,
        _: &domain::FrontstageExecutableUpgradeTarget,
    ) -> anyhow::Result<domain::FrontstageExecutableUpgradeStart> {
        Ok(self.start.clone())
    }

    async fn capture_frontstage_executable_upgrade_snapshot(
        &self,
        _: &domain::FrontstageExecutableUpgradeTarget,
        run_id: Uuid,
    ) -> anyhow::Result<domain::LegacyFrontstageExecutableSnapshot> {
        Ok(domain::LegacyFrontstageExecutableSnapshot {
            run_id,
            rows: self.rows.clone(),
            snapshot_sha256: "a".repeat(64),
        })
    }

    async fn commit_frontstage_executable_upgrade(
        &self,
        _: &domain::FrontstageExecutableUpgradeTarget,
        _: &domain::LegacyFrontstageExecutableSnapshot,
        _: &[domain::CompiledFrontstageExecutable],
    ) -> anyhow::Result<()> {
        *self.commits.lock().expect("fixture commit mutex") += 1;
        Ok(())
    }

    async fn record_frontstage_executable_upgrade_failure(
        &self,
        _: &domain::FrontstageExecutableUpgradeTarget,
        failure: &domain::FrontstageExecutableUpgradeFailure,
    ) -> anyhow::Result<()> {
        self.failures
            .lock()
            .expect("fixture failure mutex")
            .push(failure.clone());
        Ok(())
    }

    async fn require_frontstage_executable_cutover(
        &self,
        _: &domain::FrontstageExecutableUpgradeTarget,
    ) -> anyhow::Result<()> {
        *self.cutover_checks.lock().expect("fixture cutover mutex") += 1;
        Ok(())
    }
}

struct FixtureCompiler {
    failing_row: Option<Uuid>,
}

#[async_trait]
impl FrontstageExecutableUpgradeCompiler for FixtureCompiler {
    async fn compile_frontstage_executable(
        &self,
        target: &domain::FrontstageExecutableUpgradeTarget,
        source: &domain::LegacyFrontstageExecutableSnapshotRow,
    ) -> Result<domain::CompiledFrontstageExecutable, FrontstageExecutableCompilerFailure> {
        if self.failing_row == Some(source.row_id) {
            return Err(FrontstageExecutableCompilerFailure {
                error_code: "tsx_transform_failed".into(),
            });
        }
        let generated_css = format!(".row-{} {{}}", source.row_id);
        Ok(domain::CompiledFrontstageExecutable {
            row_id: source.row_id,
            source_sha256: source.source_sha256.clone(),
            dependency_lock: source.dependency_lock.clone(),
            generated_css_sha256: format!("{:x}", Sha256::digest(generated_css.as_bytes())),
            generated_css,
            compiler_identity: target.compiler_identity.clone(),
            toolchain_lock: target.toolchain_lock.clone(),
            contract_identity: target.contract_identity.clone(),
        })
    }
}

#[tokio::test]
async fn second_compiler_failure_records_evidence_without_committing_rows() {
    let run_id = Uuid::now_v7();
    let rows = vec![snapshot_row("first"), snapshot_row("second")];
    let failing_row = rows[1].row_id;
    let repository = FixtureRepository {
        start: domain::FrontstageExecutableUpgradeStart::Run { run_id, attempt: 1 },
        rows,
        commits: Mutex::new(0),
        failures: Mutex::new(Vec::new()),
        cutover_checks: Mutex::new(0),
    };
    let service = FrontstageExecutableUpgradeService::new(
        repository,
        FixtureCompiler {
            failing_row: Some(failing_row),
        },
    );

    let error = service
        .run(target())
        .await
        .expect_err("second row must fail");
    assert!(matches!(
        error,
        FrontstageExecutableUpgradeError::CompilationFailed(_)
    ));
    assert_eq!(*service.repository.commits.lock().unwrap(), 0);
    let failures = service.repository.failures.lock().unwrap();
    assert_eq!(failures.len(), 1);
    assert_eq!(failures[0].error_code, "tsx_transform_failed");
    assert_eq!(failures[0].target_identity["row_id"], json!(failing_row));
}

#[tokio::test]
async fn fresh_database_commits_empty_snapshot_and_completed_target_is_noop() {
    let run_id = Uuid::now_v7();
    let repository = FixtureRepository {
        start: domain::FrontstageExecutableUpgradeStart::Run { run_id, attempt: 1 },
        rows: Vec::new(),
        commits: Mutex::new(0),
        failures: Mutex::new(Vec::new()),
        cutover_checks: Mutex::new(0),
    };
    let service =
        FrontstageExecutableUpgradeService::new(repository, FixtureCompiler { failing_row: None });
    let outcome = service.run(target()).await.unwrap();
    assert_eq!(
        outcome,
        domain::FrontstageExecutableUpgradeOutcome::Completed {
            run_id: Some(run_id),
            upgraded: 0
        }
    );
    assert_eq!(*service.repository.commits.lock().unwrap(), 1);
    assert_eq!(*service.repository.cutover_checks.lock().unwrap(), 1);

    let repository = FixtureRepository {
        start: domain::FrontstageExecutableUpgradeStart::Completed,
        rows: Vec::new(),
        commits: Mutex::new(0),
        failures: Mutex::new(Vec::new()),
        cutover_checks: Mutex::new(0),
    };
    let service =
        FrontstageExecutableUpgradeService::new(repository, FixtureCompiler { failing_row: None });
    service.run(target()).await.unwrap();
    assert_eq!(*service.repository.commits.lock().unwrap(), 0);
    assert_eq!(*service.repository.cutover_checks.lock().unwrap(), 1);
}

fn target() -> domain::FrontstageExecutableUpgradeTarget {
    domain::FrontstageExecutableUpgradeTarget {
        marker: "tailwind-4.3.3-v1".into(),
        contract_identity: json!({ "artifact": "compiler-4.3.3" }),
        compiler_identity: json!({ "name": "tailwind", "version": "4.3.3" }),
        toolchain_lock: json!({ "package": "tailwindcss", "version": "4.3.3" }),
    }
}

fn snapshot_row(code_ref: &str) -> domain::LegacyFrontstageExecutableSnapshotRow {
    let source_code = format!("export default function {code_ref}() {{ return null; }}");
    domain::LegacyFrontstageExecutableSnapshotRow {
        row_id: Uuid::now_v7(),
        workspace_id: Uuid::now_v7(),
        page_id: Uuid::now_v7(),
        code_ref: code_ref.into(),
        source_sha256: format!("{:x}", Sha256::digest(source_code.as_bytes())),
        source_code,
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
