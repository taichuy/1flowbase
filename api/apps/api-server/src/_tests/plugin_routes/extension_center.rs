use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

use crate::_tests::support::{
    login_and_capture_cookie, test_app, test_app_with_official_extension_source,
};
use crate::official_extension_catalog::{
    DownloadedOfficialExtensionArtifact, LocatedOfficialExtensionCatalogEntry,
    OfficialExtensionArtifactDescriptor, OfficialExtensionCatalogEntry,
    OfficialExtensionCatalogEntrySource, OfficialExtensionCatalogPage,
    OfficialExtensionCatalogSearchQuery, OfficialExtensionCatalogSearchResult,
    OfficialExtensionCatalogSourcePort,
};
use async_trait::async_trait;
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use serde_json::{json, Value};
use tower::ServiceExt;

use super::support::{create_member, create_role, replace_member_roles};

#[derive(Clone)]
struct UnavailableArtifactSource {
    requests: Arc<AtomicUsize>,
}

fn unavailable_artifact_entry() -> OfficialExtensionCatalogEntry {
    OfficialExtensionCatalogEntry {
        id: "i18n:taichuy/publisher-cutover".to_string(),
        name: "Publisher Cutover".to_string(),
        category: "i18n".to_string(),
        organization: "taichuy".to_string(),
        artifact: "publisher-cutover".to_string(),
        version: "1.0.0".to_string(),
        description: "Publisher cutover fixture".to_string(),
        host_version_requirement: ">=0.1.0".to_string(),
        slot_codes: Vec::new(),
        keywords: Vec::new(),
        source: OfficialExtensionCatalogEntrySource {
            kind: "repository_file".to_string(),
            locator: "i18n/@taichuy/publisher-cutover".to_string(),
            metadata: Default::default(),
        },
        signature: None,
        checksum: Some(format!("sha256:{}", "0".repeat(64))),
        download_locator: json!({
            "kind": "https",
            "locator": "https://example.test/publisher-cutover.bin"
        }),
        catalog_page: 1,
    }
}

#[async_trait]
impl OfficialExtensionCatalogSourcePort for UnavailableArtifactSource {
    async fn search(
        &self,
        _category: &str,
        _query: OfficialExtensionCatalogSearchQuery,
    ) -> anyhow::Result<OfficialExtensionCatalogSearchResult> {
        anyhow::bail!("not used")
    }

    async fn list_page(
        &self,
        _category: &str,
        _cursor: Option<&str>,
    ) -> anyhow::Result<OfficialExtensionCatalogPage> {
        anyhow::bail!("not used")
    }

    async fn find_entry(
        &self,
        category: &str,
        catalog_id: &str,
    ) -> anyhow::Result<Option<LocatedOfficialExtensionCatalogEntry>> {
        let entry = unavailable_artifact_entry();
        Ok(
            (category == entry.category && catalog_id == entry.id).then_some(
                LocatedOfficialExtensionCatalogEntry {
                    source_kind: "official_repository".to_string(),
                    entry,
                },
            ),
        )
    }

    fn resolve_artifact(
        &self,
        _entry: &OfficialExtensionCatalogEntry,
    ) -> anyhow::Result<OfficialExtensionArtifactDescriptor> {
        Ok(OfficialExtensionArtifactDescriptor {
            locator_kind: "https".to_string(),
            locator: "https://example.test/publisher-cutover.bin".to_string(),
            expected_checksum: Some(format!("sha256:{}", "0".repeat(64))),
            signature: None,
            platform: None,
        })
    }

    async fn download_artifact(
        &self,
        _entry: &OfficialExtensionCatalogEntry,
    ) -> anyhow::Result<DownloadedOfficialExtensionArtifact> {
        self.requests.fetch_add(1, Ordering::SeqCst);
        anyhow::bail!("publisher cutover artifact unavailable")
    }
}

#[tokio::test]
async fn publisher_cutover_download_failure_returns_stable_502_without_install() {
    let requests = Arc::new(AtomicUsize::new(0));
    let app = test_app_with_official_extension_source(Arc::new(UnavailableArtifactSource {
        requests: Arc::clone(&requests),
    }))
    .await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/settings/extension-center/install")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "category": "i18n",
                        "catalog_id": "i18n:taichuy/publisher-cutover",
                        "version": "1.0.0",
                        "risk_override": {
                            "reason": "test download mapping",
                            "acknowledged_warnings": ["signature_missing"]
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    let payload: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(payload["code"], "extension_artifact_download_unavailable");
    assert_eq!(requests.load(Ordering::SeqCst), 1);

    let installed = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/console/settings/extension-center/installed?category=i18n")
                .header("cookie", cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let installed_payload: Value =
        serde_json::from_slice(&to_bytes(installed.into_body(), usize::MAX).await.unwrap())
            .unwrap();
    assert!(installed_payload["data"]["entries"]
        .as_array()
        .unwrap()
        .iter()
        .all(|entry| entry["artifact_id"] != "publisher-cutover"));
}

#[tokio::test]
async fn root_1545_ac_3_installed_route_reads_generic_inventory_shape() {
    let app = test_app().await;
    let (cookie, _) = login_and_capture_cookie(&app, "root", "change-me").await;
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/console/settings/extension-center/installed?limit=20")
                .header("cookie", cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let payload: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(payload["data"]["limit"], 20);
    assert!(payload["data"]["entries"].is_array());
}

#[tokio::test]
async fn root_1545_ac_5_install_route_rejects_missing_csrf_before_catalog_network() {
    let app = test_app().await;
    let (cookie, _) = login_and_capture_cookie(&app, "root", "change-me").await;
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/settings/extension-center/install")
                .header("cookie", cookie)
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
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let payload: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(payload["code"], "not_authenticated");
}

#[tokio::test]
async fn root_1545_ac_5_installed_route_respects_console_operation_acl() {
    let app = test_app().await;
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let member_id = create_member(
        &app,
        &root_cookie,
        &root_csrf,
        "extension-no-access",
        "temp-pass",
    )
    .await;
    create_role(&app, &root_cookie, &root_csrf, "extension_no_access").await;
    replace_member_roles(
        &app,
        &root_cookie,
        &root_csrf,
        &member_id,
        &["extension_no_access"],
    )
    .await;
    let (cookie, _) = login_and_capture_cookie(&app, "extension-no-access", "temp-pass").await;
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/console/settings/extension-center/installed")
                .header("cookie", cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}
