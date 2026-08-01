use std::{
    io::{Cursor, Write},
    sync::Arc,
};

use async_trait::async_trait;
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tower::ServiceExt;
use uuid::Uuid;
use zip::{write::SimpleFileOptions, CompressionMethod, ZipArchive, ZipWriter};

use control_plane::{
    plugin_management::{
        ExtensionArtifactInstallOutcome, ExtensionCatalogCategory, ExtensionInstallationService,
        InstallExtensionArtifactCommand,
    },
    ports::AuthRepository,
};

use crate::_tests::support::{
    get_json, login_and_capture_cookie, test_api_state_with_database_url, test_app,
    test_app_with_runtime_profile_error, test_config,
};

#[derive(Clone)]
struct FailingLegacyMcpCatalogSource;

#[async_trait]
impl crate::official_mcp_bundles::OfficialMcpBundleSourcePort for FailingLegacyMcpCatalogSource {
    async fn list_catalog(
        &self,
    ) -> anyhow::Result<crate::official_mcp_bundles::OfficialMcpBundleCatalogSnapshot> {
        anyhow::bail!("legacy MCP catalog source must not be called")
    }

    async fn download_bundle(
        &self,
        _organization: &str,
        _bundle_id: &str,
    ) -> anyhow::Result<crate::official_mcp_bundles::DownloadedOfficialMcpBundle> {
        anyhow::bail!("legacy MCP catalog source must not be called")
    }
}

#[derive(Clone)]
struct FixtureOfficialMcpExtensionCatalogSource {
    bundle: Arc<Vec<u8>>,
}

fn fixture_official_mcp_entry() -> crate::official_extension_catalog::OfficialExtensionCatalogEntry
{
    crate::official_extension_catalog::OfficialExtensionCatalogEntry {
        id: "mcp:taichuy/test_bundle".to_string(),
        name: "Test bundle".to_string(),
        category: "mcp".to_string(),
        organization: "taichuy".to_string(),
        artifact: "test_bundle".to_string(),
        version: "1.0.0".to_string(),
        description: "Fixture MCP bundle".to_string(),
        host_version_requirement: "0.2.0".to_string(),
        source: crate::official_extension_catalog::OfficialExtensionCatalogEntrySource {
            kind: "mcp_bundle".to_string(),
            locator: "fixture".to_string(),
            metadata: std::collections::BTreeMap::from([
                ("locale".to_string(), json!("zh_Hans")),
                ("exported_from_system_version".to_string(), json!("0.2.0")),
                ("release_tag".to_string(), json!("mcp-test-v1.0.0")),
            ]),
        },
        signature: None,
        checksum: Some("sha256:fixture".to_string()),
        download_locator: json!({"kind":"repository_file","locator":"https://unused.test/bundle.zip"}),
        catalog_page: 1,
    }
}

#[async_trait]
impl crate::official_extension_catalog::OfficialExtensionCatalogSourcePort
    for FixtureOfficialMcpExtensionCatalogSource
{
    async fn list_page(
        &self,
        category: &str,
        _cursor: Option<&str>,
    ) -> anyhow::Result<crate::official_extension_catalog::OfficialExtensionCatalogPage> {
        assert_eq!(category, "mcp");
        Ok(
            crate::official_extension_catalog::OfficialExtensionCatalogPage {
                source_kind: "official_repository".to_string(),
                category: "mcp".to_string(),
                metadata: crate::official_extension_catalog::OfficialExtensionCatalogPageMetadata {
                    page: 1,
                    cursor: "start".to_string(),
                    checksum: "sha256:page".to_string(),
                    locator: "fixture://mcp/page".to_string(),
                    next_cursor: None,
                    page_size: 20,
                    total_entries: 1,
                    freshness:
                        crate::official_extension_catalog::OfficialExtensionCatalogFreshness::Fresh,
                },
                entries: vec![fixture_official_mcp_entry()],
            },
        )
    }

    async fn find_entry(
        &self,
        category: &str,
        catalog_id: &str,
    ) -> anyhow::Result<
        Option<crate::official_extension_catalog::LocatedOfficialExtensionCatalogEntry>,
    > {
        if category != "mcp" || catalog_id != "mcp:taichuy/test_bundle" {
            return Ok(None);
        }
        Ok(Some(
            crate::official_extension_catalog::LocatedOfficialExtensionCatalogEntry {
                source_kind: "official_repository".to_string(),
                entry: fixture_official_mcp_entry(),
            },
        ))
    }

    fn resolve_artifact(
        &self,
        _entry: &crate::official_extension_catalog::OfficialExtensionCatalogEntry,
    ) -> anyhow::Result<crate::official_extension_catalog::OfficialExtensionArtifactDescriptor>
    {
        Ok(
            crate::official_extension_catalog::OfficialExtensionArtifactDescriptor {
                locator_kind: "repository_file".to_string(),
                locator: "fixture://mcp/bundle".to_string(),
                expected_checksum: None,
                signature: None,
                platform: None,
            },
        )
    }

    async fn download_artifact(
        &self,
        entry: &crate::official_extension_catalog::OfficialExtensionCatalogEntry,
    ) -> anyhow::Result<crate::official_extension_catalog::DownloadedOfficialExtensionArtifact>
    {
        Ok(
            crate::official_extension_catalog::DownloadedOfficialExtensionArtifact {
                descriptor: self.resolve_artifact(entry)?,
                file_name: "test_bundle.zip".to_string(),
                artifact_bytes: self.bundle.as_ref().clone(),
            },
        )
    }
}

async fn response_json(response: axum::response::Response) -> Value {
    serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap()
}

#[tokio::test]
async fn mcp_bundle_export_defaults_use_the_backend_system_version() {
    // AC-001: export defaults come from the backend version source of truth.
    let app = test_app().await;
    let (cookie, _) = login_and_capture_cookie(&app, "root", "change-me").await;

    let payload = get_json(&app, "/api/console/mcp/bundles/export-defaults", &cookie).await;

    assert_eq!(
        payload["data"]["minimum_host_version"],
        json!(env!("CARGO_PKG_VERSION"))
    );
    assert_eq!(
        payload["data"]["current_system_version"],
        json!(env!("CARGO_PKG_VERSION"))
    );
}

fn sha256(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn bundle_zip(interface_id: &str, source_version: &str, duplicate_binding: bool) -> Vec<u8> {
    let tool = serde_json::to_vec_pretty(&json!({
        "tool_id": "bundle_runtime_profile",
        "name": "Runtime profile",
        "short_description": "Read runtime profile",
        "full_description": "Read runtime profile from the target host.",
        "interface_id": interface_id,
        "parameter_schema_snapshot": {"type": "object", "properties": {"stale": {"type": "string"}}},
        "result_schema_snapshot": {"type": "object"},
        "input_mapping": {},
        "output_mapping": {},
        "permission_code_snapshot": "stale.permission",
        "risk_level_snapshot": "critical",
        "status": "enabled"
    }))
    .unwrap();
    let mut instance_document = json!({
        "instance_id": "bundle_system",
        "name": "Bundle System",
        "description_short": "Imported MCP bundle",
        "status": "enabled",
        "default_entry_path": "/",
        "groups": [{
            "path": "/system",
            "display_name": "System",
            "description_short": "System tools",
            "enabled": true,
            "sort_order": 1
        }],
        "bindings": [{
            "group_path": "/system",
            "tool_id": "bundle_runtime_profile",
            "display_alias": null,
            "visible": true,
            "sort_order": 1
        }],
        "discovery_policy": {
            "list_default_limit": 20,
            "list_max_depth": 4,
            "list_regex_enabled": false,
            "list_regex_max_length": 120,
            "list_return_fields": ["id", "name", "path"]
        }
    });
    if duplicate_binding {
        let binding = instance_document["bindings"][0].clone();
        instance_document["bindings"]
            .as_array_mut()
            .unwrap()
            .push(binding);
    }
    let instance = serde_json::to_vec_pretty(&instance_document).unwrap();
    let manifest = serde_json::to_vec_pretty(&json!({
        "schema_version": "1flowbase.mcp.bundle/v1",
        "organization": "taichuy",
        "bundle_id": "route_test_bundle",
        "bundle_version": "1.0.0",
        "locale": "zh_Hans",
        "minimum_host_version": "0.2.0",
        "exported_from_system_version": source_version,
        "exported_at": "2026-07-13T10:00:00Z",
        "files": [
            {"path": "tools/runtime_profile.json", "kind": "tool", "sha256": sha256(&tool)},
            {"path": "instances/system.json", "kind": "instance", "sha256": sha256(&instance)}
        ]
    }))
    .unwrap();

    let mut archive = ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    for (path, bytes) in [
        ("manifest.json", manifest.as_slice()),
        ("tools/runtime_profile.json", tool.as_slice()),
        ("instances/system.json", instance.as_slice()),
    ] {
        archive.start_file(path, options).unwrap();
        archive.write_all(bytes).unwrap();
    }
    archive.finish().unwrap().into_inner()
}

fn multipart_body(file_name: &str, bytes: &[u8]) -> (String, Vec<u8>) {
    let boundary = "----1flowbase-mcp-bundle-test";
    let mut body = Vec::new();
    write!(
        body,
        "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{file_name}\"\r\nContent-Type: application/zip\r\n\r\n"
    )
    .unwrap();
    body.extend_from_slice(bytes);
    write!(body, "\r\n--{boundary}--\r\n").unwrap();
    (boundary.to_string(), body)
}

async fn app_with_installed_mcp_extension(
    bundle: Vec<u8>,
    replacement_local_bytes: Option<Vec<u8>>,
) -> (axum::Router, Uuid) {
    let (mut state, _) = test_api_state_with_database_url().await;
    let actor = AuthRepository::find_user_for_password_login(
        &state.store,
        domain::PASSWORD_LOCAL_AUTHENTICATOR_ID,
        "root",
    )
    .await
    .unwrap()
    .unwrap();
    let outcome =
        ExtensionInstallationService::new(state.store.clone(), &state.provider_install_root)
            .install_from_bytes(InstallExtensionArtifactCommand {
                actor_user_id: actor.id,
                category: ExtensionCatalogCategory::Mcp,
                organization: "taichuy".to_string(),
                artifact_id: "route_test_bundle".to_string(),
                version: "1.0.0".to_string(),
                node_id: state.api_node_id.clone(),
                expected_checksum: Some(sha256(&bundle)),
                artifact_bytes: bundle,
                source: "upload".to_string(),
                trust: "trusted".to_string(),
                signature_status: domain::ExtensionSignatureStatus::Verified,
                signature_algorithm: Some("ed25519".to_string()),
                signing_key_id: Some("fixture-key".to_string()),
                declared_warnings: Vec::new(),
                risk_override: None,
                confirmation_receipt: None,
                application_action: domain::ExtensionApplicationAction::ImportMcp,
            })
            .await
            .unwrap();
    let ExtensionArtifactInstallOutcome::Installed { installation, .. } = outcome else {
        panic!("verified MCP fixture must install without confirmation");
    };
    if let Some(bytes) = replacement_local_bytes {
        tokio::fs::write(&installation.local_path, bytes)
            .await
            .unwrap();
    }
    Arc::get_mut(&mut state).unwrap().official_mcp_bundle_source =
        Arc::new(FailingLegacyMcpCatalogSource);
    (
        crate::app_with_state_and_config(state, &test_config()),
        installation.id,
    )
}

async fn post_bundle(
    app: &axum::Router,
    path: &str,
    cookie: &str,
    csrf: &str,
    bundle: &[u8],
) -> axum::response::Response {
    let (boundary, body) = multipart_body("route-test.mcp.zip", bundle);
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(path)
                .header("cookie", cookie)
                .header("x-csrf-token", csrf)
                .header(
                    "content-type",
                    format!("multipart/form-data; boundary={boundary}"),
                )
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap()
}

async fn post_json(
    app: &axum::Router,
    path: &str,
    cookie: &str,
    csrf: &str,
    body: Value,
) -> axum::response::Response {
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(path)
                .header("cookie", cookie)
                .header("x-csrf-token", csrf)
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap()
}

#[tokio::test]
async fn mcp_bundle_preview_reports_older_source_and_missing_interface_without_writing() {
    // AC-005, AC-007, AC-009: preview is non-mutating and exposes version/interface risks.
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let bundle = bundle_zip("removed_interface", "0.1.0", false);

    let response = post_bundle(
        &app,
        "/api/console/mcp/bundles/preview-upload",
        &cookie,
        &csrf,
        &bundle,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let payload = response_json(response).await;
    assert_eq!(
        payload["data"]["version_status"],
        json!("exported_from_older_system")
    );
    assert_eq!(
        payload["data"]["current_system_version"],
        json!(env!("CARGO_PKG_VERSION"))
    );
    assert_eq!(payload["data"]["tools"][0]["result"], json!("unavailable"));
    assert_eq!(
        payload["data"]["tools"][0]["reason"],
        json!("interface_missing")
    );

    let catalog_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/console/mcp/catalog")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let catalog = response_json(catalog_response).await;
    assert!(catalog["data"]["tools"].as_array().unwrap().is_empty());
    assert!(catalog["data"]["instances"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn mcp_bundle_import_keeps_missing_interface_disabled_and_continues_instance_binding() {
    // AC-008 through AC-011: Tool-first import is partial-success and idempotent by stable ids.
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let bundle = bundle_zip("removed_interface", "0.2.6", false);

    let response = post_bundle(
        &app,
        "/api/console/mcp/bundles/import-upload",
        &cookie,
        &csrf,
        &bundle,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let payload = response_json(response).await;
    assert_eq!(payload["data"]["status"], json!("completed_with_warnings"));
    assert_eq!(payload["data"]["tools"][0]["result"], json!("unavailable"));
    assert_eq!(payload["data"]["instances"][0]["result"], json!("imported"));

    let catalog_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/console/mcp/catalog")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let catalog = response_json(catalog_response).await;
    let tool = &catalog["data"]["tools"][0];
    assert_eq!(tool["tool_id"], json!("bundle_runtime_profile"));
    assert_eq!(tool["status"], json!("disabled"));
    assert_eq!(tool["availability_status"], json!("interface_missing"));
    assert_eq!(catalog["data"]["instances"][0]["status"], json!("enabled"));
    assert_eq!(
        catalog["data"]["bindings"][0]["tool_id"],
        json!("bundle_runtime_profile")
    );

    let second = post_bundle(
        &app,
        "/api/console/mcp/bundles/import-upload",
        &cookie,
        &csrf,
        &bundle,
    )
    .await;
    assert_eq!(second.status(), StatusCode::OK);
    let second_payload = response_json(second).await;
    assert_eq!(
        second_payload["data"]["tools"][0]["result"],
        json!("skipped")
    );
    assert_eq!(
        second_payload["data"]["instances"][0]["result"],
        json!("skipped")
    );
}

#[tokio::test]
async fn mcp_bundle_import_rolls_back_an_instance_when_assembly_fails() {
    // AC-013: duplicate bindings fail inside the repository transaction without a partial instance.
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let bundle = bundle_zip("removed_interface", "0.2.6", true);

    let response = post_bundle(
        &app,
        "/api/console/mcp/bundles/import-upload",
        &cookie,
        &csrf,
        &bundle,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let payload = response_json(response).await;
    assert_eq!(payload["data"]["instances"][0]["result"], json!("failed"));
    assert_eq!(
        payload["data"]["instances"][0]["reason"],
        json!("instance_write_failed")
    );

    let catalog_response = app
        .oneshot(
            Request::builder()
                .uri("/api/console/mcp/catalog")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let catalog = response_json(catalog_response).await;
    assert!(catalog["data"]["instances"].as_array().unwrap().is_empty());
    assert!(catalog["data"]["groups"].as_array().unwrap().is_empty());
    assert!(catalog["data"]["bindings"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn delivery_1560_d5_ac_004_installed_mcp_artifact_previews_without_workspace_import() {
    let (app, extension_installation_id) =
        app_with_installed_mcp_extension(bundle_zip("removed_interface", "0.2.6", false), None)
            .await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;

    let preview = post_json(
        &app,
        "/api/console/mcp/bundles/preview-official",
        &cookie,
        &csrf,
        json!({"extension_installation_id": extension_installation_id}),
    )
    .await;
    assert_eq!(preview.status(), StatusCode::OK);
    let payload = response_json(preview).await;
    assert_eq!(payload["data"]["artifact_installation_status"], "installed");
    assert_eq!(
        payload["data"]["workspace_application_status"],
        "ready_to_import"
    );
    assert_eq!(
        payload["data"]["preview"]["manifest"]["bundle_id"],
        "route_test_bundle"
    );

    let catalog = get_json(&app, "/api/console/mcp/catalog", &cookie).await;
    assert!(catalog["data"]["tools"].as_array().unwrap().is_empty());
    let installed = get_json(
        &app,
        "/api/console/settings/extension-center/installed?category=mcp",
        &cookie,
    )
    .await;
    assert_eq!(
        installed["data"]["entries"][0]["id"],
        extension_installation_id.to_string()
    );
    assert_eq!(installed["data"]["entries"][0]["status"], "installed");
    assert_eq!(
        installed["data"]["entries"][0]["application_status"],
        "not_applied"
    );
}

#[tokio::test]
async fn delivery_1560_d5_ac_008_conflict_requires_confirmation_and_confirmed_retry_is_explainable()
{
    let (app, extension_installation_id) =
        app_with_installed_mcp_extension(bundle_zip("removed_interface", "0.2.6", false), None)
            .await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let confirmed_body = json!({
        "extension_installation_id": extension_installation_id,
        "conflict_resolution": "keep_existing"
    });
    let first = post_json(
        &app,
        "/api/console/mcp/bundles/import-official",
        &cookie,
        &csrf,
        confirmed_body.clone(),
    )
    .await;
    assert_eq!(first.status(), StatusCode::OK);
    let first_payload = response_json(first).await;
    assert_eq!(
        first_payload["data"]["workspace_application_status"],
        "imported"
    );
    let installed = get_json(
        &app,
        "/api/console/settings/extension-center/installed?category=mcp",
        &cookie,
    )
    .await;
    assert_eq!(
        installed["data"]["entries"][0]["application_status"],
        "applied"
    );

    let preview = post_json(
        &app,
        "/api/console/mcp/bundles/preview-official",
        &cookie,
        &csrf,
        json!({"extension_installation_id": extension_installation_id}),
    )
    .await;
    let preview_payload = response_json(preview).await;
    assert_eq!(
        preview_payload["data"]["workspace_application_status"],
        "confirmation_required"
    );
    assert_eq!(
        preview_payload["data"]["required_conflict_resolution"],
        "keep_existing"
    );

    let unconfirmed = post_json(
        &app,
        "/api/console/mcp/bundles/import-official",
        &cookie,
        &csrf,
        json!({"extension_installation_id": extension_installation_id}),
    )
    .await;
    assert_eq!(unconfirmed.status(), StatusCode::CONFLICT);
    let unconfirmed_payload = response_json(unconfirmed).await;
    assert_eq!(
        unconfirmed_payload["code"],
        "mcp_bundle_conflict_confirmation_required"
    );
    assert_eq!(
        unconfirmed_payload["workspace_application_status"],
        "not_imported"
    );
    assert_eq!(
        unconfirmed_payload["preview"]["tools"][0]["reason"],
        "tool_id_conflict"
    );

    let retry = post_json(
        &app,
        "/api/console/mcp/bundles/import-official",
        &cookie,
        &csrf,
        confirmed_body,
    )
    .await;
    assert_eq!(retry.status(), StatusCode::OK);
    let retry_payload = response_json(retry).await;
    assert_eq!(
        retry_payload["data"]["import_report"]["tools"][0]["result"],
        "skipped"
    );
    let catalog = get_json(&app, "/api/console/mcp/catalog", &cookie).await;
    assert_eq!(
        catalog["data"]["tools"][0]["tool_id"],
        "bundle_runtime_profile"
    );
}

#[tokio::test]
async fn delivery_1560_d5_ac_008_installed_mcp_preview_keeps_mcp_settings_scope() {
    let (app, cookie) = test_app_with_runtime_profile_error(&["system_runtime.view.all"]).await;
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/mcp/bundles/preview-official")
                .header("cookie", cookie)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"extension_installation_id": Uuid::nil()}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn delivery_1560_d5_f01_integrity_warning_previews_and_requires_structured_override() {
    let original = bundle_zip("removed_interface", "0.2.5", false);
    let local = bundle_zip("removed_interface", "0.2.6", false);
    let (app, extension_installation_id) =
        app_with_installed_mcp_extension(original, Some(local)).await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let selector = json!({"extension_installation_id": extension_installation_id});

    let preview = post_json(
        &app,
        "/api/console/mcp/bundles/preview-official",
        &cookie,
        &csrf,
        selector.clone(),
    )
    .await;
    assert_eq!(preview.status(), StatusCode::OK);
    let preview_payload = response_json(preview).await;
    assert_eq!(
        preview_payload["data"]["integrity_warnings"][0]["code"],
        "checksum_mismatch"
    );
    assert_eq!(
        preview_payload["data"]["required_integrity_override"]["warnings"][0]["code"],
        "checksum_mismatch"
    );

    let unconfirmed = post_json(
        &app,
        "/api/console/mcp/bundles/import-official",
        &cookie,
        &csrf,
        selector.clone(),
    )
    .await;
    assert_eq!(unconfirmed.status(), StatusCode::CONFLICT);
    let unconfirmed_payload = response_json(unconfirmed).await;
    assert_eq!(
        unconfirmed_payload["code"],
        "mcp_bundle_integrity_confirmation_required"
    );
    let empty_catalog = get_json(&app, "/api/console/mcp/catalog", &cookie).await;
    assert!(empty_catalog["data"]["tools"]
        .as_array()
        .unwrap()
        .is_empty());

    let confirmed = post_json(
        &app,
        "/api/console/mcp/bundles/import-official",
        &cookie,
        &csrf,
        json!({
            "extension_installation_id": extension_installation_id,
            "integrity_override": {
                "reason": "operator_accepts_local_artifact",
                "acknowledged_warnings": ["checksum_mismatch"]
            }
        }),
    )
    .await;
    assert_eq!(confirmed.status(), StatusCode::OK);
    let catalog = get_json(&app, "/api/console/mcp/catalog", &cookie).await;
    assert_eq!(
        catalog["data"]["tools"][0]["tool_id"],
        "bundle_runtime_profile"
    );
}

#[tokio::test]
async fn mcp_bundle_export_is_portable_zip_and_records_backend_system_version() {
    // AC-003 and AC-004: export contains semantic ids and backend-owned version metadata only.
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;

    let create_tool = post_json(
        &app,
        "/api/console/mcp/tools",
        &cookie,
        &csrf,
        json!({
            "tool_id": "portable_runtime_profile",
            "des_id": null,
            "name": "Runtime profile",
            "short_description": "Runtime profile",
            "full_description": "Read runtime profile.",
            "execution_target": {"kind":"interface_wrapper","interface_id":"get_runtime_profile"},
            "parameter_schema": {},
            "result_schema": {},
            "input_mapping": {},
            "output_mapping": {},
            "permission_code": null,
            "risk_level": "critical",
            "status": "enabled"
        }),
    )
    .await;
    assert_eq!(create_tool.status(), StatusCode::CREATED);

    let create_instance = post_json(
        &app,
        "/api/console/mcp/instances",
        &cookie,
        &csrf,
        json!({
            "instance_id": "portable_system",
            "name": "Portable System",
            "description_short": null,
            "status": "enabled",
            "default_entry_path": "/"
        }),
    )
    .await;
    assert_eq!(create_instance.status(), StatusCode::CREATED);
    let create_binding = post_json(
        &app,
        "/api/console/mcp/instances/portable_system/tool-bindings",
        &cookie,
        &csrf,
        json!({
            "group_path": "/",
            "tool_id": "portable_runtime_profile",
            "display_alias": null,
            "visible": true,
            "sort_order": 1
        }),
    )
    .await;
    assert_eq!(create_binding.status(), StatusCode::CREATED);

    let export = post_json(
        &app,
        "/api/console/mcp/bundles/export",
        &cookie,
        &csrf,
        json!({
            "organization": "taichuy",
            "bundle_id": "portable_bundle",
            "bundle_version": "1.0.0",
            "locale": "zh_Hans",
            "minimum_host_version": "0.2.0"
        }),
    )
    .await;
    assert_eq!(export.status(), StatusCode::OK);
    assert_eq!(export.headers()["content-type"], "application/zip");
    let bytes = to_bytes(export.into_body(), usize::MAX).await.unwrap();
    let mut archive = ZipArchive::new(Cursor::new(bytes)).unwrap();
    let manifest: Value =
        serde_json::from_reader(archive.by_name("manifest.json").unwrap()).unwrap();
    assert_eq!(manifest["schema_version"], json!("1flowbase.mcp.bundle/v2"));
    assert_eq!(
        manifest["exported_from_system_version"],
        json!(env!("CARGO_PKG_VERSION"))
    );
    assert_eq!(manifest["bundle_version"], json!("1.0.0"));

    let tool_path = manifest["files"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["kind"] == json!("tool"))
        .unwrap()["path"]
        .as_str()
        .unwrap()
        .to_string();
    let tool: Value = serde_json::from_reader(archive.by_name(&tool_path).unwrap()).unwrap();
    assert_eq!(tool["tool_id"], json!("portable_runtime_profile"));
    assert_eq!(tool["execution_target"]["kind"], json!("interface_wrapper"));
    assert!(tool.get("interface_id").is_none());
    for internal_field in [
        "id",
        "workspace_id",
        "created_by",
        "updated_by",
        "created_at",
    ] {
        assert!(
            tool.get(internal_field).is_none(),
            "must omit {internal_field}"
        );
    }

    let instance_path = manifest["files"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["kind"] == json!("instance"))
        .unwrap()["path"]
        .as_str()
        .unwrap()
        .to_string();
    let instance: Value =
        serde_json::from_reader(archive.by_name(&instance_path).unwrap()).unwrap();
    assert_eq!(
        instance["bindings"][0]["tool_id"],
        json!("portable_runtime_profile")
    );
    assert!(instance["bindings"][0].get("tool_record_id").is_none());
}

#[tokio::test]
async fn mcp_instance_bundle_export_contains_only_the_selected_instance_and_its_tools() {
    // AC-003 through AC-005: instance export is scoped, portable, and leaves full export intact.
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;

    for tool_id in ["selected_runtime_profile", "unrelated_runtime_profile"] {
        let response = post_json(
            &app,
            "/api/console/mcp/tools",
            &cookie,
            &csrf,
            json!({
                "tool_id": tool_id,
                "des_id": null,
                "name": tool_id,
                "short_description": tool_id,
                "full_description": tool_id,
                "execution_target": {"kind":"interface_wrapper","interface_id":"get_runtime_profile"},
                "parameter_schema": {},
                "result_schema": {},
                "input_mapping": {},
                "output_mapping": {},
                "permission_code": null,
                "risk_level": "low",
                "status": "enabled"
            }),
        )
        .await;
        assert_eq!(response.status(), StatusCode::CREATED);
    }

    for (instance_id, tool_id) in [
        ("selected_instance", "selected_runtime_profile"),
        ("unrelated_instance", "unrelated_runtime_profile"),
    ] {
        let instance = post_json(
            &app,
            "/api/console/mcp/instances",
            &cookie,
            &csrf,
            json!({
                "instance_id": instance_id,
                "name": instance_id,
                "description_short": null,
                "status": "enabled",
                "default_entry_path": "/"
            }),
        )
        .await;
        assert_eq!(instance.status(), StatusCode::CREATED);
        let binding = post_json(
            &app,
            &format!("/api/console/mcp/instances/{instance_id}/tool-bindings"),
            &cookie,
            &csrf,
            json!({
                "group_path": "/",
                "tool_id": tool_id,
                "display_alias": null,
                "visible": true,
                "sort_order": 1
            }),
        )
        .await;
        assert_eq!(binding.status(), StatusCode::CREATED);
    }

    let export = post_json(
        &app,
        "/api/console/mcp/instances/selected_instance/bundles/export",
        &cookie,
        &csrf,
        json!({
            "organization": "taichuy",
            "bundle_id": "selected_instance",
            "bundle_version": "1.0.0",
            "locale": "zh_Hans",
            "minimum_host_version": "0.2.0"
        }),
    )
    .await;
    assert_eq!(export.status(), StatusCode::OK);
    assert_eq!(export.headers()["content-type"], "application/zip");
    assert!(export.headers()["content-disposition"]
        .to_str()
        .unwrap()
        .contains("selected_instance"));
    let bytes = to_bytes(export.into_body(), usize::MAX).await.unwrap();
    let bundle_bytes = bytes.to_vec();
    let mut archive = ZipArchive::new(Cursor::new(bytes)).unwrap();
    let manifest: Value =
        serde_json::from_reader(archive.by_name("manifest.json").unwrap()).unwrap();
    let files = manifest["files"].as_array().unwrap();
    let instance_paths = files
        .iter()
        .filter(|entry| entry["kind"] == json!("instance"))
        .map(|entry| entry["path"].as_str().unwrap().to_string())
        .collect::<Vec<_>>();
    let tool_paths = files
        .iter()
        .filter(|entry| entry["kind"] == json!("tool"))
        .map(|entry| entry["path"].as_str().unwrap().to_string())
        .collect::<Vec<_>>();
    assert_eq!(instance_paths.len(), 1);
    assert_eq!(tool_paths.len(), 1);
    let instance: Value =
        serde_json::from_reader(archive.by_name(&instance_paths[0]).unwrap()).unwrap();
    let tool: Value = serde_json::from_reader(archive.by_name(&tool_paths[0]).unwrap()).unwrap();
    assert_eq!(instance["instance_id"], json!("selected_instance"));
    assert_eq!(tool["tool_id"], json!("selected_runtime_profile"));

    let preview = post_bundle(
        &app,
        "/api/console/mcp/bundles/preview-upload",
        &cookie,
        &csrf,
        &bundle_bytes,
    )
    .await;
    assert_eq!(preview.status(), StatusCode::OK);
}

#[tokio::test]
async fn mcp_bundle_official_catalog_and_preview_are_served_through_the_backend() {
    // AC-002 and AC-007: browser consumes the official source only through backend routes.
    let (mut state, _) = test_api_state_with_database_url().await;
    let bundle = bundle_zip("get_runtime_profile", "0.2.0", false);
    let state_mut = Arc::get_mut(&mut state).unwrap();
    state_mut.official_mcp_bundle_source = Arc::new(FailingLegacyMcpCatalogSource);
    state_mut.official_extension_catalog_source =
        Arc::new(FixtureOfficialMcpExtensionCatalogSource {
            bundle: Arc::new(bundle),
        });
    let app = crate::app_with_state_and_config(state, &test_config());
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;

    let catalog_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/console/mcp/bundles/official")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(catalog_response.status(), StatusCode::OK);
    let catalog = response_json(catalog_response).await;
    assert_eq!(
        catalog["data"]["source"]["source_kind"],
        "official_repository"
    );
    assert_eq!(
        catalog["data"]["entries"][0]["organization"],
        json!("taichuy")
    );
    assert_eq!(
        catalog["data"]["entries"][0]["bundle_id"],
        json!("test_bundle")
    );

    let preview = post_json(
        &app,
        "/api/console/mcp/bundles/preview-official",
        &cookie,
        &csrf,
        json!({"organization": "taichuy", "bundle_id": "test_bundle"}),
    )
    .await;
    assert_eq!(preview.status(), StatusCode::OK);
    let preview_payload = response_json(preview).await;
    assert_eq!(
        preview_payload["data"]["version_status"],
        json!("exported_from_older_system")
    );
    assert_eq!(
        preview_payload["data"]["tools"][0]["id"],
        json!("bundle_runtime_profile")
    );
}
