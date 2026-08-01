use std::{
    collections::{BTreeMap, HashMap},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};

use async_trait::async_trait;
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use control_plane::{
    plugin_management::{
        group_installed_extension_families, ExtensionArtifactInstallOutcome,
        ExtensionInstallationService, InstallExtensionArtifactCommand,
    },
    ports::AuthRepository,
};
use serde_json::json;
use sha2::{Digest, Sha256};
use time::OffsetDateTime;
use tower::ServiceExt;
use uuid::Uuid;

use crate::official_extension_catalog::{
    DownloadedOfficialExtensionArtifact, LocatedOfficialExtensionCatalogEntry,
    OfficialExtensionArtifactDescriptor, OfficialExtensionCatalogEntry,
    OfficialExtensionCatalogEntrySource, OfficialExtensionCatalogPage,
    OfficialExtensionCatalogSourcePort,
};

use super::upload::upload_challenge;
use super::{
    artifact_preflight_challenge, extension_update_status, paginate_installed_families,
    project_catalog_entry, project_installed_catalog_joins, requested_installation_identity,
    validate_preflight_overrides, InstalledCatalogJoin, PreflightDecision,
    UploadedExtensionArtifact,
};

#[test]
fn root_1545_ac_4_catalog_projection_joins_real_local_version_by_stable_plugin_identity() {
    let entry = runtime_entry();
    let installed = HashMap::from([(
        "runtime-extensions:taichuy/openai".to_string(),
        InstalledCatalogJoin {
            current_version: "0.1.32-dev".to_string(),
            source: "upload".to_string(),
            trust: "unknown".to_string(),
        },
    )]);

    let response = project_catalog_entry(
        entry,
        "official",
        &installed,
        &["official-key-2026-04".to_string()],
    );

    assert_eq!(response.version, "0.2.0");
    assert_eq!(response.current_version.as_deref(), Some("0.1.32-dev"));
    assert_eq!(response.installation_status, "installed");
    assert_eq!(response.installation_source.as_deref(), Some("upload"));
    assert_eq!(response.trust, "unknown");
    assert!(response.artifact_kind.is_none());
}

#[test]
fn root_1545_bf1_canonical_catalog_identity_rejects_category_mismatch_and_traversal() {
    let identity = requested_installation_identity(
        super::ExtensionCatalogCategory::RuntimeExtensions,
        "runtime-extensions:taichuy/openai",
        "1.2.0",
        "node-a",
    )
    .unwrap();
    assert_eq!(identity.catalog_id(), "runtime-extensions:taichuy/openai");
    assert_eq!(identity.artifact_id, "openai");
    assert!(requested_installation_identity(
        super::ExtensionCatalogCategory::I18n,
        "runtime-extensions:taichuy/openai",
        "1.2.0",
        "node-a",
    )
    .is_err());
    assert!(requested_installation_identity(
        super::ExtensionCatalogCategory::RuntimeExtensions,
        "runtime-extensions:../openai",
        "1.2.0",
        "node-a",
    )
    .is_err());
    assert!(requested_installation_identity(
        super::ExtensionCatalogCategory::RuntimeExtensions,
        "runtime-extensions:taichuy/openai",
        "../1.2.0",
        "node-a",
    )
    .is_err());
}

#[test]
fn root_1545_bf1_catalog_projection_keeps_newest_version_for_stable_identity() {
    let newer = installation_record("1.2.0", OffsetDateTime::now_utc());
    let older = installation_record("1.1.0", OffsetDateTime::now_utc() - time::Duration::DAY);
    let installed = project_installed_catalog_joins(
        [newer, older],
        super::ExtensionCatalogCategory::RuntimeExtensions,
    );
    let response = project_catalog_entry(runtime_entry(), "official", &installed, &[]);
    assert_eq!(response.current_version.as_deref(), Some("1.2.0"));
}

#[test]
fn root_1545_d4_ac_13_paginates_installed_families_instead_of_version_records() {
    let now = OffsetDateTime::now_utc();
    let mut anthropic_old = installation_record("0.1.18", now);
    anthropic_old.identity.artifact_id = "anthropic".to_string();
    let mut anthropic_current = installation_record("0.1.23", now);
    anthropic_current.identity.artifact_id = "anthropic".to_string();
    let mut deepseek = installation_record("0.1.15", now);
    deepseek.identity.artifact_id = "deepseek".to_string();
    let families = group_installed_extension_families([anthropic_old, deepseek, anthropic_current]);

    let (total, next_cursor, first_page) = paginate_installed_families(families, None, 1);
    assert_eq!(total, 2);
    assert_eq!(first_page.len(), 1);
    assert_eq!(first_page[0].installed_versions.len(), 2);
    assert_eq!(
        next_cursor.as_deref(),
        Some("runtime-extensions:taichuy/anthropic")
    );
}

#[test]
fn root_1545_d4_ac_16_treats_catalog_latest_as_current_when_any_local_version_matches() {
    let installed_versions = vec!["0.1.23".to_string(), "0.1.22".to_string()];
    assert_eq!(
        extension_update_status(Some("0.1.22"), &installed_versions),
        "current"
    );
    assert_eq!(
        extension_update_status(Some("0.1.24"), &installed_versions),
        "update_available"
    );
    assert_eq!(
        extension_update_status(None, &installed_versions),
        "unknown_error"
    );
}

fn installation_record(
    version: &str,
    updated_at: OffsetDateTime,
) -> domain::ExtensionInstallationRecord {
    domain::ExtensionInstallationRecord {
        id: Uuid::now_v7(),
        identity: domain::ExtensionInstallationIdentity {
            category: domain::ExtensionCategory::RuntimeExtensions,
            organization: "taichuy".to_string(),
            artifact_id: "openai".to_string(),
            version: version.to_string(),
            node_id: "node-a".to_string(),
        },
        source: "official".to_string(),
        trust: "official".to_string(),
        local_path: "/tmp/openai".to_string(),
        checksum: "sha256:fixture".to_string(),
        signature_status: domain::ExtensionSignatureStatus::Verified,
        signature_algorithm: Some("ed25519".to_string()),
        signing_key_id: Some("official-key-2026-04".to_string()),
        warnings: Vec::new(),
        receipt: json!({}),
        status: domain::ExtensionInstallationStatus::Installed,
        installed_by: Uuid::now_v7(),
        created_at: updated_at,
        updated_at,
    }
}

#[derive(Clone)]
struct CountingCatalogSource {
    calls: Arc<AtomicUsize>,
}

#[async_trait]
impl OfficialExtensionCatalogSourcePort for CountingCatalogSource {
    async fn list_page(
        &self,
        _category: &str,
        _cursor: Option<&str>,
    ) -> anyhow::Result<OfficialExtensionCatalogPage> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        anyhow::bail!("catalog network must not be called for an exact local version")
    }

    async fn find_entry(
        &self,
        _category: &str,
        _catalog_id: &str,
    ) -> anyhow::Result<Option<LocatedOfficialExtensionCatalogEntry>> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        anyhow::bail!("catalog network must not be called for an exact local version")
    }

    fn resolve_artifact(
        &self,
        _entry: &OfficialExtensionCatalogEntry,
    ) -> anyhow::Result<OfficialExtensionArtifactDescriptor> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        anyhow::bail!("catalog network must not be called for an exact local version")
    }

    async fn download_artifact(
        &self,
        _entry: &OfficialExtensionCatalogEntry,
    ) -> anyhow::Result<DownloadedOfficialExtensionArtifact> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        anyhow::bail!("catalog network must not be called for an exact local version")
    }
}

#[tokio::test]
async fn root_1545_bf1_exact_local_version_returns_without_catalog_network() {
    let (mut state, _) = crate::_tests::support::test_api_state_with_database_url().await;
    let actor = AuthRepository::find_user_for_password_login(
        &state.store,
        domain::PASSWORD_LOCAL_AUTHENTICATOR_ID,
        "root",
    )
    .await
    .unwrap()
    .unwrap();
    let bytes = b"local-i18n".to_vec();
    let outcome =
        ExtensionInstallationService::new(state.store.clone(), &state.provider_install_root)
            .install_from_bytes(InstallExtensionArtifactCommand {
                actor_user_id: actor.id,
                category: super::ExtensionCatalogCategory::I18n,
                organization: "taichuy".to_string(),
                artifact_id: "platform".to_string(),
                version: "2.0.1".to_string(),
                node_id: state.api_node_id.clone(),
                artifact_bytes: bytes.clone(),
                source: "official".to_string(),
                trust: "official".to_string(),
                expected_checksum: Some(format!("sha256:{:x}", Sha256::digest(&bytes))),
                signature_status: domain::ExtensionSignatureStatus::Verified,
                signature_algorithm: Some("ed25519".to_string()),
                signing_key_id: Some("official-key-2026-04".to_string()),
                declared_warnings: Vec::new(),
                risk_override: None,
                confirmation_receipt: None,
            })
            .await
            .unwrap();
    assert!(matches!(
        outcome,
        ExtensionArtifactInstallOutcome::Installed { .. }
    ));

    let calls = Arc::new(AtomicUsize::new(0));
    Arc::get_mut(&mut state)
        .unwrap()
        .official_extension_catalog_source = Arc::new(CountingCatalogSource {
        calls: Arc::clone(&calls),
    });
    let app = crate::app_with_state_and_config(state, &crate::_tests::support::test_config());
    let (cookie, csrf) =
        crate::_tests::support::login_and_capture_cookie(&app, "root", "change-me").await;
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/settings/extension-center/install")
                .header("cookie", cookie)
                .header("x-csrf-token", csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "category": "i18n",
                        "catalog_id": "i18n:taichuy/platform",
                        "version": "2.0.1"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let payload: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(
        payload["data"]["installation"]["catalog_id"],
        "i18n:taichuy/platform"
    );
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[test]
fn root_1545_ac_4_catalog_projection_does_not_fabricate_uninstalled_artifact_kind() {
    let mut entry = runtime_entry();
    entry.signature = None;
    entry.checksum = None;
    entry.host_version_requirement = ">=99.0.0".to_string();

    let response = project_catalog_entry(entry, "official", &HashMap::new(), &[]);

    assert!(response.current_version.is_none());
    assert_eq!(response.installation_status, "not_installed");
    assert!(response.artifact_kind.is_none());
    assert_eq!(response.trust, "unknown");
    assert_eq!(
        response
            .warnings
            .iter()
            .map(|warning| warning.code.as_str())
            .collect::<Vec<_>>(),
        vec![
            "signature_missing",
            "checksum_missing",
            "below_minimum_host_version"
        ]
    );
    assert!(response
        .warnings
        .iter()
        .all(|warning| warning.overridable && !warning.message.is_empty()));
    let compatibility = response.compatibility.unwrap();
    assert_eq!(compatibility.reason, "below_minimum_host_version");
    assert_eq!(compatibility.minimum_host_version, "99.0.0");
    assert!(!compatibility.current_host_version.is_empty());
}

#[test]
fn root_1545_ac_5_preflight_requires_exact_integrity_and_compatibility_acknowledgement() {
    let mut entry = runtime_entry();
    entry.host_version_requirement = ">=99.0.0".to_string();
    let descriptor = OfficialExtensionArtifactDescriptor {
        locator_kind: "release_asset".to_string(),
        locator: "https://example.test/plugin.1flowbasepkg".to_string(),
        expected_checksum: None,
        signature: None,
        platform: None,
    };
    let challenge = artifact_preflight_challenge(&entry, &descriptor, &[]);
    assert_eq!(
        challenge
            .warnings
            .iter()
            .map(|warning| warning.code.as_str())
            .collect::<Vec<_>>(),
        vec!["checksum_missing", "signature_missing"]
    );
    assert!(matches!(
        validate_preflight_overrides(&challenge, None, None).unwrap(),
        PreflightDecision::Challenge
    ));
    let risk = super::PluginRiskOverrideBody {
        reason: "Operator accepts development artifact".to_string(),
        acknowledged_warnings: vec![
            "checksum_missing".to_string(),
            "signature_missing".to_string(),
        ],
    };
    let compatibility = challenge.compatibility.as_ref().unwrap();
    let compatibility_override = super::PluginCompatibilityOverrideBody {
        reason: compatibility.reason.clone(),
        acknowledged_current_host_version: compatibility.current_host_version.clone(),
        acknowledged_minimum_host_version: compatibility.minimum_host_version.clone(),
    };
    assert!(matches!(
        validate_preflight_overrides(&challenge, Some(&risk), Some(&compatibility_override))
            .unwrap(),
        PreflightDecision::Accepted(Some(_))
    ));
}

#[test]
fn root_1545_ac_5_uploaded_invalid_signature_remains_overridable() {
    let challenge = upload_challenge(&UploadedExtensionArtifact {
        category: super::ExtensionCatalogCategory::I18n,
        organization: "taichuy".to_string(),
        artifact_id: "platform".to_string(),
        version: "2.0.1".to_string(),
        minimum_host_version: None,
        node_plugin: false,
        signature_status: domain::ExtensionSignatureStatus::Invalid,
        signature_algorithm: Some("ed25519".to_string()),
        signing_key_id: Some("unknown".to_string()),
    });
    assert_eq!(
        challenge
            .warnings
            .iter()
            .map(|warning| warning.code.as_str())
            .collect::<Vec<_>>(),
        vec!["checksum_missing", "signature_invalid"]
    );
    assert!(challenge.warnings.iter().all(|warning| warning.overridable));
}

fn runtime_entry() -> OfficialExtensionCatalogEntry {
    OfficialExtensionCatalogEntry {
        id: "runtime-extensions:taichuy/openai".to_string(),
        name: "OpenAI".to_string(),
        category: "runtime-extensions".to_string(),
        organization: "taichuy".to_string(),
        artifact: "openai".to_string(),
        version: "0.2.0".to_string(),
        description: "OpenAI runtime extension".to_string(),
        host_version_requirement: ">=0.3.0".to_string(),
        source: OfficialExtensionCatalogEntrySource {
            kind: "runtime_extension_manifest".to_string(),
            locator: "runtime-extensions/@taichuy/openai/manifest.yaml".to_string(),
            metadata: BTreeMap::from([("plugin_id".to_string(), json!("1flowbase.openai"))]),
        },
        signature: Some(json!({
            "algorithm": "ed25519",
            "key_id": "official-key-2026-04"
        })),
        checksum: Some(format!("sha256:{}", "a".repeat(64))),
        download_locator: json!({
            "kind": "platform_release_assets",
            "artifacts": []
        }),
        catalog_page: 1,
    }
}
