use super::*;

#[tokio::test]
async fn mcp_bundle_import_upserts_duplicate_bindings_without_duplicate_rows() {
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
    assert_eq!(payload["data"]["status"], json!("completed_with_warnings"));

    let catalog = get_json(&app, "/api/console/mcp/catalog", &cookie).await;
    assert_eq!(catalog["data"]["tools"].as_array().unwrap().len(), 1);
    assert_eq!(catalog["data"]["instances"].as_array().unwrap().len(), 1);
    assert_eq!(catalog["data"]["groups"].as_array().unwrap().len(), 1);
    let bindings = catalog["data"]["bindings"].as_array().unwrap();
    assert_eq!(bindings.len(), 1);
    assert_eq!(bindings[0]["tool_id"], json!("bundle_runtime_profile"));
    assert_eq!(bindings[0]["group_path"], json!("/system"));
}

#[tokio::test]
async fn mcp_bundle_import_rolls_back_the_graph_when_a_binding_tool_is_missing() {
    // AC-013: a missing binding tool fails after tool/group inserts, rolling back the whole graph.
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let bundle = bundle_zip_with_binding_tool("removed_interface", "0.2.6", "missing_bundle_tool");

    let response = post_bundle(
        &app,
        "/api/console/mcp/bundles/import-upload",
        &cookie,
        &csrf,
        &bundle,
    )
    .await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);

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
    assert!(catalog["data"]["tools"].as_array().unwrap().is_empty());
    assert!(catalog["data"]["instances"].as_array().unwrap().is_empty());
    assert!(catalog["data"]["groups"].as_array().unwrap().is_empty());
    assert!(catalog["data"]["bindings"].as_array().unwrap().is_empty());
}
