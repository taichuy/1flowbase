use super::*;

#[tokio::test]
async fn placement_mismatch_is_rejected_by_create_move_and_group_metadata_routes() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let workspace_id = current_workspace_id(&app, &cookie).await;
    let (group_status, group_payload) = send_json(
        &app,
        "POST",
        &format!("/api/console/frontstage/{workspace_id}/pages/groups"),
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
        &format!("/api/console/frontstage/{workspace_id}/pages"),
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
        &format!("/api/console/frontstage/{workspace_id}/pages"),
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
    let page_id = page_payload["data"]["id"].as_str().unwrap();

    let (move_status, move_payload) = send_json(
        &app,
        "POST",
        &format!("/api/console/frontstage/{workspace_id}/pages/{page_id}/move"),
        &cookie,
        &csrf,
        json!({"parent_id": group_id, "rank": "b"}),
    )
    .await;
    assert_eq!(move_status, StatusCode::BAD_REQUEST);
    assert_eq!(move_payload["code"], "frontstage_page_placement_mismatch");

    let (valid_child_status, _) = send_json(
        &app,
        "POST",
        &format!("/api/console/frontstage/{workspace_id}/pages"),
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

    let (metadata_status, metadata_payload) = send_json(
        &app,
        "PATCH",
        &format!("/api/console/frontstage/{workspace_id}/pages/{group_id}"),
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
}

#[tokio::test]
async fn group_under_group_is_allowed() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let workspace_id = current_workspace_id(&app, &cookie).await;
    let (status, payload) =
        create_group(&app, &cookie, &csrf, &workspace_id, Some("Parent"), "a").await;
    assert_eq!(status, StatusCode::CREATED);
    let parent_id = payload["data"]["id"].as_str().unwrap();

    let (nested_status, _) = send_json(
        &app,
        "POST",
        &format!("/api/console/frontstage/{workspace_id}/pages/groups"),
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
}

#[tokio::test]
async fn cross_workspace_parent_is_rejected() {
    let (app, database_url) = test_app_with_database_url().await;
    let other_workspace_id = seed_workspace(&database_url, "Other Workspace").await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let workspace_id = current_workspace_id(&app, &cookie).await;
    let (other_group_status, other_group_payload) = create_group(
        &app,
        &cookie,
        &csrf,
        &other_workspace_id.to_string(),
        Some("Other"),
        "a",
    )
    .await;
    assert_eq!(other_group_status, StatusCode::CREATED);
    let other_group_id = other_group_payload["data"]["id"].as_str().unwrap();

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
    let first_page_id = first_payload["data"]["id"].as_str().unwrap();
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
    let second_page_id = second_payload["data"]["id"].as_str().unwrap();

    let (move_status, _) = send_json(
        &app,
        "POST",
        &format!("/api/console/frontstage/{workspace_id}/pages/{second_page_id}/move"),
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
    let page_id = page_payload["data"]["id"].as_str().unwrap();

    let delete_status = delete_node(&app, &cookie, &csrf, &workspace_id, group_id).await;
    assert_eq!(delete_status, StatusCode::NO_CONTENT);

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
    assert_eq!(payload["data"], json!([]));
    assert!(!payload.to_string().contains(page_id));
}
