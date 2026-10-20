use super::*;

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

    let rejected_client_version = post_json(
        &app,
        "/api/console/mcp/bundles/export",
        &cookie,
        &csrf,
        json!({
            "organization": "taichuy",
            "bundle_id": "portable_bundle",
            "bundle_version": "1.0.0",
            "locale": "zh_Hans",
            "minimum_host_version": "0.1.0"
        }),
    )
    .await;
    assert_eq!(
        rejected_client_version.status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );

    let export = post_json(
        &app,
        "/api/console/mcp/bundles/export",
        &cookie,
        &csrf,
        json!({
            "organization": "taichuy",
            "bundle_id": "portable_bundle",
            "bundle_version": "1.0.0",
            "locale": "zh_Hans"
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
    assert_eq!(
        manifest["minimum_host_version"],
        json!(env!("CARGO_PKG_VERSION"))
    );
    assert_eq!(manifest["bundle_version"], json!("1.0.0"));
    let manifest_files = manifest["files"].as_array().unwrap();
    let kind_rank = |kind: &str| match kind {
        "tool" => 0,
        "instance" => 1,
        "connection" => 2,
        _ => panic!("unexpected MCP bundle file kind"),
    };
    let manifest_order = manifest_files
        .iter()
        .map(|entry| {
            (
                kind_rank(entry["kind"].as_str().unwrap()),
                entry["path"].as_str().unwrap().to_string(),
            )
        })
        .collect::<Vec<_>>();
    let mut expected_order = manifest_order.clone();
    expected_order.sort();
    assert_eq!(manifest_order, expected_order);
    assert_eq!(
        manifest_files
            .iter()
            .map(|entry| entry["kind"].as_str().unwrap())
            .collect::<Vec<_>>(),
        vec!["tool", "instance"],
        "manifest files must group tools, instances, then connections"
    );

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
            "export_profile": "official_builtin"
        }),
    )
    .await;
    assert_eq!(export.status(), StatusCode::OK);
    assert_eq!(export.headers()["content-type"], "application/zip");
    assert!(export.headers()["content-disposition"]
        .to_str()
        .unwrap()
        .contains("selected_instance"));
    assert_eq!(export.headers()["x-1flowbase-mcp-excluded-tool-count"], "0");
    assert_eq!(export.headers()["x-1flowbase-mcp-exclusion-reasons"], "");
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

    let portable_export = post_json(
        &app,
        "/api/console/mcp/instances/selected_instance/bundles/export",
        &cookie,
        &csrf,
        json!({
            "organization": "taichuy",
            "bundle_id": "selected_instance",
            "bundle_version": "1.0.1",
            "locale": "zh_Hans"
        }),
    )
    .await;
    assert_eq!(portable_export.status(), StatusCode::OK);
    assert!(portable_export
        .headers()
        .get("x-1flowbase-mcp-excluded-tool-count")
        .is_none());
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
