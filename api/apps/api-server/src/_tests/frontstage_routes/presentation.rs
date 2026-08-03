use super::*;

#[tokio::test]
async fn page_presentation_and_tab_route_segments_are_persisted_and_resolved() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let workspace_id = current_workspace_id(&app, &cookie).await;
    let (page_status, page_payload) = create_page(
        &app,
        &cookie,
        &csrf,
        &workspace_id,
        Some("Presentation"),
        None,
        "a",
    )
    .await;
    assert_eq!(page_status, StatusCode::CREATED);
    let page_id = page_payload["data"]["id"].as_str().unwrap();
    let default_tab_id = page_payload["data"]["default_tab"]["id"].as_str().unwrap();
    assert_eq!(
        page_payload["data"]["content_presentation"],
        json!("single"),
        "AC-001: new pages default to the persisted single presentation"
    );
    assert_eq!(
        page_payload["data"]["default_tab"]["route_segment"],
        Value::Null,
        "the default Tab continues to use the Page URL"
    );

    let (presentation_status, presentation_payload) = send_json(
        &app,
        "PATCH",
        &format!("/api/console/frontstage/{workspace_id}/pages/{page_id}"),
        &cookie,
        &csrf,
        json!({ "content_presentation": "tabs" }),
    )
    .await;
    assert_eq!(presentation_status, StatusCode::OK);
    assert_eq!(
        presentation_payload["data"]["content_presentation"],
        json!("tabs"),
        "AC-001: the persisted Page configuration controls the Tab container"
    );

    let (create_tab_status, tab_payload) = send_json(
        &app,
        "POST",
        &format!("/api/console/frontstage/{workspace_id}/pages/{page_id}/tabs"),
        &cookie,
        &csrf,
        json!({
            "title": "Details",
            "route_segment": "details",
            "rank": "b"
        }),
    )
    .await;
    assert_eq!(create_tab_status, StatusCode::CREATED);
    let detail_tab_id = tab_payload["data"]["id"].as_str().unwrap();
    assert_eq!(tab_payload["data"]["route_segment"], json!("details"));

    let (resolved_status, resolved_payload) = get_json(
        &app,
        &format!("/api/console/frontstage/{workspace_id}/pages/{page_id}/tabs/details"),
        &cookie,
    )
    .await;
    assert_eq!(resolved_status, StatusCode::OK);
    assert_eq!(resolved_payload["data"]["tab"]["id"], json!(detail_tab_id));
    assert_eq!(
        resolved_payload["data"]["page"]["content_presentation"],
        json!("tabs")
    );

    let (legacy_status, legacy_payload) = get_json(
        &app,
        &format!("/api/console/frontstage/{workspace_id}/pages/{page_id}/tabs/{default_tab_id}"),
        &cookie,
    )
    .await;
    assert_eq!(legacy_status, StatusCode::OK);
    assert_eq!(legacy_payload["data"]["tab"]["is_default"], json!(true));
}

#[tokio::test]
async fn frontstage_read_apis_require_visibility_but_writes_keep_design_permission() {
    let app = test_app().await;
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let member_id = create_member(
        &app,
        &root_cookie,
        &root_csrf,
        "frontstage-code-viewer",
        "temp-pass",
    )
    .await;
    create_role(&app, &root_cookie, &root_csrf, "frontstage_code_viewer").await;
    replace_role_permissions(
        &app,
        &root_cookie,
        &root_csrf,
        "frontstage_code_viewer",
        &["frontstage.page.design"],
    )
    .await;
    replace_member_roles(
        &app,
        &root_cookie,
        &root_csrf,
        &member_id,
        &["frontstage_code_viewer"],
    )
    .await;

    let workspace_id = current_workspace_id(&app, &root_cookie).await;
    let (_, page_payload) = create_page(
        &app,
        &root_cookie,
        &root_csrf,
        &workspace_id,
        Some("Readable"),
        None,
        "a",
    )
    .await;
    let page_id = page_payload["data"]["id"].as_str().unwrap();
    let tab_id = page_payload["data"]["default_tab"]["id"].as_str().unwrap();
    let code_path =
        format!("/api/console/frontstage/{workspace_id}/pages/{page_id}/block-codes/hero");
    let (save_status, _) = send_json(
        &app,
        "PUT",
        &code_path,
        &root_cookie,
        &root_csrf,
        json!({ "code": "export default 1;" }),
    )
    .await;
    assert_eq!(save_status, StatusCode::OK);

    let (viewer_cookie, viewer_csrf) =
        login_and_capture_cookie(&app, "frontstage-code-viewer", "temp-pass").await;
    let (detail_status, _) = get_json(
        &app,
        &format!("/api/console/frontstage/{workspace_id}/pages/{page_id}/tabs/{tab_id}"),
        &viewer_cookie,
    )
    .await;
    assert_eq!(detail_status, StatusCode::NOT_FOUND);

    let (read_status, _) = get_json(&app, &code_path, &viewer_cookie).await;
    assert_eq!(read_status, StatusCode::NOT_FOUND);

    let (write_status, _) = send_json(
        &app,
        "PUT",
        &code_path,
        &viewer_cookie,
        &viewer_csrf,
        json!({ "code": "export default 2;" }),
    )
    .await;
    assert_eq!(write_status, StatusCode::OK);

    let (content_write_status, _) = save_page_content(
        &app,
        &viewer_cookie,
        &viewer_csrf,
        &workspace_id,
        page_id,
        tab_id,
        json!({ "version": 1, "blocks": [] }),
    )
    .await;
    assert_eq!(content_write_status, StatusCode::OK);
}

#[tokio::test]
async fn ac_010_capability_dispatch_rejects_unknown_ids_and_rechecks_action_permission() {
    let app = test_app().await;
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let workspace_id = current_workspace_id(&app, &root_cookie).await;
    let (_, page_payload) = create_page(
        &app,
        &root_cookie,
        &root_csrf,
        &workspace_id,
        Some("Capabilities"),
        None,
        "a",
    )
    .await;
    let page_id = page_payload["data"]["id"].as_str().unwrap();
    let tab_id = page_payload["data"]["default_tab"]["id"].as_str().unwrap();

    let (unknown_query_status, unknown_query_payload) = dispatch_capability(
        &app,
        &root_cookie,
        &root_csrf,
        &workspace_id,
        page_id,
        tab_id,
        "queries",
        "query_id",
        "frontstage.unknown.query",
        json!({ "model": "users" }),
    )
    .await;
    assert_eq!(unknown_query_status, StatusCode::NOT_FOUND);
    assert_eq!(unknown_query_payload["code"], "resource_action");

    let (unknown_action_status, unknown_action_payload) = dispatch_capability(
        &app,
        &root_cookie,
        &root_csrf,
        &workspace_id,
        page_id,
        tab_id,
        "actions",
        "action_id",
        "frontstage.unknown.action",
        json!({}),
    )
    .await;
    assert_eq!(unknown_action_status, StatusCode::NOT_FOUND);
    assert_eq!(unknown_action_payload["code"], "resource_action");

    let member_id = create_member(
        &app,
        &root_cookie,
        &root_csrf,
        "frontstage-capability-designer",
        "temp-pass",
    )
    .await;
    create_role(
        &app,
        &root_cookie,
        &root_csrf,
        "frontstage_capability_designer",
    )
    .await;
    replace_role_permissions(
        &app,
        &root_cookie,
        &root_csrf,
        "frontstage_capability_designer",
        &["frontstage.page.design"],
    )
    .await;
    replace_member_roles(
        &app,
        &root_cookie,
        &root_csrf,
        &member_id,
        &["frontstage_capability_designer"],
    )
    .await;
    let (member_cookie, member_csrf) =
        login_and_capture_cookie(&app, "frontstage-capability-designer", "temp-pass").await;
    let save_params = json!({ "payload": { "version": 1, "blocks": [] } });

    let (allowed_status, _) = dispatch_capability(
        &app,
        &member_cookie,
        &member_csrf,
        &workspace_id,
        page_id,
        tab_id,
        "actions",
        "action_id",
        "frontstage.page_tab.document.save",
        save_params.clone(),
    )
    .await;
    assert_eq!(allowed_status, StatusCode::OK);

    replace_role_permissions(
        &app,
        &root_cookie,
        &root_csrf,
        "frontstage_capability_designer",
        &[],
    )
    .await;
    let (revoked_status, _) = dispatch_capability(
        &app,
        &member_cookie,
        &member_csrf,
        &workspace_id,
        page_id,
        tab_id,
        "actions",
        "action_id",
        "frontstage.page_tab.document.save",
        save_params,
    )
    .await;
    assert_eq!(revoked_status, StatusCode::FORBIDDEN);
}
