use super::*;

#[tokio::test]
async fn custom_block_read_permissions_allow_page_use_without_design_or_writes() {
    let app = test_app().await;
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let workspace_id = current_workspace_id(&app, &root_cookie).await;
    let (page_id, tab_id) = create_block_page(&app, &root_cookie, &root_csrf, &workspace_id).await;
    let (created, block) = create_block(
        &app,
        &root_cookie,
        &root_csrf,
        &workspace_id,
        &page_id,
        Some(&tab_id),
        "Readable root",
        None,
        "export default () => null;",
        None,
    )
    .await;
    assert_eq!(created, StatusCode::CREATED, "{block}");
    let block_id = block["data"]["block_id"].as_str().unwrap();
    let (_, child) = create_block(
        &app,
        &root_cookie,
        &root_csrf,
        &workspace_id,
        &page_id,
        Some(&tab_id),
        "Readable child",
        Some(block_id),
        "export default () => null;",
        None,
    )
    .await;
    let child_id = child["data"]["block_id"].as_str().unwrap();
    let member = create_member(
        &app,
        &root_cookie,
        &root_csrf,
        "block-read-only",
        "temp-pass",
    )
    .await;
    create_role(&app, &root_cookie, &root_csrf, "block_read_only").await;
    replace_role_permissions(&app, &root_cookie, &root_csrf, "block_read_only", &[]).await;
    replace_member_roles(
        &app,
        &root_cookie,
        &root_csrf,
        &member,
        &["block_read_only"],
    )
    .await;
    let (status, body) = send_json(
        &app,
        "PUT",
        "/api/console/settings/roles/block_read_only/frontstage-routes",
        &root_cookie,
        &root_csrf,
        json!({"page_ids":[page_id],"tab_ids":[tab_id]}),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");

    let (status, catalog) = get_json(
        &app,
        "/api/console/settings/roles/console-policy-catalog",
        &root_cookie,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{catalog}");
    let group = catalog["data"]["groups"]
        .as_array()
        .unwrap()
        .iter()
        .find(|group| group["group_id"] == "system.ui-management")
        .unwrap();
    let operations = group["operations"].as_array().unwrap();
    let mut reads = Vec::new();
    for suffix in [
        "/blocks",
        "/blocks/search",
        "/blocks/:block_id",
        "/blocks/:block_id/children",
        "/blocks/:block_id/ancestors",
        "/blocks/:block_id/descendants",
        "/blocks/:block_id/code",
        "/blocks/:block_id/code/search",
        "/blocks/:block_id/code/fragment",
        "/blocks/:block_id/open",
        "/blocks/:block_id/runtime-assembly",
    ] {
        let path = format!("/api/console/frontstage/pages/:page_id{suffix}");
        let operation = operations
            .iter()
            .find(|operation| {
                operation["route"]["method"] == "GET" && operation["route"]["path"] == path
            })
            .unwrap_or_else(|| panic!("UI management custom policy is missing GET {path}"));
        assert_eq!(operation["authorization"]["kind"], "simple");
        reads
            .push(json!({"kind":"simple","operation_id":operation["operation_id"],"enabled":true}));
    }
    assert_eq!(
        reads.len(),
        reads
            .iter()
            .map(|read| read["operation_id"].as_str().unwrap())
            .collect::<BTreeSet<_>>()
            .len(),
        "each HTTP interface has its own toggle"
    );
    super::super::ui_management::set_ui_policy(
        &app,
        &root_cookie,
        &root_csrf,
        "block_read_only",
        true,
        "custom",
        json!(reads),
    )
    .await;
    let (viewer_cookie, viewer_csrf) =
        login_and_capture_cookie(&app, "block-read-only", "temp-pass").await;
    let base = format!("/api/console/frontstage/pages/{page_id}/blocks");
    let (status, roots) = get_json(&app, &format!("{base}?tab_id={tab_id}"), &viewer_cookie).await;
    assert_eq!(status, StatusCode::OK, "{roots}");
    assert_eq!(roots["data"][0]["block_id"], block_id);
    let (status, children) =
        get_json(&app, &format!("{base}/{block_id}/children"), &viewer_cookie).await;
    assert_eq!(status, StatusCode::OK, "{children}");
    assert_eq!(children["data"][0]["block_id"], child_id);
    for suffix in [
        format!("/search?tab_id={tab_id}&query=Readable"),
        format!("/{block_id}"),
        format!("/{child_id}/ancestors"),
        format!("/{block_id}/descendants"),
        format!("/{block_id}/code"),
        format!("/{block_id}/code/fragment"),
        format!("/{block_id}/code/search?query=export"),
        format!("/{block_id}/open"),
        format!("/{block_id}/runtime-assembly"),
    ] {
        let (status, payload) = get_json(&app, &format!("{base}{suffix}"), &viewer_cookie).await;
        assert_eq!(status, StatusCode::OK, "{suffix}: {payload}");
    }
    let (status, me) = get_json(&app, "/api/console/me", &viewer_cookie).await;
    assert_eq!(status, StatusCode::OK, "{me}");
    assert!(!me["data"]["permissions"]
        .as_array()
        .unwrap()
        .iter()
        .any(|permission| permission == "frontstage.page.design"));

    // A stored block executes with the caller's identity; target APIs still own authorization.
    let dispatch = format!(
        "/api/console/frontstage/pages/{page_id}/tabs/{tab_id}/callable-interfaces/dispatch"
    );
    let (status, own) = send_json(
        &app,
        "POST",
        &dispatch,
        &viewer_cookie,
        &viewer_csrf,
        json!({"block_id":block_id,"method":"GET","path":"/api/console/me","request":{}}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{own}");
    let (status, denied) = send_json(&app, "POST", &dispatch, &viewer_cookie, &viewer_csrf,
        json!({"block_id":block_id,"method":"GET","path":"/api/console/settings/roles","request":{}})).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{denied}");

    for (method, suffix, payload) in [
        ("PATCH", format!("/{block_id}"), json!({"title":"Denied"})),
        ("DELETE", format!("/{block_id}"), json!({})),
        (
            "PUT",
            format!("/{block_id}/code"),
            ready_executable_payload("export default 2;"),
        ),
        (
            "PATCH",
            format!("/{block_id}/code"),
            json!({"expected_source_revision":"a".repeat(64),"edits":[]}),
        ),
    ] {
        let (status, body) = send_json(
            &app,
            method,
            &format!("{base}{suffix}"),
            &viewer_cookie,
            &viewer_csrf,
            payload,
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{method} {suffix}: {body}");
    }

    // Revoking one read must not revoke the other reads or be bypassed by page visibility.
    let roots_id = reads[0]["operation_id"].clone();
    reads[0]["enabled"] = json!(false);
    super::super::ui_management::set_ui_policy(
        &app,
        &root_cookie,
        &root_csrf,
        "block_read_only",
        true,
        "custom",
        json!(reads),
    )
    .await;
    let (status, denied) = get_json(&app, &format!("{base}?tab_id={tab_id}"), &viewer_cookie).await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "revoked {roots_id}: {denied}"
    );
    let (status, code) = get_json(&app, &format!("{base}/{block_id}/code"), &viewer_cookie).await;
    assert_eq!(status, StatusCode::OK, "{code}");

    // Operation authority never replaces the page/tab allowlist, even with a cached source ETag.
    let (status, hidden) = send_json(
        &app,
        "PUT",
        "/api/console/settings/roles/block_read_only/frontstage-routes",
        &root_cookie,
        &root_csrf,
        json!({"page_ids":[],"tab_ids":[]}),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{hidden}");
    let (status, hidden) = get_json(&app, &format!("{base}/{block_id}/code"), &viewer_cookie).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{hidden}");
}
