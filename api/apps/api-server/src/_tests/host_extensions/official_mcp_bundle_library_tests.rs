use async_trait::async_trait;
use axum::{routing::get, Router};
use base64::{engine::general_purpose::STANDARD, Engine};
use ed25519_dalek::{pkcs8::EncodePublicKey, Signer, SigningKey};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

use crate::{
    config::ResolvedOfficialMcpBundleSourceConfig,
    official_mcp_bundles::{ApiOfficialMcpBundleRegistry, OfficialMcpBundleSourcePort},
};

#[tokio::test]
async fn mcp_library_verifies_signed_releases_and_resolves_existing_local_artifact_offline() {
    // AC-004: sync writes only the fixed local library; resolve remains local-first.
    let artifact = b"fixture-mcp-bundle-zip".to_vec();
    let signing_key = SigningKey::from_bytes(&[23; 32]);
    let checksum = format!("sha256:{:x}", Sha256::digest(&artifact));
    let signature = STANDARD.encode(signing_key.sign(&artifact).to_bytes());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let catalog = catalog(&base, &checksum, &signature);
    let served_artifact = artifact.clone();
    let server = tokio::spawn(async move {
        axum::serve(
            listener,
            Router::new()
                .route(
                    "/catalog.json",
                    get(move || {
                        let catalog = catalog.clone();
                        async move { axum::Json(catalog) }
                    }),
                )
                .route(
                    "/bundle.zip",
                    get(move || {
                        let artifact = served_artifact.clone();
                        async move { artifact }
                    }),
                ),
        )
        .await
        .unwrap();
    });
    let root = temp_root();
    let (library, installation_repository) = build_library(&base, root.clone(), &signing_key);

    let local_only = library.library_catalog().await.unwrap();
    assert!(local_only.bundles.is_empty());

    let projected = library.refresh_catalog().await.unwrap();
    assert_eq!(
        projected.bundles[0].remote_versions[0].bundle_version,
        "1.10.0"
    );

    library.sync("taichuy", "zh_hans", None).await.unwrap();
    let indexed = installation_repository.records();
    assert_eq!(indexed.len(), 1);
    assert_eq!(indexed[0].identity.category, domain::ExtensionCategory::Mcp);
    assert_eq!(indexed[0].identity.version, "1.10.0");
    assert_eq!(
        indexed[0].application_action,
        domain::ExtensionApplicationAction::ImportMcp
    );
    assert_eq!(
        indexed[0].signature_status,
        domain::ExtensionSignatureStatus::Verified
    );
    assert!(indexed[0].local_path.ends_with("/bundle.zip"));

    assert_eq!(
        library
            .resolve_artifact("taichuy", "zh_hans", None)
            .await
            .unwrap(),
        artifact
    );
    assert!(root
        .join("@taichuy/zh_hans/releases/1.10.0/bundle.zip")
        .is_file());
    library
        .sync("taichuy", "zh_hans", Some("1.2.0"))
        .await
        .unwrap();
    std::fs::remove_file(root.join("@taichuy/zh_hans/releases/1.10.0/receipt.json")).unwrap();
    let with_history = library.library_catalog().await.unwrap();
    assert_eq!(
        with_history.bundles[0].local_versions[0].bundle_version,
        "1.10.0"
    );
    library
        .switch_current("taichuy", "zh_hans", "1.10.0")
        .await
        .unwrap();
    library
        .repair("taichuy", "zh_hans", "1.10.0")
        .await
        .unwrap();
    library
        .delete_local_version("taichuy", "zh_hans", "1.2.0")
        .await
        .unwrap();

    server.abort();
    let _ = server.await;
    assert_eq!(
        library
            .resolve_artifact("taichuy", "zh_hans", None)
            .await
            .unwrap(),
        artifact,
        "an existing current release must not require remote catalog access"
    );
    std::fs::write(
        root.join("@taichuy/zh_hans/releases/1.10.0/bundle.zip"),
        b"tampered",
    )
    .unwrap();
    assert!(library
        .resolve_artifact("taichuy", "zh_hans", None)
        .await
        .is_err());
    let (reconciled_library, reconciled_repository) =
        build_library(&base, root.clone(), &signing_key);
    reconciled_library
        .reconcile_local_installations()
        .await
        .unwrap();
    assert_eq!(reconciled_repository.records().len(), 1);
    std::fs::remove_file(root.join("@taichuy/zh_hans/releases/1.10.0/bundle.zip")).unwrap();
    reconciled_library
        .reconcile_local_installations()
        .await
        .unwrap();
    assert_eq!(
        reconciled_repository.records()[0].status,
        domain::ExtensionInstallationStatus::Missing
    );
    let _ = std::fs::remove_dir_all(root);
}

fn build_library(
    base: &str,
    root: std::path::PathBuf,
    signing_key: &SigningKey,
) -> (
    ApiOfficialMcpBundleRegistry,
    Arc<TestExtensionInstallationRepository>,
) {
    let actor_user_id = Uuid::now_v7();
    let installation_repository = Arc::new(TestExtensionInstallationRepository::default());
    let library = ApiOfficialMcpBundleRegistry::new(
        ResolvedOfficialMcpBundleSourceConfig {
            source_kind: "official_registry".into(),
            source_label: "Official".into(),
            catalog_url: format!("{base}/catalog.json"),
            github_proxy_url: None,
        },
        root,
        installation_repository.clone(),
        "test-node".into(),
        actor_user_id,
        vec![plugin_framework::TrustedPublicKey {
            key_id: "fixture-key".into(),
            algorithm: "ed25519".into(),
            public_key_pem: signing_key
                .verifying_key()
                .to_public_key_pem(Default::default())
                .unwrap(),
        }],
    );
    (library, installation_repository)
}

#[derive(Default)]
struct TestExtensionInstallationRepository {
    records: Mutex<Vec<domain::ExtensionInstallationRecord>>,
}

impl TestExtensionInstallationRepository {
    fn records(&self) -> Vec<domain::ExtensionInstallationRecord> {
        self.records.lock().unwrap().clone()
    }
}

#[async_trait]
impl control_plane::ports::ExtensionInstallationRepository for TestExtensionInstallationRepository {
    async fn upsert_extension_installation(
        &self,
        input: &control_plane::ports::UpsertExtensionInstallationInput,
    ) -> anyhow::Result<domain::ExtensionInstallationRecord> {
        let mut records = self.records.lock().unwrap();
        if input.is_current {
            for record in records.iter_mut().filter(|record| {
                record.identity.category == input.identity.category
                    && record.identity.organization == input.identity.organization
                    && record.identity.artifact_id == input.identity.artifact_id
                    && record.identity.node_id == input.identity.node_id
            }) {
                record.is_current = false;
            }
        }
        let now = time::OffsetDateTime::now_utc();
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
            application_action: input.application_action,
            status: input.status,
            is_current: input.is_current,
            installed_by: input.installed_by,
            created_at: now,
            updated_at: now,
        };
        if let Some(existing) = records
            .iter_mut()
            .find(|existing| existing.identity == record.identity)
        {
            *existing = record.clone();
        } else {
            records.push(record.clone());
        }
        Ok(record)
    }

    async fn find_extension_installation(
        &self,
        identity: &domain::ExtensionInstallationIdentity,
    ) -> anyhow::Result<Option<domain::ExtensionInstallationRecord>> {
        Ok(self
            .records
            .lock()
            .unwrap()
            .iter()
            .find(|record| &record.identity == identity)
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
            .iter()
            .find(|record| record.identity.node_id == node_id && record.id == installation_id)
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
            .iter()
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
            .iter_mut()
            .find(|record| record.id == installation_id)
        {
            record.status = status;
            if status == domain::ExtensionInstallationStatus::Missing {
                record.is_current = false;
            }
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
            .iter()
            .find(|record| record.identity.node_id == node_id && record.id == installation_id)
            .cloned()
        else {
            return Ok(None);
        };
        for record in records.iter_mut().filter(|record| {
            record.identity.node_id == node_id
                && record.identity.category == target.identity.category
                && record.identity.organization == target.identity.organization
                && record.identity.artifact_id == target.identity.artifact_id
        }) {
            record.is_current = record.id == installation_id;
        }
        Ok(records
            .iter()
            .find(|record| record.id == installation_id)
            .cloned())
    }

    async fn remove_extension_installation(
        &self,
        node_id: &str,
        installation_id: Uuid,
    ) -> anyhow::Result<Option<domain::ExtensionInstallationRecord>> {
        let mut records = self.records.lock().unwrap();
        let Some(record) = records
            .iter_mut()
            .find(|record| record.identity.node_id == node_id && record.id == installation_id)
        else {
            return Ok(None);
        };
        record.status = domain::ExtensionInstallationStatus::Missing;
        record.is_current = false;
        Ok(Some(record.clone()))
    }
}

fn catalog(base: &str, checksum: &str, signature: &str) -> serde_json::Value {
    json!({
        "schema_version": "1flowbase.mcp-catalog/v2",
        "bundles": [{
            "organization": "taichuy",
            "bundle_id": "zh_hans",
            "source_path": "./",
            "versions": [{
                "bundle_version": "1.2.0",
                "locale": "zh_Hans",
                "minimum_host_version": "0.3.1",
                "exported_from_system_version": "0.3.1",
                "release_tag": "mcp-taichuy-zh_hans-v1.1.1",
                "download_url": format!("{base}/bundle.zip"),
                "checksum": checksum,
                "algorithm": "ed25519",
                "key_id": "fixture-key",
                "signature": signature
            }, {
                "bundle_version": "1.10.0",
                "locale": "zh_Hans",
                "minimum_host_version": "0.3.1",
                "exported_from_system_version": "0.3.1",
                "release_tag": "mcp-taichuy-zh_hans-v1.10.0",
                "download_url": format!("{base}/bundle.zip"),
                "checksum": checksum,
                "algorithm": "ed25519",
                "key_id": "fixture-key",
                "signature": signature
            }]
        }]
    })
}

fn temp_root() -> std::path::PathBuf {
    std::env::temp_dir().join(format!("mcp-library-{}", uuid::Uuid::now_v7()))
}
