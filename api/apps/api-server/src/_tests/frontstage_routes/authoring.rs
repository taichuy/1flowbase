use super::*;

#[tokio::test]
async fn root_can_create_group_and_page_and_catalog_schema_validates_tree() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let workspace_id = current_workspace_id(&app, &cookie).await;

    let (group_status, group_payload) =
        create_group(&app, &cookie, &csrf, &workspace_id, Some("Landing"), "a").await;
    assert_eq!(group_status, StatusCode::CREATED);
    let group_id = group_payload["data"]["id"].as_str().unwrap();

    let (page_status, page_payload) = create_page(
        &app,
        &cookie,
        &csrf,
        &workspace_id,
        Some("Home"),
        Some(group_id),
        "a",
    )
    .await;
    assert_eq!(page_status, StatusCode::CREATED);
    assert_eq!(page_payload["data"]["kind"], json!("page"));

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/console/frontstage/{workspace_id}/pages"))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let payload: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(payload["data"][0]["id"], json!(group_id));
    assert_eq!(payload["data"][0]["children"][0]["title"], json!("Home"));

    let openapi = serde_json::to_value(crate::openapi::ApiDoc::openapi())
        .expect("global OpenAPI should serialize");
    let operation = crate::openapi_docs::DocsCatalogOperation {
        id: "list_frontstage_pages".into(),
        method: "GET".into(),
        path: "/api/console/frontstage/{workspace_id}/pages".into(),
        summary: None,
        description: None,
        tags: Vec::new(),
        group: "other".into(),
        deprecated: false,
    };
    let interface = crate::openapi_interface::catalog_entry_from_operation(&operation, &openapi)
        .expect("frontstage page tree interface catalog entry");
    let response_validator = jsonschema::validator_for(&interface.response_schema)
        .expect("frontstage page tree response schema should be self-contained");
    assert!(response_validator.validate(&payload["data"]).is_ok());
}

#[tokio::test]
async fn manager_can_create_group_and_page() {
    let app = test_app().await;
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    create_member(
        &app,
        &root_cookie,
        &root_csrf,
        "frontstage-manager",
        "temp-pass",
    )
    .await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "frontstage-manager", "temp-pass").await;
    let workspace_id = current_workspace_id(&app, &cookie).await;

    let (group_status, _) =
        create_group(&app, &cookie, &csrf, &workspace_id, Some("Group"), "a").await;
    assert_eq!(group_status, StatusCode::CREATED);

    let (page_status, _) =
        create_page(&app, &cookie, &csrf, &workspace_id, Some("Page"), None, "b").await;
    assert_eq!(page_status, StatusCode::CREATED);
}

#[tokio::test]
async fn workspace_member_without_design_permission_cannot_write() {
    let app = test_app().await;
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let member_id = create_member(
        &app,
        &root_cookie,
        &root_csrf,
        "frontstage-viewer",
        "temp-pass",
    )
    .await;
    create_role(&app, &root_cookie, &root_csrf, "frontstage_viewer").await;
    replace_role_permissions(&app, &root_cookie, &root_csrf, "frontstage_viewer", &[]).await;
    replace_member_roles(
        &app,
        &root_cookie,
        &root_csrf,
        &member_id,
        &["frontstage_viewer"],
    )
    .await;

    let root_workspace_id = current_workspace_id(&app, &root_cookie).await;
    let (_, page_payload) = create_page(
        &app,
        &root_cookie,
        &root_csrf,
        &root_workspace_id,
        Some("Protected"),
        None,
        "a",
    )
    .await;
    let page_id = page_payload["data"]["id"].as_str().unwrap();

    let (cookie, csrf) = login_and_capture_cookie(&app, "frontstage-viewer", "temp-pass").await;
    let workspace_id = current_workspace_id(&app, &cookie).await;
    let (status, _) = create_group(&app, &cookie, &csrf, &workspace_id, Some("Group"), "a").await;

    assert_eq!(status, StatusCode::FORBIDDEN);

    let (tab_status, _) = send_json(
        &app,
        "POST",
        &format!("/api/console/frontstage/{workspace_id}/pages/{page_id}/tabs"),
        &cookie,
        &csrf,
        json!({"title": "Denied"}),
    )
    .await;
    assert_eq!(tab_status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn rename_allows_empty_title() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let workspace_id = current_workspace_id(&app, &cookie).await;
    let (status, payload) = create_page(
        &app,
        &cookie,
        &csrf,
        &workspace_id,
        Some("Named"),
        None,
        "a",
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let page_id = payload["data"]["id"].as_str().unwrap();

    let (rename_status, rename_payload) = send_json(
        &app,
        "PATCH",
        &format!("/api/console/frontstage/{workspace_id}/pages/{page_id}"),
        &cookie,
        &csrf,
        json!({ "title": "" }),
    )
    .await;

    assert_eq!(rename_status, StatusCode::OK);
    assert_eq!(rename_payload["data"]["title"], json!(""));
}

#[tokio::test]
async fn patch_page_metadata_persists_tooltip_and_hidden_state() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let workspace_id = current_workspace_id(&app, &cookie).await;
    let (status, payload) = create_page(
        &app,
        &cookie,
        &csrf,
        &workspace_id,
        Some("Named"),
        None,
        "a",
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let page_id = payload["data"]["id"].as_str().unwrap();

    let (patch_status, patch_payload) = send_json(
        &app,
        "PATCH",
        &format!("/api/console/frontstage/{workspace_id}/pages/{page_id}"),
        &cookie,
        &csrf,
        json!({
            "icon": "FileTextOutlined",
            "tooltip": "展示在页面树",
            "is_hidden": true
        }),
    )
    .await;

    assert_eq!(patch_status, StatusCode::OK);
    assert_eq!(patch_payload["data"]["title"], json!("Named"));
    assert_eq!(patch_payload["data"]["icon"], json!("FileTextOutlined"));
    assert_eq!(patch_payload["data"]["tooltip"], json!("展示在页面树"));
    assert_eq!(patch_payload["data"]["is_hidden"], json!(true));

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/console/frontstage/{workspace_id}/pages"))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let payload: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(payload["data"][0]["title"], json!("Named"));
    assert_eq!(payload["data"][0]["icon"], json!("FileTextOutlined"));
    assert_eq!(payload["data"][0]["tooltip"], json!("展示在页面树"));
    assert_eq!(payload["data"][0]["is_hidden"], json!(true));
}
