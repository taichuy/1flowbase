use std::io::{Cursor, Write};

use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tower::ServiceExt;
use zip::{CompressionMethod, ZipArchive, ZipWriter, write::SimpleFileOptions};

use crate::_tests::support::{login_and_capture_cookie, test_app};

async fn response_json(response: axum::response::Response) -> Value {
    serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap()
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
    assert_eq!(payload["data"]["current_system_version"], json!("0.2.6"));
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
    assert_eq!(manifest["exported_from_system_version"], json!("0.2.6"));
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
async fn mcp_bundle_official_catalog_and_preview_are_served_through_the_backend() {
    // AC-002 and AC-007: browser consumes the official source only through backend routes.
    let app = test_app().await;
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
        json!("official_runtime_profile")
    );
}
