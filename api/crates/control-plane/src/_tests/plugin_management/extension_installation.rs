use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex},
};

use async_trait::async_trait;
use sha2::{Digest, Sha256};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::{
    plugin_management::{
        group_installed_extension_families, ExtensionArtifactInstallOutcome,
        ExtensionCatalogCategory, ExtensionInstallationService, ExtensionRiskOverride,
        InstallExtensionArtifactCommand, EXTENSION_RISK_CHECKSUM_MISMATCH,
        EXTENSION_RISK_SIGNATURE_INVALID, EXTENSION_RISK_SIGNATURE_MISSING,
        EXTENSION_RISK_SIGNING_KEY_UNKNOWN,
    },
    ports::{ExtensionInstallationRepository, UpsertExtensionInstallationInput},
};

#[derive(Clone, Default)]
struct MemoryExtensionInstallationRepository {
    records: Arc<
        Mutex<
            HashMap<
                (String, domain::ExtensionInstallationIdentity),
                domain::ExtensionInstallationRecord,
            >,
        >,
    >,
}

#[async_trait]
impl ExtensionInstallationRepository for MemoryExtensionInstallationRepository {
    async fn upsert_extension_installation(
        &self,
        input: &UpsertExtensionInstallationInput,
    ) -> anyhow::Result<domain::ExtensionInstallationRecord> {
        let now = OffsetDateTime::now_utc();
        let mut records = self.records.lock().unwrap();
        if input.is_current {
            for record in records.values_mut() {
                if record.identity.category == input.identity.category
                    && record.identity.organization == input.identity.organization
                    && record.identity.artifact_id == input.identity.artifact_id
                    && record.node_id == input.node_id
                {
                    record.is_current = false;
                }
            }
        }
        let created_at = records
            .get(&(input.node_id.clone(), input.identity.clone()))
            .map(|record| record.created_at)
            .unwrap_or(now);
        let record = domain::ExtensionInstallationRecord {
            id: input.installation_id,
            identity: input.identity.clone(),
            source_kind: input.source_kind.clone(),
            trust_level: input.trust_level.clone(),
            expected_checksum: input.expected_checksum.clone(),
            signature_status: input.signature_status,
            signature_algorithm: input.signature_algorithm.clone(),
            signing_key_id: input.signing_key_id.clone(),
            warnings: input.warnings.clone(),
            receipt: input.receipt.clone(),
            application_action: input.application_action,
            is_system_reserved: false,
            node_id: input.node_id.clone(),
            local_path: Some(input.local_path.clone()),
            local_checksum: Some(input.local_checksum.clone()),
            status: input.status,
            is_current: input.is_current,
            created_by: input.created_by,
            created_at,
            updated_at: now,
        };
        records.insert(
            (input.node_id.clone(), input.identity.clone()),
            record.clone(),
        );
        Ok(record)
    }

    async fn find_extension_installation(
        &self,
        node_id: &str,
        identity: &domain::ExtensionInstallationIdentity,
    ) -> anyhow::Result<Option<domain::ExtensionInstallationRecord>> {
        Ok(self
            .records
            .lock()
            .unwrap()
            .get(&(node_id.to_string(), identity.clone()))
            .cloned())
    }

    async fn find_extension_installation_by_id(
        &self,
        node_id: &str,
        installation_id: Uuid,
    ) -> anyhow::Result<Option<domain::ExtensionInstallationRecord>> {
        Ok(self
            .records
            .lock()
            .unwrap()
            .values()
            .find(|record| record.node_id == node_id && record.id == installation_id)
            .cloned())
    }

    async fn list_extension_installations_for_node(
        &self,
        node_id: &str,
    ) -> anyhow::Result<Vec<domain::ExtensionInstallationRecord>> {
        Ok(self
            .records
            .lock()
            .unwrap()
            .values()
            .filter(|record| record.node_id == node_id)
            .cloned()
            .collect())
    }

    async fn set_extension_installation_status(
        &self,
        node_id: &str,
        installation_id: Uuid,
        status: domain::ExtensionInstallationStatus,
    ) -> anyhow::Result<()> {
        if let Some(record) = self
            .records
            .lock()
            .unwrap()
            .values_mut()
            .find(|record| record.node_id == node_id && record.id == installation_id)
        {
            record.status = status;
        }
        Ok(())
    }

    async fn select_current_extension_installation(
        &self,
        node_id: &str,
        installation_id: Uuid,
    ) -> anyhow::Result<Option<domain::ExtensionInstallationRecord>> {
        let mut records = self.records.lock().unwrap();
        let Some(target) = records
            .values()
            .find(|record| record.node_id == node_id && record.id == installation_id)
            .cloned()
        else {
            return Ok(None);
        };
        for record in records.values_mut() {
            if record.identity.category == target.identity.category
                && record.identity.organization == target.identity.organization
                && record.identity.artifact_id == target.identity.artifact_id
                && record.node_id == node_id
            {
                record.is_current = record.id == installation_id;
            }
        }
        Ok(records
            .values()
            .find(|record| record.id == installation_id)
            .cloned())
    }

    async fn remove_extension_installation(
        &self,
        node_id: &str,
        installation_id: Uuid,
    ) -> anyhow::Result<Option<domain::ExtensionInstallationRecord>> {
        let mut records = self.records.lock().unwrap();
        let identity = records
            .values()
            .find(|record| record.node_id == node_id && record.id == installation_id)
            .map(|record| (record.node_id.clone(), record.identity.clone()));
        let Some(key) = identity else {
            return Ok(None);
        };
        let record = records.get_mut(&key).unwrap();
        record.status = domain::ExtensionInstallationStatus::Missing;
        record.is_current = false;
        Ok(Some(record.clone()))
    }

    async fn extension_deletion_decision(
        &self,
        node_id: &str,
        installation_id: Uuid,
    ) -> anyhow::Result<Option<domain::ExtensionDeletionDecision>> {
        Ok(self
            .records
            .lock()
            .unwrap()
            .values()
            .find(|record| record.node_id == node_id && record.id == installation_id)
            .map(|record| domain::ExtensionDeletionDecision {
                deletable: !record.is_current && !record.is_system_reserved,
                reasons: if record.is_current {
                    vec!["current_version".to_string()]
                } else {
                    Vec::new()
                },
            }))
    }
}

fn test_root(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!("1flowbase-extension-{label}-{}", Uuid::now_v7()))
}

fn checksum(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn command(
    category: ExtensionCatalogCategory,
    artifact_id: &str,
    bytes: &[u8],
) -> InstallExtensionArtifactCommand {
    InstallExtensionArtifactCommand {
        actor_user_id: Uuid::now_v7(),
        category,
        organization: "taichuy".into(),
        artifact_id: artifact_id.into(),
        version: "1.0.0".into(),
        node_id: "node-a".into(),
        artifact_bytes: bytes.to_vec(),
        source: "official".into(),
        trust: "official".into(),
        expected_checksum: Some(checksum(bytes)),
        signature_status: domain::ExtensionSignatureStatus::Verified,
        signature_algorithm: Some("ed25519".into()),
        signing_key_id: Some("official-2026".into()),
        declared_warnings: Vec::new(),
        risk_override: None,
        confirmation_receipt: None,
        application_action: domain::ExtensionApplicationAction::None,
    }
}

#[tokio::test]
async fn root_1545_ac3_all_six_categories_install_into_canonical_local_truth() {
    let root = test_root("six-categories");
    let repository = MemoryExtensionInstallationRepository::default();
    let service = ExtensionInstallationService::new(repository, &root);

    for (index, category) in [
        ExtensionCatalogCategory::AgentFlow,
        ExtensionCatalogCategory::CapabilityPlugins,
        ExtensionCatalogCategory::HostExtensions,
        ExtensionCatalogCategory::I18n,
        ExtensionCatalogCategory::Mcp,
        ExtensionCatalogCategory::RuntimeExtensions,
    ]
    .into_iter()
    .enumerate()
    {
        let artifact_id = format!("fixture-{index}");
        let outcome = service
            .install_from_bytes(command(category, &artifact_id, artifact_id.as_bytes()))
            .await
            .unwrap();
        let ExtensionArtifactInstallOutcome::Installed { installation, .. } = outcome else {
            panic!("verified fixture should install without a warning challenge");
        };
        let local_path = installation.local_path.as_deref().unwrap();
        assert!(PathBuf::from(local_path).is_file());
        assert!(local_path.contains(category.as_str()));
        assert!(local_path.contains("@taichuy"));
    }

    assert_eq!(
        service
            .list_installed_for_node("node-a")
            .await
            .unwrap()
            .len(),
        6
    );
    let _ = tokio::fs::remove_dir_all(root).await;
}

#[tokio::test]
async fn root_1545_ac4_local_artifact_wins_and_duplicate_install_is_idempotent() {
    let root = test_root("local-first");
    let repository = MemoryExtensionInstallationRepository::default();
    let service = ExtensionInstallationService::new(repository, &root);
    let mut initial = command(ExtensionCatalogCategory::Mcp, "local-debug", b"local-debug");
    let first = service.install_from_bytes(initial.clone()).await.unwrap();
    let ExtensionArtifactInstallOutcome::Installed {
        installation: first,
        ..
    } = first
    else {
        panic!("initial fixture should install");
    };

    initial.artifact_bytes = b"remote-replacement".to_vec();
    let second = service.install_from_bytes(initial).await.unwrap();
    let ExtensionArtifactInstallOutcome::Installed {
        installation: second,
        local_artifact_was_present,
    } = second
    else {
        panic!("existing local fixture should remain installed");
    };
    assert!(local_artifact_was_present);
    assert_eq!(first.id, second.id);
    let second_path = second.local_path.as_deref().unwrap();
    assert_eq!(tokio::fs::read(second_path).await.unwrap(), b"local-debug");

    tokio::fs::remove_file(second_path).await.unwrap();
    let inventory = service.list_installed_for_node("node-a").await.unwrap();
    assert_eq!(inventory.len(), 1, "ordinary inventory is DB-only");
    assert_eq!(
        inventory[0].status,
        domain::ExtensionInstallationStatus::Installed
    );
    assert_eq!(service.reconcile_node_inventory("node-a").await.unwrap(), 1);
    assert!(service
        .list_installed_for_node("node-a")
        .await
        .unwrap()
        .is_empty());
    tokio::fs::write(second_path, b"corrupted").await.unwrap();
    assert_eq!(service.reconcile_node_inventory("node-a").await.unwrap(), 1);
    assert!(service
        .list_installed_for_node("node-a")
        .await
        .unwrap()
        .is_empty());
    tokio::fs::write(second_path, b"local-debug").await.unwrap();
    assert_eq!(service.reconcile_node_inventory("node-a").await.unwrap(), 0);
    assert_eq!(
        service.list_installed_for_node("node-a").await.unwrap()[0].status,
        domain::ExtensionInstallationStatus::Installed
    );
    let _ = tokio::fs::remove_dir_all(root).await;
}

#[tokio::test]
async fn root_1545_ac5_integrity_warnings_require_an_exact_override_and_never_block_after_it() {
    for (label, expected_code, signature_status, expected_checksum) in [
        (
            "missing-signature",
            EXTENSION_RISK_SIGNATURE_MISSING,
            domain::ExtensionSignatureStatus::Missing,
            None,
        ),
        (
            "checksum-mismatch",
            EXTENSION_RISK_CHECKSUM_MISMATCH,
            domain::ExtensionSignatureStatus::Verified,
            Some("sha256:not-the-artifact".to_string()),
        ),
        (
            "invalid-signature",
            EXTENSION_RISK_SIGNATURE_INVALID,
            domain::ExtensionSignatureStatus::Invalid,
            None,
        ),
        (
            "unknown-key",
            EXTENSION_RISK_SIGNING_KEY_UNKNOWN,
            domain::ExtensionSignatureStatus::UnknownKey,
            None,
        ),
    ] {
        let root = test_root(label);
        let service = ExtensionInstallationService::new(
            MemoryExtensionInstallationRepository::default(),
            &root,
        );
        let mut install = command(ExtensionCatalogCategory::I18n, label, b"fixture");
        install.signature_status = signature_status;
        install.expected_checksum = expected_checksum.or_else(|| Some(checksum(b"fixture")));
        let challenge = service.install_from_bytes(install.clone()).await.unwrap();
        let ExtensionArtifactInstallOutcome::RiskConfirmationRequired { risk_challenge } =
            challenge
        else {
            panic!("unsafe fixture must return a structured challenge");
        };
        assert!(risk_challenge.compatibility.is_none());
        assert_eq!(risk_challenge.warnings[0].code, expected_code);

        install.risk_override = Some(ExtensionRiskOverride {
            reason: "Imprecise acknowledgement".into(),
            acknowledged_warnings: vec!["different_warning".into()],
        });
        assert!(service.install_from_bytes(install.clone()).await.is_err());

        install.risk_override = Some(ExtensionRiskOverride {
            reason: "Operator approved local development artifact".into(),
            acknowledged_warnings: vec![expected_code.into()],
        });
        assert!(matches!(
            service.install_from_bytes(install).await.unwrap(),
            ExtensionArtifactInstallOutcome::Installed { .. }
        ));
        let _ = tokio::fs::remove_dir_all(root).await;
    }
}

#[tokio::test]
async fn root_1545_ac4_rejects_path_traversal_before_writing() {
    let root = test_root("path-safety");
    let service =
        ExtensionInstallationService::new(MemoryExtensionInstallationRepository::default(), &root);
    let install = command(ExtensionCatalogCategory::AgentFlow, "../escape", b"fixture");
    assert!(service.install_from_bytes(install).await.is_err());
    assert!(!root.exists());
}

#[test]
fn root_1545_d4_ac_13_groups_installed_versions_by_stable_family_identity() {
    let now = OffsetDateTime::now_utc();
    let mut explicit_current = installed_record("anthropic", "0.1.18", now);
    explicit_current.is_current = true;
    let families = group_installed_extension_families([
        explicit_current,
        installed_record("deepseek", "0.1.15", now),
        installed_record("anthropic", "0.1.23", now - time::Duration::DAY),
    ]);

    assert_eq!(families.len(), 2);
    assert_eq!(
        families[0].catalog_id(),
        "runtime-extensions:taichuy/anthropic"
    );
    assert_eq!(families[0].current.identity.version, "0.1.18");
    assert_eq!(
        families[0]
            .installed_versions
            .iter()
            .map(|record| record.identity.version.as_str())
            .collect::<Vec<_>>(),
        vec!["0.1.18", "0.1.23"]
    );
    assert_eq!(
        families[1].catalog_id(),
        "runtime-extensions:taichuy/deepseek"
    );
}

#[test]
fn ac_001_installed_families_exclude_missing_history_and_all_missing_families() {
    let now = OffsetDateTime::now_utc();
    let installed = installed_record("anthropic", "1.0.0", now);
    let mut missing_newer = installed_record("anthropic", "2.0.0", now);
    missing_newer.status = domain::ExtensionInstallationStatus::Missing;
    missing_newer.is_current = true;
    let mut all_missing = installed_record("deepseek", "1.0.0", now);
    all_missing.status = domain::ExtensionInstallationStatus::Missing;

    let families = group_installed_extension_families([installed, missing_newer, all_missing]);

    assert_eq!(families.len(), 1);
    assert_eq!(families[0].current.identity.artifact_id, "anthropic");
    assert_eq!(families[0].current.identity.version, "1.0.0");
    assert_eq!(families[0].installed_versions.len(), 1);
}

fn installed_record(
    artifact_id: &str,
    version: &str,
    updated_at: OffsetDateTime,
) -> domain::ExtensionInstallationRecord {
    domain::ExtensionInstallationRecord {
        id: Uuid::now_v7(),
        identity: domain::ExtensionInstallationIdentity {
            category: domain::ExtensionCategory::RuntimeExtensions,
            organization: "taichuy".to_string(),
            artifact_id: artifact_id.to_string(),
            version: version.to_string(),
        },
        source_kind: "official_registry".to_string(),
        trust_level: "official".to_string(),
        expected_checksum: Some("sha256:fixture".to_string()),
        signature_status: domain::ExtensionSignatureStatus::Verified,
        signature_algorithm: Some("ed25519".to_string()),
        signing_key_id: Some("official-key".to_string()),
        warnings: Vec::new(),
        receipt: serde_json::json!({}),
        application_action: domain::ExtensionApplicationAction::ConfigureModelProvider,
        is_system_reserved: false,
        node_id: "node-a".to_string(),
        local_path: Some(format!("/tmp/{artifact_id}/{version}")),
        local_checksum: Some("sha256:fixture".to_string()),
        status: domain::ExtensionInstallationStatus::Installed,
        is_current: false,
        created_by: Uuid::now_v7(),
        created_at: updated_at,
        updated_at,
    }
}
