use super::*;

#[tokio::test]
async fn create_and_group_metadata_reject_mismatch_while_move_inherits_sidebar_placement() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let _workspace_id = current_workspace_id(&app, &cookie).await;
    let (group_status, group_payload) = send_json(
        &app,
        "POST",
        "/api/console/frontstage/pages/groups",
        &cookie,
        &csrf,
        json!({"title": "Sidebar", "rank": "a", "placement": "sidebar"}),
    )
    .await;
    assert_eq!(group_status, StatusCode::CREATED);
    let group_id = group_payload["data"]["id"].as_str().unwrap();

    let (create_status, create_payload) = send_json(
        &app,
        "POST",
        "/api/console/frontstage/pages",
        &cookie,
        &csrf,
        json!({
            "title": "Topbar child",
            "parent_id": group_id,
            "rank": "a",
            "placement": "topbar"
        }),
    )
    .await;
    assert_eq!(create_status, StatusCode::BAD_REQUEST);
    assert_eq!(create_payload["code"], "frontstage_page_placement_mismatch");

    let (page_status, page_payload) = send_json(
        &app,
        "POST",
        "/api/console/frontstage/pages",
        &cookie,
        &csrf,
        json!({
            "title": "Topbar root",
            "rank": "b",
            "placement": "topbar",
            "slug": "topbar-root"
        }),
    )
    .await;
    assert_eq!(page_status, StatusCode::CREATED);
    let page_id = page_payload["data"]["page"]["id"].as_str().unwrap();
    let tab_id = page_payload["data"]["default_tab"]["id"].as_str().unwrap();
    let document_root_uid = page_payload["data"]["default_tab"]["document_root_uid"]
        .as_str()
        .unwrap();

    let (move_status, move_payload) = send_json(
        &app,
        "POST",
        &format!("/api/console/frontstage/pages/{page_id}/move"),
        &cookie,
        &csrf,
        json!({"parent_id": group_id, "rank": "b"}),
    )
    .await;
    assert_eq!(move_status, StatusCode::OK);
    assert_eq!(move_payload["data"]["id"], json!(page_id));
    assert_eq!(move_payload["data"]["parent_id"], json!(group_id));
    assert_eq!(move_payload["data"]["placement"], "sidebar");
    assert_eq!(move_payload["data"]["rank"], "b");
    assert_eq!(move_payload["data"]["slug"], Value::Null);

    let (detail_status, detail_payload) = get_json(
        &app,
        &format!("/api/console/frontstage/pages/{page_id}/tabs/{tab_id}"),
        &cookie,
    )
    .await;
    assert_eq!(detail_status, StatusCode::OK);
    assert_eq!(detail_payload["data"]["page"]["id"], json!(page_id));
    assert_eq!(detail_payload["data"]["page"]["parent_id"], json!(group_id));
    assert_eq!(
        detail_payload["data"]["document"]["root_uid"],
        json!(document_root_uid)
    );

    let (valid_child_status, valid_child_payload) = send_json(
        &app,
        "POST",
        "/api/console/frontstage/pages",
        &cookie,
        &csrf,
        json!({
            "title": "Sidebar child",
            "parent_id": group_id,
            "rank": "c",
            "placement": "sidebar"
        }),
    )
    .await;
    assert_eq!(valid_child_status, StatusCode::CREATED);
    assert_eq!(
        valid_child_payload["data"]["page"]["parent_id"],
        json!(group_id)
    );
    let child_id = valid_child_payload["data"]["page"]["id"].as_str().unwrap();
    let (tree_status, tree_before) = get_json(&app, "/api/console/frontstage/pages", &cookie).await;
    assert_eq!(tree_status, StatusCode::OK);
    let roots = tree_before["data"].as_array().unwrap();
    assert_eq!(roots.len(), 1);
    assert_eq!(roots[0]["id"], json!(group_id));
    assert_eq!(roots[0]["placement"], "sidebar");
    let children = roots[0]["children"].as_array().unwrap();
    assert_eq!(children.len(), 2);
    assert_eq!(children[0]["id"], json!(page_id));
    assert_eq!(children[1]["id"], json!(child_id));
    for child in children {
        assert_eq!(child["placement"], "sidebar");
    }

    let (metadata_status, metadata_payload) = send_json(
        &app,
        "PATCH",
        &format!("/api/console/frontstage/pages/{group_id}"),
        &cookie,
        &csrf,
        json!({"placement": "topbar", "slug": "sidebar-group"}),
    )
    .await;
    assert_eq!(metadata_status, StatusCode::BAD_REQUEST);
    assert_eq!(
        metadata_payload["code"],
        "frontstage_group_placement_requires_empty_group"
    );
    let (tree_status, tree_after) = get_json(&app, "/api/console/frontstage/pages", &cookie).await;
    assert_eq!(tree_status, StatusCode::OK);
    assert_eq!(tree_after["data"], tree_before["data"]);
}

#[tokio::test]
async fn group_under_group_is_allowed_but_move_under_descendant_is_atomic() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let workspace_id = current_workspace_id(&app, &cookie).await;
    let (status, payload) =
        create_group(&app, &cookie, &csrf, &workspace_id, Some("Parent"), "a").await;
    assert_eq!(status, StatusCode::CREATED);
    let parent_id = payload["data"]["id"].as_str().unwrap();

    let (nested_status, nested_payload) = send_json(
        &app,
        "POST",
        "/api/console/frontstage/pages/groups",
        &cookie,
        &csrf,
        json!({
            "title": "Nested",
            "parent_id": parent_id,
            "rank": "b"
        }),
    )
    .await;

    assert_eq!(nested_status, StatusCode::CREATED);
    let nested_id = nested_payload["data"]["id"].as_str().unwrap();
    let (tree_status, tree_before) = get_json(&app, "/api/console/frontstage/pages", &cookie).await;
    assert_eq!(tree_status, StatusCode::OK);
    let (move_status, _) = send_json(
        &app,
        "POST",
        &format!("/api/console/frontstage/pages/{parent_id}/move"),
        &cookie,
        &csrf,
        json!({"parent_id": nested_id, "rank": "c"}),
    )
    .await;
    assert_eq!(move_status, StatusCode::BAD_REQUEST);
    let (tree_status, tree_after) = get_json(&app, "/api/console/frontstage/pages", &cookie).await;
    assert_eq!(tree_status, StatusCode::OK);
    assert_eq!(tree_after["data"], tree_before["data"]);
}

#[tokio::test]
async fn cross_workspace_parent_is_rejected() {
    let (app, database_url) = test_app_with_database_url().await;
    let other_workspace_id = seed_workspace(&database_url, "Other Workspace").await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let workspace_id = current_workspace_id(&app, &cookie).await;
    let other_csrf = switch_workspace(&app, &cookie, &csrf, &other_workspace_id.to_string()).await;
    let (other_group_status, other_group_payload) = create_group(
        &app,
        &cookie,
        &other_csrf,
        &other_workspace_id.to_string(),
        Some("Other"),
        "a",
    )
    .await;
    assert_eq!(other_group_status, StatusCode::CREATED);
    let other_group_id = other_group_payload["data"]["id"].as_str().unwrap();
    let csrf = switch_workspace(&app, &cookie, &other_csrf, &workspace_id).await;

    let (page_status, _) = create_page(
        &app,
        &cookie,
        &csrf,
        &workspace_id,
        Some("Bad Parent"),
        Some(other_group_id),
        "a",
    )
    .await;

    assert_eq!(page_status, StatusCode::BAD_REQUEST);

    let (page_status, page_payload) = create_page(
        &app,
        &cookie,
        &csrf,
        &workspace_id,
        Some("Local page"),
        None,
        "a",
    )
    .await;
    assert_eq!(page_status, StatusCode::CREATED);
    let page_id = page_payload["data"]["page"]["id"].as_str().unwrap();
    let (tree_status, tree_before) = get_json(&app, "/api/console/frontstage/pages", &cookie).await;
    assert_eq!(tree_status, StatusCode::OK);
    let (move_status, _) = send_json(
        &app,
        "POST",
        &format!("/api/console/frontstage/pages/{page_id}/move"),
        &cookie,
        &csrf,
        json!({"parent_id": other_group_id, "rank": "b"}),
    )
    .await;
    assert_eq!(move_status, StatusCode::BAD_REQUEST);
    let (tree_status, tree_after) = get_json(&app, "/api/console/frontstage/pages", &cookie).await;
    assert_eq!(tree_status, StatusCode::OK);
    assert_eq!(tree_after["data"], tree_before["data"]);
}

#[tokio::test]
async fn moving_page_keeps_get_tree_order_stable() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let workspace_id = current_workspace_id(&app, &cookie).await;
    let (_, group_payload) =
        create_group(&app, &cookie, &csrf, &workspace_id, Some("Group"), "z").await;
    let group_id = group_payload["data"]["id"].as_str().unwrap();
    let (_, first_payload) = create_page(
        &app,
        &cookie,
        &csrf,
        &workspace_id,
        Some("First"),
        None,
        "a",
    )
    .await;
    let first_page_id = first_payload["data"]["page"]["id"].as_str().unwrap();
    let (_, second_payload) = create_page(
        &app,
        &cookie,
        &csrf,
        &workspace_id,
        Some("Second"),
        None,
        "b",
    )
    .await;
    let second_page_id = second_payload["data"]["page"]["id"].as_str().unwrap();

    let (move_status, _) = send_json(
        &app,
        "POST",
        &format!("/api/console/frontstage/pages/{second_page_id}/move"),
        &cookie,
        &csrf,
        json!({
            "parent_id": group_id,
            "rank": "a"
        }),
    )
    .await;
    assert_eq!(move_status, StatusCode::OK);

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/console/frontstage/pages")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let payload: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();

    assert_eq!(payload["data"][0]["id"], json!(first_page_id));
    assert_eq!(payload["data"][1]["id"], json!(group_id));
    assert_eq!(
        payload["data"][1]["children"][0]["id"],
        json!(second_page_id)
    );
}

#[tokio::test]
async fn deleting_group_removes_child_page_from_tree() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let workspace_id = current_workspace_id(&app, &cookie).await;
    let (_, group_payload) =
        create_group(&app, &cookie, &csrf, &workspace_id, Some("Group"), "a").await;
    let group_id = group_payload["data"]["id"].as_str().unwrap();
    let (_, page_payload) = create_page(
        &app,
        &cookie,
        &csrf,
        &workspace_id,
        Some("Child"),
        Some(group_id),
        "a",
    )
    .await;
    let page_id = page_payload["data"]["page"]["id"].as_str().unwrap();

    let delete_status = delete_node(&app, &cookie, &csrf, &workspace_id, group_id).await;
    assert_eq!(delete_status, StatusCode::NO_CONTENT);

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/console/frontstage/pages")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let payload: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(payload["data"], json!([]));
    assert!(!payload.to_string().contains(page_id));
}
