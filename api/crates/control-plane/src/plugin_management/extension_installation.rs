use std::{cmp::Ordering, collections::BTreeMap, io::ErrorKind, path::PathBuf};

use anyhow::{Context, Result};
use semver::Version;
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
    pub application_action: domain::ExtensionApplicationAction,
}

#[derive(Debug, Clone)]
#[allow(clippy::large_enum_variant)] // Installed outcomes retain the record by value for the existing service contract.
pub enum ExtensionArtifactInstallOutcome {
    RiskConfirmationRequired {
        risk_challenge: domain::ExtensionRiskChallenge,
    },
    Installed {
        installation: domain::ExtensionInstallationRecord,
        local_artifact_was_present: bool,
    },
}

#[derive(Debug, Clone)]
pub struct InstalledExtensionFamily {
    pub current: domain::ExtensionInstallationRecord,
    pub installed_versions: Vec<domain::ExtensionInstallationRecord>,
}

impl InstalledExtensionFamily {
    pub fn catalog_id(&self) -> String {
        self.current.identity.catalog_id()
    }
}

pub fn group_installed_extension_families(
    records: impl IntoIterator<Item = domain::ExtensionInstallationRecord>,
) -> Vec<InstalledExtensionFamily> {
    let mut grouped = BTreeMap::<String, Vec<domain::ExtensionInstallationRecord>>::new();
    for record in records {
        if record.status != domain::ExtensionInstallationStatus::Installed {
            continue;
        }
        grouped
            .entry(record.identity.catalog_id())
            .or_default()
            .push(record);
    }

    grouped
        .into_values()
        .filter_map(|mut installed_versions| {
            installed_versions.sort_by(|left, right| {
                right
                    .is_current
                    .cmp(&left.is_current)
                    .then_with(|| compare_installed_extension_versions(left, right))
            });
            let current = installed_versions.first()?.clone();
            Some(InstalledExtensionFamily {
                current,
                installed_versions,
            })
        })
        .collect()
}

fn compare_installed_extension_versions(
    left: &domain::ExtensionInstallationRecord,
    right: &domain::ExtensionInstallationRecord,
) -> Ordering {
    match (
        Version::parse(&left.identity.version),
        Version::parse(&right.identity.version),
    ) {
        (Ok(left_version), Ok(right_version)) => right_version
            .cmp(&left_version)
            .then_with(|| right.updated_at.cmp(&left.updated_at))
            .then_with(|| right.id.cmp(&left.id)),
        _ => right
            .updated_at
            .cmp(&left.updated_at)
            .then_with(|| right.id.cmp(&left.id)),
    }
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
            .find_extension_installation(&command.node_id, &identity)
            .await?;
        let installation = self
            .repository
            .upsert_extension_installation(&crate::ports::UpsertExtensionInstallationInput {
                installation_id: existing
                    .as_ref()
                    .map(|record| record.id)
                    .unwrap_or_else(Uuid::now_v7),
                identity,
                node_id: command.node_id,
                source_kind: canonical_extension_source_kind(&command.source).to_string(),
                trust_level: canonical_extension_trust_level(&command.trust).to_string(),
                local_path: local_path.to_string_lossy().into_owned(),
                expected_checksum: receipt.expected_checksum.clone(),
                local_checksum: actual_checksum,
                signature_status: command.signature_status,
                signature_algorithm: command.signature_algorithm,
                signing_key_id: command.signing_key_id,
                warnings,
                receipt: serde_json::to_value(receipt)?,
                application_action: command.application_action,
                status: domain::ExtensionInstallationStatus::Installed,
                is_current: true,
                created_by: command.actor_user_id,
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
        Ok(self
            .repository
            .list_extension_installations_for_node(node_id)
            .await?
            .into_iter()
            .filter(|record| record.status == domain::ExtensionInstallationStatus::Installed)
            .collect())
    }

    pub async fn list_installed_families_for_node(
        &self,
        node_id: &str,
    ) -> Result<Vec<InstalledExtensionFamily>> {
        Ok(group_installed_extension_families(
            self.list_installed_for_node(node_id).await?,
        ))
    }

    pub async fn find_local_installation(
        &self,
        node_id: &str,
        identity: &domain::ExtensionInstallationIdentity,
    ) -> Result<Option<domain::ExtensionInstallationRecord>> {
        let Some(record) = self
            .repository
            .find_extension_installation(node_id, identity)
            .await?
        else {
            return Ok(None);
        };
        Ok((record.status == domain::ExtensionInstallationStatus::Installed).then_some(record))
    }

    pub async fn find_local_installation_by_id(
        &self,
        node_id: &str,
        installation_id: Uuid,
    ) -> Result<Option<domain::ExtensionInstallationRecord>> {
        let Some(record) = self
            .repository
            .find_extension_installation_by_id(node_id, installation_id)
            .await?
        else {
            return Ok(None);
        };
        let Some(local_path) = record.local_path.as_deref() else {
            return Ok(None);
        };
        match local_artifact_is_present(local_path).await {
            Ok(true) => Ok(Some(record)),
            Ok(false) => {
                self.repository
                    .set_extension_installation_status(
                        node_id,
                        installation_id,
                        domain::ExtensionInstallationStatus::Missing,
                    )
                    .await?;
                Ok(None)
            }
            Err(error) => Err(error).context("failed to inspect extension artifact"),
        }
    }

    pub async fn select_current_installation(
        &self,
        node_id: &str,
        installation_id: Uuid,
    ) -> Result<Option<domain::ExtensionInstallationRecord>> {
        let Some(record) = self
            .repository
            .find_extension_installation_by_id(node_id, installation_id)
            .await?
        else {
            return Ok(None);
        };
        let Some(local_path) = record.local_path.as_deref() else {
            return Ok(None);
        };
        match local_artifact_is_present(local_path).await {
            Ok(true) => {}
            Ok(false) => {
                self.repository
                    .set_extension_installation_status(
                        node_id,
                        installation_id,
                        domain::ExtensionInstallationStatus::Missing,
                    )
                    .await?;
                return Ok(None);
            }
            Err(error) => return Err(error).context("failed to inspect extension artifact"),
        }
        self.repository
            .select_current_extension_installation(node_id, installation_id)
            .await
    }

    pub async fn delete_local_installation(
        &self,
        node_id: &str,
        installation_id: Uuid,
    ) -> Result<Option<domain::ExtensionInstallationRecord>> {
        let Some(record) = self
            .repository
            .find_extension_installation_by_id(node_id, installation_id)
            .await?
        else {
            return Ok(None);
        };
        let decision = self
            .repository
            .extension_deletion_decision(node_id, installation_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("extension_installation"))?;
        if !decision.deletable {
            return Err(
                ControlPlaneError::Conflict(deletion_conflict_code(&decision.reasons)).into(),
            );
        }
        let Some(local_path) = record.local_path.as_deref() else {
            return Ok(None);
        };
        let local_path = PathBuf::from(local_path);
        let tombstone = local_path.with_file_name(format!(
            ".{}.deleting-{}",
            local_path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("extension-artifact"),
            Uuid::now_v7()
        ));
        let moved = match fs::rename(&local_path, &tombstone).await {
            Ok(()) => true,
            Err(error) if error.kind() == ErrorKind::NotFound => false,
            Err(error) => return Err(error).context("failed to stage extension artifact deletion"),
        };
        let removed = self
            .repository
            .remove_extension_installation(node_id, installation_id)
            .await;
        let removed = match removed {
            Ok(removed) => removed,
            Err(error) => {
                if moved {
                    fs::rename(&tombstone, &local_path)
                        .await
                        .context("failed to restore extension artifact after database rejection")?;
                }
                return Err(error);
            }
        };
        if moved {
            remove_local_artifact(&tombstone)
                .await
                .context("failed to remove staged extension artifact")?;
        }
        Ok(removed)
    }

    pub async fn reconcile_node_inventory(&self, node_id: &str) -> Result<usize> {
        let records = self
            .repository
            .list_extension_installations_for_node(node_id)
            .await?;
        let mut missing = 0usize;
        for record in records {
            match local_artifact_matches_record(&record).await {
                Ok(true) => {
                    if record.status != domain::ExtensionInstallationStatus::Installed {
                        self.repository
                            .set_extension_installation_status(
                                node_id,
                                record.id,
                                domain::ExtensionInstallationStatus::Installed,
                            )
                            .await?;
                    }
                }
                Ok(false) => {
                    self.repository
                        .set_extension_installation_status(
                            node_id,
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

fn deletion_conflict_code(reasons: &[String]) -> &'static str {
    match reasons.first().map(String::as_str) {
        Some("system_reserved") => "extension_system_reserved",
        Some("current_version") => "extension_current_version",
        Some("workspace_assignment") => "extension_workspace_assignment",
        Some("active_task") => "extension_active_task",
        Some("active_worker") => "extension_active_worker",
        Some("model_provider_instance") => "extension_model_provider_instance",
        Some("model_provider_preview") => "extension_model_provider_preview",
        Some("data_source_instance") => "extension_data_source_instance",
        Some("host_infrastructure_config") => "extension_host_infrastructure_config",
        Some("application_js_selection") => "extension_application_js_selection",
        Some("application_source") => "extension_application_source",
        Some("mcp_import") => "extension_mcp_import",
        _ => "extension_installation_delete_blocked",
    }
}

fn canonical_extension_source_kind(source: &str) -> &str {
    match source {
        "official" => "official_registry",
        "mirror" => "mirror_registry",
        "upload" => "uploaded",
        canonical => canonical,
    }
}

fn canonical_extension_trust_level(trust: &str) -> &str {
    match trust {
        "official" => "verified_official",
        "trusted" => "checksum_only",
        "unknown" => "unverified",
        canonical => canonical,
    }
}

async fn local_artifact_is_present(path: &str) -> std::io::Result<bool> {
    let path = PathBuf::from(path);
    match fs::metadata(&path).await {
        Ok(metadata) if metadata.is_file() => Ok(true),
        Ok(metadata) if metadata.is_dir() => {
            Ok(fs::metadata(path.join("manifest.yaml")).await.is_ok())
        }
        Ok(_) => Ok(false),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

async fn remove_local_artifact(path: &std::path::Path) -> std::io::Result<()> {
    let path = path.to_path_buf();
    match fs::metadata(&path).await {
        Ok(metadata) if metadata.is_dir() => fs::remove_dir_all(path).await,
        Ok(_) => fs::remove_file(path).await,
        Err(error) => Err(error),
    }
}

async fn local_artifact_matches_record(
    record: &domain::ExtensionInstallationRecord,
) -> std::io::Result<bool> {
    let Some(local_path) = record.local_path.as_deref() else {
        return Ok(false);
    };
    let path = PathBuf::from(local_path);
    match fs::metadata(&path).await {
        Ok(metadata) if metadata.is_file() => {
            let bytes = fs::read(path).await?;
            Ok(record
                .expected_checksum
                .as_deref()
                .is_none_or(|expected| sha256_checksum(&bytes) == expected))
        }
        Ok(metadata) if metadata.is_dir() => {
            Ok(fs::metadata(path.join("manifest.yaml")).await.is_ok())
        }
        Ok(_) => Ok(false),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
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

pub fn installed_extension_integrity_warnings(
    installation: &domain::ExtensionInstallationRecord,
    artifact_bytes: &[u8],
) -> Vec<domain::ExtensionIntegrityWarning> {
    merge_integrity_warnings(
        installation.warnings.clone(),
        integrity_warnings(
            installation.expected_checksum.as_deref(),
            &sha256_checksum(artifact_bytes),
            installation.signature_status,
        ),
    )
}

pub fn validate_extension_integrity_override(
    warnings: &[domain::ExtensionIntegrityWarning],
    risk_override: Option<&ExtensionRiskOverride>,
) -> Result<bool> {
    Ok(matches!(
        validate_risk_override(warnings, risk_override)?,
        RiskOverrideDecision::Accepted(_)
    ))
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
