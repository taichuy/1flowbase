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
        ExtensionArtifactInstallOutcome, ExtensionCatalogCategory, ExtensionInstallationService,
        ExtensionRiskOverride, InstallExtensionArtifactCommand, EXTENSION_RISK_CHECKSUM_MISMATCH,
        EXTENSION_RISK_SIGNATURE_INVALID, EXTENSION_RISK_SIGNATURE_MISSING,
        EXTENSION_RISK_SIGNING_KEY_UNKNOWN,
    },
    ports::{ExtensionInstallationRepository, UpsertExtensionInstallationInput},
};

#[derive(Clone, Default)]
struct MemoryExtensionInstallationRepository {
    records: Arc<
        Mutex<HashMap<domain::ExtensionInstallationIdentity, domain::ExtensionInstallationRecord>>,
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
        let created_at = records
            .get(&input.identity)
            .map(|record| record.created_at)
            .unwrap_or(now);
        let record = domain::ExtensionInstallationRecord {
            id: input.installation_id,
            identity: input.identity.clone(),
            source: input.source.clone(),
            trust: input.trust.clone(),
            local_path: input.local_path.clone(),
            checksum: input.checksum.clone(),
            signature_status: input.signature_status,
            signature_algorithm: input.signature_algorithm.clone(),
            signing_key_id: input.signing_key_id.clone(),
            warnings: input.warnings.clone(),
            receipt: input.receipt.clone(),
            status: input.status,
            installed_by: input.installed_by,
            created_at,
            updated_at: now,
        };
        records.insert(input.identity.clone(), record.clone());
        Ok(record)
    }

    async fn find_extension_installation(
        &self,
        identity: &domain::ExtensionInstallationIdentity,
    ) -> anyhow::Result<Option<domain::ExtensionInstallationRecord>> {
        Ok(self.records.lock().unwrap().get(identity).cloned())
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
            .filter(|record| record.identity.node_id == node_id)
            .cloned()
            .collect())
    }

    async fn set_extension_installation_status(
        &self,
        installation_id: Uuid,
        status: domain::ExtensionInstallationStatus,
    ) -> anyhow::Result<()> {
        if let Some(record) = self
            .records
            .lock()
            .unwrap()
            .values_mut()
            .find(|record| record.id == installation_id)
        {
            record.status = status;
        }
        Ok(())
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
        assert!(PathBuf::from(&installation.local_path).is_file());
        assert!(installation.local_path.contains(category.as_str()));
        assert!(installation.local_path.contains("@taichuy"));
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
    assert_eq!(
        tokio::fs::read(&second.local_path).await.unwrap(),
        b"local-debug"
    );

    tokio::fs::remove_file(&second.local_path).await.unwrap();
    assert!(service
        .list_installed_for_node("node-a")
        .await
        .unwrap()
        .is_empty());
    assert_eq!(service.reconcile_node_inventory("node-a").await.unwrap(), 1);
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
