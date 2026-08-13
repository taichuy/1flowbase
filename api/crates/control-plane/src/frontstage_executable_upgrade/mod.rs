use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::ports::{
    FrontstageExecutableCompilerFailure, FrontstageExecutableUpgradeCompiler,
    FrontstageExecutableUpgradeRepository,
};

#[derive(Debug, Error)]
pub enum FrontstageExecutableUpgradeError {
    #[error("frontstage executable upgrade target is invalid")]
    InvalidTarget,
    #[error("frontstage executable compilation failed: {0}")]
    CompilationFailed(String),
    #[error(transparent)]
    Repository(#[from] anyhow::Error),
}

pub struct FrontstageExecutableUpgradeService<R, C> {
    repository: R,
    compiler: C,
}

impl<R, C> FrontstageExecutableUpgradeService<R, C>
where
    R: FrontstageExecutableUpgradeRepository,
    C: FrontstageExecutableUpgradeCompiler,
{
    pub fn new(repository: R, compiler: C) -> Self {
        Self {
            repository,
            compiler,
        }
    }

    pub async fn run(
        &self,
        target: domain::FrontstageExecutableUpgradeTarget,
    ) -> Result<domain::FrontstageExecutableUpgradeOutcome, FrontstageExecutableUpgradeError> {
        validate_target(&target)?;
        let start = self
            .repository
            .begin_frontstage_executable_upgrade(&target)
            .await?;
        let domain::FrontstageExecutableUpgradeStart::Run { run_id, .. } = start else {
            self.repository
                .require_frontstage_executable_cutover(&target)
                .await?;
            return Ok(domain::FrontstageExecutableUpgradeOutcome::Completed {
                run_id: None,
                upgraded: 0,
            });
        };

        let snapshot = self
            .repository
            .capture_frontstage_executable_upgrade_snapshot(&target, run_id)
            .await?;
        let mut compiled = Vec::with_capacity(snapshot.rows.len());
        for source in &snapshot.rows {
            match self
                .compiler
                .compile_frontstage_executable(&target, source)
                .await
            {
                Ok(payload) => match validate_compiled_payload(&target, source, &payload) {
                    Ok(()) => compiled.push(payload),
                    Err(error) => {
                        self.record_failure(
                            &target,
                            run_id,
                            source,
                            FrontstageExecutableCompilerFailure {
                                error_code: "compiled_payload_invalid".into(),
                            },
                        )
                        .await?;
                        return Err(error);
                    }
                },
                Err(failure) => {
                    self.record_failure(&target, run_id, source, failure)
                        .await?;
                    return Err(FrontstageExecutableUpgradeError::CompilationFailed(
                        source.code_ref.clone(),
                    ));
                }
            }
        }
        self.repository
            .commit_frontstage_executable_upgrade(&target, &snapshot, &compiled)
            .await?;
        self.repository
            .require_frontstage_executable_cutover(&target)
            .await?;
        Ok(domain::FrontstageExecutableUpgradeOutcome::Completed {
            run_id: Some(run_id),
            upgraded: compiled.len(),
        })
    }

    async fn record_failure(
        &self,
        target: &domain::FrontstageExecutableUpgradeTarget,
        run_id: uuid::Uuid,
        source: &domain::LegacyFrontstageExecutableSnapshotRow,
        failure: FrontstageExecutableCompilerFailure,
    ) -> Result<(), FrontstageExecutableUpgradeError> {
        self.repository
            .record_frontstage_executable_upgrade_failure(
                target,
                &domain::FrontstageExecutableUpgradeFailure {
                    run_id,
                    marker: target.marker.clone(),
                    error_code: failure.error_code,
                    target_identity: serde_json::json!({
                        "row_id": source.row_id,
                        "workspace_id": source.workspace_id,
                        "page_id": source.page_id,
                        "code_ref": source.code_ref,
                        "source_sha256": source.source_sha256,
                    }),
                    compiler_identity: target.compiler_identity.clone(),
                },
            )
            .await?;
        Ok(())
    }
}

fn validate_target(
    target: &domain::FrontstageExecutableUpgradeTarget,
) -> Result<(), FrontstageExecutableUpgradeError> {
    if target.marker.trim().is_empty()
        || target.marker.trim() != target.marker
        || !non_empty_string_object(&target.contract_identity)
        || !non_empty_string_object(&target.compiler_identity)
        || !non_empty_string_object(&target.toolchain_lock)
    {
        return Err(FrontstageExecutableUpgradeError::InvalidTarget);
    }
    Ok(())
}

fn validate_compiled_payload(
    target: &domain::FrontstageExecutableUpgradeTarget,
    source: &domain::LegacyFrontstageExecutableSnapshotRow,
    payload: &domain::CompiledFrontstageExecutable,
) -> Result<(), FrontstageExecutableUpgradeError> {
    let generated_digest = format!("{:x}", Sha256::digest(payload.generated_css.as_bytes()));
    if payload.row_id != source.row_id
        || payload.source_sha256 != source.source_sha256
        || payload.dependency_lock != source.dependency_lock
        || payload.generated_css_sha256 != generated_digest
        || payload.compiler_identity != target.compiler_identity
        || payload.toolchain_lock != target.toolchain_lock
        || payload.contract_identity != target.contract_identity
    {
        return Err(FrontstageExecutableUpgradeError::CompilationFailed(
            source.code_ref.clone(),
        ));
    }
    Ok(())
}

fn non_empty_string_object(value: &serde_json::Value) -> bool {
    value.as_object().is_some_and(|object| {
        !object.is_empty()
            && object
                .values()
                .all(|entry| entry.as_str().is_some_and(|entry| !entry.trim().is_empty()))
    })
}

#[cfg(test)]
#[path = "_tests/service.rs"]
mod tests;
