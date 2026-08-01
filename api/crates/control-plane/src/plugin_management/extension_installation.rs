use std::{io::ErrorKind, path::PathBuf};

use anyhow::{Context, Result};
use serde_json::json;
use sha2::{Digest, Sha256};
use tokio::fs;
use uuid::Uuid;

use crate::{errors::ControlPlaneError, ports::ExtensionInstallationRepository};

use super::ExtensionCatalogCategory;

pub const EXTENSION_RISK_CHECKSUM_MISMATCH: &str = "checksum_mismatch";
pub const EXTENSION_RISK_CHECKSUM_MISSING: &str = "checksum_missing";
pub const EXTENSION_RISK_SIGNATURE_MISSING: &str = "signature_missing";
pub const EXTENSION_RISK_SIGNATURE_INVALID: &str = "signature_invalid";
pub const EXTENSION_RISK_SIGNING_KEY_UNKNOWN: &str = "signing_key_unknown";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtensionRiskOverride {
    pub reason: String,
    pub acknowledged_warnings: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct InstallExtensionArtifactCommand {
    pub actor_user_id: Uuid,
    pub category: ExtensionCatalogCategory,
    pub organization: String,
    pub artifact_id: String,
    pub version: String,
    pub node_id: String,
    pub artifact_bytes: Vec<u8>,
    pub source: String,
    pub trust: String,
    pub expected_checksum: Option<String>,
    pub signature_status: domain::ExtensionSignatureStatus,
    pub signature_algorithm: Option<String>,
    pub signing_key_id: Option<String>,
    pub declared_warnings: Vec<domain::ExtensionIntegrityWarning>,
    pub risk_override: Option<ExtensionRiskOverride>,
    pub confirmation_receipt: Option<serde_json::Value>,
}

#[derive(Debug, Clone)]
pub enum ExtensionArtifactInstallOutcome {
    RiskConfirmationRequired {
        risk_challenge: domain::ExtensionRiskChallenge,
    },
    Installed {
        installation: domain::ExtensionInstallationRecord,
        local_artifact_was_present: bool,
    },
}

pub struct ExtensionInstallationService<R> {
    repository: R,
    install_root: PathBuf,
}

impl<R> ExtensionInstallationService<R> {
    pub fn new(repository: R, install_root: impl Into<PathBuf>) -> Self {
        Self {
            repository,
            install_root: install_root.into(),
        }
    }
}

impl<R> ExtensionInstallationService<R>
where
    R: ExtensionInstallationRepository,
{
    pub async fn install_from_bytes(
        &self,
        command: InstallExtensionArtifactCommand,
    ) -> Result<ExtensionArtifactInstallOutcome> {
        let identity = installation_identity(&command)?;
        let local_path = self
            .install_root
            .join("installed")
            .join(identity.category.as_str())
            .join(format!("@{}", identity.organization))
            .join(&identity.artifact_id)
            .join(&identity.version)
            .join("artifact.bin");

        // Presence wins over catalog metadata and request bytes. In particular, integrity
        // diagnostics must never turn a local development artifact into a remote repair.
        let (artifact_bytes, local_artifact_was_present) = match fs::read(&local_path).await {
            Ok(bytes) => (bytes, true),
            Err(error) if error.kind() == ErrorKind::NotFound => {
                (command.artifact_bytes.clone(), false)
            }
            Err(error) => return Err(error).context("failed to read local extension artifact"),
        };
        let initial_checksum = sha256_checksum(&artifact_bytes);
        let initial_warnings = merge_integrity_warnings(
            command.declared_warnings.clone(),
            integrity_warnings(
                command.expected_checksum.as_deref(),
                &initial_checksum,
                command.signature_status,
            ),
        );
        match validate_risk_override(&initial_warnings, command.risk_override.as_ref())? {
            RiskOverrideDecision::Challenge(risk_challenge) => {
                return Ok(ExtensionArtifactInstallOutcome::RiskConfirmationRequired {
                    risk_challenge,
                });
            }
            RiskOverrideDecision::Accepted(_) => {}
        }

        if !local_artifact_was_present {
            create_artifact_without_replacing(&local_path, &command.artifact_bytes).await?;
        }

        // Re-read after atomic activation. If another local writer won the race, its bytes are
        // authoritative and the request must be challenged against those bytes instead.
        let installed_bytes = fs::read(&local_path)
            .await
            .context("failed to read installed extension artifact")?;
        let actual_checksum = sha256_checksum(&installed_bytes);
        let warnings = merge_integrity_warnings(
            command.declared_warnings.clone(),
            integrity_warnings(
                command.expected_checksum.as_deref(),
                &actual_checksum,
                command.signature_status,
            ),
        );
        let risk_override_receipt =
            match validate_risk_override(&warnings, command.risk_override.as_ref())? {
                RiskOverrideDecision::Challenge(risk_challenge) => {
                    return Ok(ExtensionArtifactInstallOutcome::RiskConfirmationRequired {
                        risk_challenge,
                    });
                }
                RiskOverrideDecision::Accepted(receipt) => receipt,
            };
        let override_receipt = match (risk_override_receipt, command.confirmation_receipt) {
            (None, None) => None,
            (risk, confirmation) => Some(json!({
                "risk": risk,
                "confirmation": confirmation,
            })),
        };

        let receipt = domain::ExtensionInstallationReceipt {
            source: command.source.clone(),
            trust: command.trust.clone(),
            expected_checksum: command.expected_checksum,
            actual_checksum: actual_checksum.clone(),
            signature_status: command.signature_status,
            signature_algorithm: command.signature_algorithm.clone(),
            signing_key_id: command.signing_key_id.clone(),
            warnings: warnings.clone(),
            override_receipt,
        };
        let existing = self
            .repository
            .find_extension_installation(&identity)
            .await?;
        let installation = self
            .repository
            .upsert_extension_installation(&crate::ports::UpsertExtensionInstallationInput {
                installation_id: existing
                    .as_ref()
                    .map(|record| record.id)
                    .unwrap_or_else(Uuid::now_v7),
                identity,
                source: command.source,
                trust: command.trust,
                local_path: local_path.to_string_lossy().into_owned(),
                checksum: actual_checksum,
                signature_status: command.signature_status,
                signature_algorithm: command.signature_algorithm,
                signing_key_id: command.signing_key_id,
                warnings,
                receipt: serde_json::to_value(receipt)?,
                status: domain::ExtensionInstallationStatus::Installed,
                installed_by: command.actor_user_id,
            })
            .await?;

        Ok(ExtensionArtifactInstallOutcome::Installed {
            installation,
            local_artifact_was_present,
        })
    }

    pub async fn list_installed_for_node(
        &self,
        node_id: &str,
    ) -> Result<Vec<domain::ExtensionInstallationRecord>> {
        let records = self
            .repository
            .list_extension_installations_for_node(node_id)
            .await?;
        let mut installed = Vec::with_capacity(records.len());
        for mut record in records {
            match fs::metadata(&record.local_path).await {
                Ok(metadata) if metadata.is_file() => {
                    record.status = domain::ExtensionInstallationStatus::Installed;
                    installed.push(record);
                }
                Ok(_) => {}
                Err(error) if error.kind() == ErrorKind::NotFound => {}
                Err(error) => return Err(error).context("failed to inspect extension artifact"),
            }
        }
        Ok(installed)
    }

    pub async fn find_local_installation(
        &self,
        identity: &domain::ExtensionInstallationIdentity,
    ) -> Result<Option<domain::ExtensionInstallationRecord>> {
        let Some(mut record) = self
            .repository
            .find_extension_installation(identity)
            .await?
        else {
            return Ok(None);
        };
        match fs::metadata(&record.local_path).await {
            Ok(metadata) if metadata.is_file() => {
                record.status = domain::ExtensionInstallationStatus::Installed;
                Ok(Some(record))
            }
            Ok(_) => Ok(None),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error).context("failed to inspect local extension artifact"),
        }
    }

    pub async fn reconcile_node_inventory(&self, node_id: &str) -> Result<usize> {
        let records = self
            .repository
            .list_extension_installations_for_node(node_id)
            .await?;
        let mut missing = 0usize;
        for record in records {
            match fs::metadata(&record.local_path).await {
                Ok(metadata) if metadata.is_file() => {
                    if record.status != domain::ExtensionInstallationStatus::Installed {
                        self.repository
                            .set_extension_installation_status(
                                record.id,
                                domain::ExtensionInstallationStatus::Installed,
                            )
                            .await?;
                    }
                }
                Ok(_) => {
                    self.repository
                        .set_extension_installation_status(
                            record.id,
                            domain::ExtensionInstallationStatus::Missing,
                        )
                        .await?;
                    missing = missing.saturating_add(1);
                }
                Err(error) if error.kind() == ErrorKind::NotFound => {
                    self.repository
                        .set_extension_installation_status(
                            record.id,
                            domain::ExtensionInstallationStatus::Missing,
                        )
                        .await?;
                    missing = missing.saturating_add(1);
                }
                Err(error) => return Err(error).context("failed to inspect extension artifact"),
            }
        }
        Ok(missing)
    }
}

fn installation_identity(
    command: &InstallExtensionArtifactCommand,
) -> Result<domain::ExtensionInstallationIdentity> {
    validate_segment(&command.organization, "extension_organization")?;
    validate_segment(&command.artifact_id, "extension_artifact_id")?;
    validate_segment(&command.version, "extension_version")?;
    if command.source.trim().is_empty() {
        return Err(ControlPlaneError::InvalidInput("extension_source").into());
    }
    if command.trust.trim().is_empty() {
        return Err(ControlPlaneError::InvalidInput("extension_trust").into());
    }
    if command.node_id.trim().is_empty() {
        return Err(ControlPlaneError::InvalidInput("extension_node_id").into());
    }
    let category = domain::ExtensionCategory::parse(command.category.as_str())
        .ok_or(ControlPlaneError::InvalidInput("extension_category"))?;
    Ok(domain::ExtensionInstallationIdentity {
        category,
        organization: command.organization.clone(),
        artifact_id: command.artifact_id.clone(),
        version: command.version.clone(),
        node_id: command.node_id.clone(),
    })
}

fn validate_segment(value: &str, field: &'static str) -> Result<()> {
    let value = value.trim();
    if value.is_empty()
        || matches!(value, "." | "..")
        || value.contains('/')
        || value.contains('\\')
        || value.contains('\0')
    {
        return Err(ControlPlaneError::InvalidInput(field).into());
    }
    Ok(())
}

fn sha256_checksum(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn integrity_warnings(
    expected_checksum: Option<&str>,
    actual_checksum: &str,
    signature_status: domain::ExtensionSignatureStatus,
) -> Vec<domain::ExtensionIntegrityWarning> {
    let mut warnings = Vec::new();
    if expected_checksum.is_none() {
        warnings.push(warning(
            EXTENSION_RISK_CHECKSUM_MISSING,
            "The artifact does not include an expected checksum.",
        ));
    } else if expected_checksum.is_some_and(|expected| expected != actual_checksum) {
        warnings.push(warning(
            EXTENSION_RISK_CHECKSUM_MISMATCH,
            "The artifact checksum does not match the catalog checksum.",
        ));
    }
    match signature_status {
        domain::ExtensionSignatureStatus::Verified => {}
        domain::ExtensionSignatureStatus::Missing => warnings.push(warning(
            EXTENSION_RISK_SIGNATURE_MISSING,
            "The artifact does not include a verifiable signature.",
        )),
        domain::ExtensionSignatureStatus::UnknownKey => warnings.push(warning(
            EXTENSION_RISK_SIGNING_KEY_UNKNOWN,
            "The artifact was signed by a key that is not configured as trusted.",
        )),
        domain::ExtensionSignatureStatus::Invalid => warnings.push(warning(
            EXTENSION_RISK_SIGNATURE_INVALID,
            "The artifact signature is invalid.",
        )),
    }
    warnings.sort_by(|left, right| left.code.cmp(&right.code));
    warnings
}

fn warning(code: &str, message: &str) -> domain::ExtensionIntegrityWarning {
    domain::ExtensionIntegrityWarning {
        code: code.to_string(),
        message: message.to_string(),
        overridable: true,
    }
}

fn merge_integrity_warnings(
    mut declared: Vec<domain::ExtensionIntegrityWarning>,
    discovered: Vec<domain::ExtensionIntegrityWarning>,
) -> Vec<domain::ExtensionIntegrityWarning> {
    for warning in discovered {
        if !declared.iter().any(|current| current.code == warning.code) {
            declared.push(warning);
        }
    }
    declared.sort_by(|left, right| left.code.cmp(&right.code));
    declared
}

enum RiskOverrideDecision {
    Challenge(domain::ExtensionRiskChallenge),
    Accepted(Option<serde_json::Value>),
}

fn validate_risk_override(
    warnings: &[domain::ExtensionIntegrityWarning],
    risk_override: Option<&ExtensionRiskOverride>,
) -> Result<RiskOverrideDecision> {
    if warnings.is_empty() {
        if risk_override.is_some() {
            return Err(ControlPlaneError::InvalidInput("extension_risk_override").into());
        }
        return Ok(RiskOverrideDecision::Accepted(None));
    }
    let Some(risk_override) = risk_override else {
        return Ok(RiskOverrideDecision::Challenge(
            domain::ExtensionRiskChallenge {
                warnings: warnings.to_vec(),
                compatibility: None,
            },
        ));
    };
    let expected = warnings
        .iter()
        .map(|warning| warning.code.clone())
        .collect::<Vec<_>>();
    let mut acknowledged = risk_override.acknowledged_warnings.clone();
    acknowledged.sort();
    acknowledged.dedup();
    if risk_override.reason.trim().is_empty() || acknowledged != expected {
        return Err(ControlPlaneError::InvalidInput("extension_risk_override").into());
    }
    Ok(RiskOverrideDecision::Accepted(Some(json!({
        "reason": risk_override.reason,
        "acknowledged_warnings": expected,
    }))))
}

async fn create_artifact_without_replacing(path: &std::path::Path, bytes: &[u8]) -> Result<()> {
    let parent = path
        .parent()
        .ok_or(ControlPlaneError::InvalidInput("extension_local_path"))?;
    fs::create_dir_all(parent).await?;
    let staged_path = parent.join(format!(".installing-{}", Uuid::now_v7()));
    fs::write(&staged_path, bytes).await?;
    match fs::hard_link(&staged_path, path).await {
        Ok(()) => {}
        Err(error) if error.kind() == ErrorKind::AlreadyExists => {}
        Err(error) => {
            let _ = fs::remove_file(&staged_path).await;
            return Err(error).context("failed to activate extension artifact");
        }
    }
    fs::remove_file(&staged_path).await?;
    Ok(())
}
