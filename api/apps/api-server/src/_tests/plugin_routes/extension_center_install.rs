//! Real HTTP + PostgreSQL intake: public catalog identity is independent of installation routing.
use crate::_tests::support::{login_and_capture_cookie, test_app_with_official_extension_source};
use crate::official_extension_catalog::{
    DownloadedOfficialExtensionArtifact, LocatedOfficialExtensionCatalogEntry,
    OfficialExtensionArtifactDescriptor, OfficialExtensionCatalogEntry,
    OfficialExtensionCatalogEntrySource, OfficialExtensionCatalogFreshness,
    OfficialExtensionCatalogPage, OfficialExtensionCatalogSearchQuery,
    OfficialExtensionCatalogSearchResult, OfficialExtensionCatalogSourcePort,
};
use async_trait::async_trait;
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    Router,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, path::PathBuf, sync::Arc};
use tower::ServiceExt;

#[derive(Clone)]
struct ManagedCatalog {
    entry: OfficialExtensionCatalogEntry,
    bytes: Vec<u8>,
}

impl ManagedCatalog {
    fn new() -> Self {
        let fixture = crate::api_workspace_root()
            .unwrap()
            .join("plugins/fixtures/acme.notification");
        let worker = PathBuf::from(
            std::env::var_os("NOTIFICATION_WORKER_FIXTURE")
                .expect("prebuild the real SDK notification_worker"),
        );
        assert!(worker.is_absolute() && worker.is_file());
        let mut archive = tar::Builder::new(flate2::write::GzEncoder::new(
            Vec::new(),
            flate2::Compression::default(),
        ));
        archive
            .append_path_with_name(fixture.join("manifest.yaml"), "manifest.yaml")
            .unwrap();
        archive.append_path_with_name(worker, "bin/worker").unwrap();
        let bytes = archive.into_inner().unwrap().finish().unwrap();
        Self {
            entry: OfficialExtensionCatalogEntry {
                id: "runtime-extensions:acme/notification-fixture".into(),
                name: "Managed notification".into(),
                category: "runtime-extensions".into(),
                organization: "acme".into(),
                artifact: "notification-fixture".into(),
                version: "1.0.0".into(),
                description: "SDK notification install fixture".into(),
                host_version_requirement: ">=0.1.0".into(),
                slot_codes: vec![],
                keywords: vec![],
                source: OfficialExtensionCatalogEntrySource {
                    kind: "runtime_extension_manifest".into(),
                    locator: "fixture://notification/manifest.yaml".into(),
                    metadata: BTreeMap::from([
                        ("plugin_id".into(), json!("acme.notification-fixture")),
                        ("plugin_type".into(), json!("runtime_extension")),
                    ]),
                },
                signature: None,
                checksum: Some(format!(
                    "sha256:{:x}",
                    base16ct::HexDisplay(&Sha256::digest(&bytes))
                )),
                download_locator: json!({"kind":"https","locator":"https://example.test/notification.1flowbasepkg"}),
                catalog_page: 1,
            },
            bytes,
        }
    }

    fn request_body(&self) -> Value {
        json!({
            "category":self.entry.category, "catalog_id":self.entry.id,
            "version":self.entry.version,
            "risk_override":{
                "reason":"isolated unsigned SDK fixture",
                "acknowledged_warnings":["signature_missing"]
            }
        })
    }
}

#[async_trait]
impl OfficialExtensionCatalogSourcePort for ManagedCatalog {
    async fn search(
        &self,
        category: &str,
        _: OfficialExtensionCatalogSearchQuery,
    ) -> anyhow::Result<OfficialExtensionCatalogSearchResult> {
        let entries = (category == self.entry.category)
            .then(|| self.entry.clone())
            .into_iter()
            .collect::<Vec<_>>();
        Ok(OfficialExtensionCatalogSearchResult {
            category: category.into(),
            source_kind: "official_repository".into(),
            freshness: OfficialExtensionCatalogFreshness::Fresh,
            snapshot_checksum: "sha256:fixture".into(),
            snapshot_locator: "fixture://catalog".into(),
            total_entries: entries.len(),
            next_cursor: None,
            entries,
        })
    }
    async fn list_page(
        &self,
        _: &str,
        _: Option<&str>,
    ) -> anyhow::Result<OfficialExtensionCatalogPage> {
        anyhow::bail!("page not used by install fixture")
    }
    async fn find_entry(
        &self,
        category: &str,
        catalog_id: &str,
    ) -> anyhow::Result<Option<LocatedOfficialExtensionCatalogEntry>> {
        Ok(
            (category == self.entry.category && catalog_id == self.entry.id).then(|| {
                LocatedOfficialExtensionCatalogEntry {
                    source_kind: "official_repository".into(),
                    entry: self.entry.clone(),
                }
            }),
        )
    }
    fn resolve_artifact(
        &self,
        _: &OfficialExtensionCatalogEntry,
    ) -> anyhow::Result<OfficialExtensionArtifactDescriptor> {
        Ok(OfficialExtensionArtifactDescriptor {
            locator_kind: "https".into(),
            locator: "https://example.test/notification.1flowbasepkg".into(),
            expected_checksum: self.entry.checksum.clone(),
            signature: None,
            platform: None,
        })
    }
    async fn download_artifact(
        &self,
        entry: &OfficialExtensionCatalogEntry,
    ) -> anyhow::Result<DownloadedOfficialExtensionArtifact> {
        Ok(DownloadedOfficialExtensionArtifact {
            file_name: "notification.1flowbasepkg".into(),
            artifact_bytes: self.bytes.clone(),
            descriptor: self.resolve_artifact(entry)?,
        })
    }
}

async fn install(app: &Router, cookie: &str, csrf: &str, body: &Value) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/settings/extension-center/install")
                .header("cookie", cookie)
                .header("x-csrf-token", csrf)
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let body =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
    (status, body)
}

async fn installed(app: &Router, cookie: &str) -> Vec<Value> {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/console/settings/extension-center/installed?limit=100")
                .header("cookie", cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
    body["data"]["entries"].as_array().unwrap().clone()
}

#[tokio::test]
async fn managed_runtime_catalog_install_succeeds_and_is_idempotent() {
    let source = ManagedCatalog::new();
    let body = source.request_body();
    let app = test_app_with_official_extension_source(Arc::new(source)).await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let (status, first) = install(&app, &cookie, &csrf, &body).await;
    assert_eq!(status, StatusCode::CREATED, "{first}");
    assert_eq!(
        first["data"]["installation"]["artifact_id"],
        "notification-fixture"
    );
    assert_eq!(first["data"]["installation"]["version"], "1.0.0");
    let (status, second) = install(&app, &cookie, &csrf, &body).await;
    assert_eq!(status, StatusCode::OK, "{second}");
    assert_eq!(
        first["data"]["installation"]["id"],
        second["data"]["installation"]["id"]
    );
    assert_eq!(second["data"]["local_artifact_was_present"], true);
    assert_eq!(
        installed(&app, &cookie)
            .await
            .iter()
            .filter(|row| row["artifact_id"] == "notification-fixture")
            .count(),
        1
    );
}

#[tokio::test]
async fn managed_runtime_catalog_mismatches_reject_before_installation() {
    let baseline = ManagedCatalog::new();
    for mismatch in ["type", "plugin_id", "organization", "version", "category"] {
        let mut source = baseline.clone();
        match mismatch {
            "type" => {
                source
                    .entry
                    .source
                    .metadata
                    .insert("plugin_type".into(), json!("model_provider"));
            }
            "plugin_id" => {
                source
                    .entry
                    .source
                    .metadata
                    .insert("plugin_id".into(), json!("acme.other"));
            }
            "organization" => {
                source.entry.organization = "other".into();
                source.entry.id = "runtime-extensions:other/notification-fixture".into();
            }
            "version" => source.entry.version = "9.9.9".into(),
            "category" => {
                source.entry.category = "capability-plugins".into();
                source.entry.id = "capability-plugins:acme/notification-fixture".into();
            }
            _ => unreachable!(),
        }
        let body = source.request_body();
        let app = test_app_with_official_extension_source(Arc::new(source)).await;
        let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
        let (status, response) = install(&app, &cookie, &csrf, &body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{mismatch}: {response}");
        assert_eq!(
            response["code"], "extension_artifact_identity",
            "{mismatch}: {response}"
        );
        assert!(
            installed(&app, &cookie)
                .await
                .iter()
                .all(|row| row["artifact_id"] != "notification-fixture"),
            "{mismatch}"
        );
    }
}
