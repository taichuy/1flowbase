use std::sync::{Arc, Mutex};

use anyhow::Result;
use async_trait::async_trait;
use uuid::Uuid;

use crate::{
    _tests::support::{
        create_member, login_and_capture_cookie, test_app, test_app_with_official_extension_source,
        InMemoryOfficialExtensionCatalogSource,
    },
    official_extension_catalog::{
        DownloadedOfficialExtensionArtifact, LocatedOfficialExtensionCatalogEntry,
        OfficialExtensionArtifactDescriptor, OfficialExtensionCatalogEntry,
        OfficialExtensionCatalogPage, OfficialExtensionCatalogSearchQuery,
        OfficialExtensionCatalogSearchResult, OfficialExtensionCatalogSourcePort,
    },
};
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use tower::ServiceExt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CatalogCall {
    DirectSearch,
    WorkspaceSearch(Uuid),
    DirectList,
    WorkspaceList(Uuid),
    DirectDownload,
    WorkspaceDownload(Uuid),
}

#[derive(Clone, Default)]
struct WorkspaceScopedCatalogRecorder {
    inner: InMemoryOfficialExtensionCatalogSource,
    calls: Arc<Mutex<Vec<CatalogCall>>>,
}

impl WorkspaceScopedCatalogRecorder {
    fn record(&self, call: CatalogCall) {
        self.calls
            .lock()
            .expect("catalog call recorder should not be poisoned")
            .push(call);
    }

    fn calls(&self) -> Vec<CatalogCall> {
        self.calls
            .lock()
            .expect("catalog call recorder should not be poisoned")
            .clone()
    }
}

#[async_trait]
impl OfficialExtensionCatalogSourcePort for WorkspaceScopedCatalogRecorder {
    async fn search(
        &self,
        category: &str,
        query: OfficialExtensionCatalogSearchQuery,
    ) -> Result<OfficialExtensionCatalogSearchResult> {
        self.record(CatalogCall::DirectSearch);
        self.inner.search(category, query).await
    }

    async fn search_for_workspace(
        &self,
        workspace_id: Uuid,
        category: &str,
        query: OfficialExtensionCatalogSearchQuery,
    ) -> Result<OfficialExtensionCatalogSearchResult> {
        self.record(CatalogCall::WorkspaceSearch(workspace_id));
        self.inner
            .search_for_workspace(workspace_id, category, query)
            .await
    }

    async fn list_page(
        &self,
        category: &str,
        cursor: Option<&str>,
    ) -> Result<OfficialExtensionCatalogPage> {
        self.record(CatalogCall::DirectList);
        self.inner.list_page(category, cursor).await
    }

    async fn list_page_for_workspace(
        &self,
        workspace_id: Uuid,
        category: &str,
        cursor: Option<&str>,
    ) -> Result<OfficialExtensionCatalogPage> {
        self.record(CatalogCall::WorkspaceList(workspace_id));
        self.inner
            .list_page_for_workspace(workspace_id, category, cursor)
            .await
    }

    async fn find_entry(
        &self,
        category: &str,
        catalog_id: &str,
    ) -> Result<Option<LocatedOfficialExtensionCatalogEntry>> {
        self.inner.find_entry(category, catalog_id).await
    }

    fn resolve_artifact(
        &self,
        entry: &OfficialExtensionCatalogEntry,
    ) -> Result<OfficialExtensionArtifactDescriptor> {
        self.inner.resolve_artifact(entry)
    }

    async fn download_artifact(
        &self,
        entry: &OfficialExtensionCatalogEntry,
    ) -> Result<DownloadedOfficialExtensionArtifact> {
        self.record(CatalogCall::DirectDownload);
        self.inner.download_artifact(entry).await
    }

    async fn download_artifact_for_workspace(
        &self,
        workspace_id: Uuid,
        entry: &OfficialExtensionCatalogEntry,
    ) -> Result<DownloadedOfficialExtensionArtifact> {
        self.record(CatalogCall::WorkspaceDownload(workspace_id));
        self.inner
            .download_artifact_for_workspace(workspace_id, entry)
            .await
    }
}

/// AC-002: the Network Center catalog itself is a workspace-scoped GitHub consumer, not a
/// direct-path precursor to the routed artifact download.
#[tokio::test]
async fn network_center_official_catalog_uses_workspace_scoped_search() {
    let source = WorkspaceScopedCatalogRecorder::default();
    let app = test_app_with_official_extension_source(Arc::new(source.clone())).await;
    let (cookie, _) = login_and_capture_cookie(&app, "root", "change-me").await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/console/settings/network-center/proxy-plugins/official-catalog")
                .header("cookie", cookie)
                .body(Body::empty())
                .expect("request should be valid"),
        )
        .await
        .expect("router should return a response");

    assert_eq!(response.status(), StatusCode::OK);
    let calls = source.calls();
    let [CatalogCall::WorkspaceSearch(workspace_id)] = calls.as_slice() else {
        panic!("official catalog bypassed workspace-scoped search: {calls:?}");
    };
    assert_ne!(*workspace_id, Uuid::nil());
}

/// AC-001/002/003: authenticated official installs must keep the current workspace on both
/// catalog resolution and artifact download so the GitHub egress route cannot be bypassed.
#[tokio::test]
async fn network_center_official_install_uses_one_workspace_scoped_catalog_path() {
    let source = WorkspaceScopedCatalogRecorder::default();
    let app = test_app_with_official_extension_source(Arc::new(source.clone())).await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;

    let _response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/settings/network-center/proxy-plugins/install-official")
                .header("cookie", cookie)
                .header("x-csrf-token", csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "plugin_id": "1flowbase.openai_compatible",
                        "risk_override": {
                            "reason": "route fixture accepts unsigned package",
                            "acknowledged_warnings": ["signature_missing"]
                        }
                    })
                    .to_string(),
                ))
                .expect("request should be valid"),
        )
        .await
        .expect("router should return a response");

    let calls = source.calls();
    let [CatalogCall::WorkspaceList(list_workspace), CatalogCall::WorkspaceDownload(download_workspace)] =
        calls.as_slice()
    else {
        panic!("official install bypassed workspace-scoped catalog operations: {calls:?}");
    };
    assert_eq!(list_workspace, download_workspace);
}

/// AC-014: a valid console session without the registered SettingsFeature must be rejected by
/// the real route before it can observe the provider registry.
#[tokio::test]
async fn network_center_provider_registry_rejects_session_without_feature_scope() {
    let app = test_app().await;
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    create_member(
        &app,
        &root_cookie,
        &root_csrf,
        "network-center-without-scope",
        "temp-pass",
    )
    .await;
    let (cookie, _) =
        login_and_capture_cookie(&app, "network-center-without-scope", "temp-pass").await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/console/settings/network-center/providers")
                .header("cookie", cookie)
                .body(Body::empty())
                .expect("request should be valid"),
        )
        .await
        .expect("router should return a response");

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

/// AC-GP04: the proxy plugin catalog is a Network Center capability, not an extension-center
/// backdoor that happens to be rendered on the proxy types page.
#[tokio::test]
async fn network_center_proxy_plugin_catalog_rejects_session_without_feature_scope() {
    let app = test_app().await;
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    create_member(
        &app,
        &root_cookie,
        &root_csrf,
        "network-plugins-without-scope",
        "temp-pass",
    )
    .await;
    let (cookie, _) =
        login_and_capture_cookie(&app, "network-plugins-without-scope", "temp-pass").await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/console/settings/network-center/proxy-plugins/official-catalog")
                .header("cookie", cookie)
                .body(Body::empty())
                .expect("request should be valid"),
        )
        .await
        .expect("router should return a response");

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

/// AC-NCP02: version history is governed by the same Network Center feature scope as the
/// official catalog; it must not become a model-provider management backdoor.
#[tokio::test]
async fn network_center_proxy_plugin_families_reject_session_without_feature_scope() {
    let app = test_app().await;
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    create_member(
        &app,
        &root_cookie,
        &root_csrf,
        "network-plugin-families-without-scope",
        "temp-pass",
    )
    .await;
    let (cookie, _) =
        login_and_capture_cookie(&app, "network-plugin-families-without-scope", "temp-pass").await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/console/settings/network-center/proxy-plugins/families")
                .header("cookie", cookie)
                .body(Body::empty())
                .expect("request should be valid"),
        )
        .await
        .expect("router should return a response");

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

/// AC-NCP03: removing a proxy plugin family is a Network Center action and must not be
/// reachable by a session that lacks the Network Center SettingsFeature.
#[tokio::test]
async fn network_center_proxy_plugin_family_uninstall_rejects_session_without_feature_scope() {
    let app = test_app().await;
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    create_member(
        &app,
        &root_cookie,
        &root_csrf,
        "network-plugin-family-uninstall-without-scope",
        "temp-pass",
    )
    .await;
    let (cookie, csrf) = login_and_capture_cookie(
        &app,
        "network-plugin-family-uninstall-without-scope",
        "temp-pass",
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/console/settings/network-center/proxy-plugins/families/clash-proxy")
                .header("cookie", cookie)
                .header("x-csrf-token", csrf)
                .body(Body::empty())
                .expect("request should be valid"),
        )
        .await
        .expect("router should return a response");

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

/// AC-015: pool state is protected by the same backend-owned Network Center feature scope.
#[tokio::test]
async fn network_center_pool_registry_rejects_session_without_feature_scope() {
    let app = test_app().await;
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    create_member(
        &app,
        &root_cookie,
        &root_csrf,
        "network-pool-without-scope",
        "temp-pass",
    )
    .await;
    let (cookie, _) =
        login_and_capture_cookie(&app, "network-pool-without-scope", "temp-pass").await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/console/network-center/pools")
                .header("cookie", cookie)
                .body(Body::empty())
                .expect("request should be valid"),
        )
        .await
        .expect("router should return a response");

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

/// AC-GP03: creating a proxy is a Network Center action and must not be exposed solely because
/// the route happens to live below the pool URL.
#[tokio::test]
async fn network_center_proxy_creation_rejects_session_without_feature_scope() {
    let app = test_app().await;
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    create_member(
        &app,
        &root_cookie,
        &root_csrf,
        "network-proxy-create-without-scope",
        "temp-pass",
    )
    .await;
    let (cookie, csrf) =
        login_and_capture_cookie(&app, "network-proxy-create-without-scope", "temp-pass").await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/network-center/pools/proxies")
                .header("cookie", cookie)
                .header("x-csrf-token", csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"provider_code":"builtin_static_http","display_name":"Blocked","config":{"host":"198.65.36.212","port":"37867"}}"#,
                ))
                .expect("request should be valid"),
        )
        .await
        .expect("router should return a response");

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

/// AC-OP05: connection tests remain a Network Center operation and never become a public proxy
/// endpoint merely because the browser has a pool-member id.
#[tokio::test]
async fn network_center_connection_test_rejects_session_without_feature_scope() {
    let app = test_app().await;
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    create_member(
        &app,
        &root_cookie,
        &root_csrf,
        "network-probe-without-scope",
        "temp-pass",
    )
    .await;
    let (cookie, csrf) =
        login_and_capture_cookie(&app, "network-probe-without-scope", "temp-pass").await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/network-center/pools/00000000-0000-0000-0000-000000000001/members/00000000-0000-0000-0000-000000000002/test-connection")
                .header("cookie", cookie)
                .header("x-csrf-token", csrf)
                .body(Body::empty())
                .expect("request should be valid"),
        )
        .await
        .expect("router should return a response");

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

/// AC-015: the route selector API is owned by the existing Network Center SettingsFeature too.
#[tokio::test]
async fn network_center_route_registry_rejects_session_without_feature_scope() {
    let app = test_app().await;
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    create_member(
        &app,
        &root_cookie,
        &root_csrf,
        "network-route-without-scope",
        "temp-pass",
    )
    .await;
    let (cookie, _) =
        login_and_capture_cookie(&app, "network-route-without-scope", "temp-pass").await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/console/network-center/routes")
                .header("cookie", cookie)
                .body(Body::empty())
                .expect("request should be valid"),
        )
        .await
        .expect("router should return a response");

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}
