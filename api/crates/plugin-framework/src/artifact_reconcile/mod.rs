use std::path::Path;

use sha2::{Digest, Sha256};

use crate::error::PluginFrameworkError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArtifactReconcileOutcome {
    Missing,
    InstallIncomplete,
    Ready,
    Corrupted,
}

pub struct ArtifactReconcileInput<'a> {
    pub package_path: Option<&'a Path>,
    pub installed_path: &'a Path,
    pub expected_artifact_sha256: Option<&'a str>,
    pub expected_manifest_fingerprint: Option<&'a str>,
}

pub struct ArtifactReconcileResult {
    pub outcome: ArtifactReconcileOutcome,
    pub manifest_fingerprint: Option<String>,
    pub last_error: Option<String>,
}

pub async fn compute_manifest_fingerprint(
    manifest_path: &Path,
) -> Result<String, PluginFrameworkError> {
    let bytes = tokio::fs::read(manifest_path)
        .await
        .map_err(|error| PluginFrameworkError::io(Some(manifest_path), error.to_string()))?;
    Ok(canonical_manifest_fingerprint(&bytes))
}

pub(crate) fn canonical_manifest_fingerprint(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

pub async fn reconcile_provider_artifact(
    input: ArtifactReconcileInput<'_>,
) -> Result<ArtifactReconcileResult, PluginFrameworkError> {
    if !input.installed_path.is_dir() {
        return Ok(ArtifactReconcileResult {
            outcome: ArtifactReconcileOutcome::Missing,
            manifest_fingerprint: None,
            last_error: Some("installed_path_missing".to_string()),
        });
    }

    let manifest_path = input.installed_path.join("manifest.yaml");
    if !manifest_path.is_file() {
        return Ok(ArtifactReconcileResult {
            outcome: ArtifactReconcileOutcome::InstallIncomplete,
            manifest_fingerprint: None,
            last_error: Some("manifest_missing".to_string()),
        });
    }

    let manifest_fingerprint = compute_manifest_fingerprint(&manifest_path).await?;
    if let Some(expected) = input.expected_manifest_fingerprint {
        if normalize_sha256(expected)? != normalize_sha256(&manifest_fingerprint)? {
            return Ok(ArtifactReconcileResult {
                outcome: ArtifactReconcileOutcome::Corrupted,
                manifest_fingerprint: Some(manifest_fingerprint),
                last_error: Some("manifest_fingerprint_mismatch".to_string()),
            });
        }
    }

    if let Some(package_path) = input.package_path {
        if !package_path.is_file() {
            return Ok(ArtifactReconcileResult {
                outcome: ArtifactReconcileOutcome::InstallIncomplete,
                manifest_fingerprint: Some(manifest_fingerprint),
                last_error: Some("package_path_missing".to_string()),
            });
        }

        if let Some(expected) = input.expected_artifact_sha256 {
            let actual = compute_file_sha256(package_path).await?;
            if normalize_sha256(expected)? != actual {
                return Ok(ArtifactReconcileResult {
                    outcome: ArtifactReconcileOutcome::Corrupted,
                    manifest_fingerprint: Some(manifest_fingerprint),
                    last_error: Some("artifact_sha256_mismatch".to_string()),
                });
            }
        }
    }

    Ok(ArtifactReconcileResult {
        outcome: ArtifactReconcileOutcome::Ready,
        manifest_fingerprint: Some(manifest_fingerprint),
        last_error: None,
    })
}

async fn compute_file_sha256(path: &Path) -> Result<String, PluginFrameworkError> {
    let bytes = tokio::fs::read(path)
        .await
        .map_err(|error| PluginFrameworkError::io(Some(path), error.to_string()))?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

fn normalize_sha256(value: &str) -> Result<String, PluginFrameworkError> {
    let normalized = value
        .trim()
        .strip_prefix("sha256:")
        .unwrap_or(value.trim())
        .to_ascii_lowercase();

    if normalized.is_empty()
        || normalized.len() != Sha256::output_size() * 2
        || !normalized.chars().all(|ch| ch.is_ascii_hexdigit())
    {
        return Err(PluginFrameworkError::invalid_provider_package(
            "sha256 checksum must be a non-empty hexadecimal string",
        ));
    }

    Ok(normalized)
}
