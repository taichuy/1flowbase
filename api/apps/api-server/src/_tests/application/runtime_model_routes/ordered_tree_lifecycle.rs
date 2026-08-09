use super::*;

async fn tree_request(
    app: &axum::Router,
    cookie: &str,
    csrf: Option<&str>,
    method: &str,
    uri: String,
    body: Option<serde_json::Value>,
) -> (StatusCode, serde_json::Value) {
    let mut request = Request::builder()
        .method(method)
        .uri(uri)
        .header("cookie", cookie);
    if let Some(csrf) = csrf {
        request = request
            .header("x-csrf-token", csrf)
            .header("content-type", "application/json");
    }
    let response = app
        .clone()
        .oneshot(
            request
                .body(body.map_or_else(Body::empty, |body| Body::from(body.to_string())))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let payload =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
    (status, payload)
}

// AC-008/AC-009/AC-011/AC-012: one route-level lifecycle fixture covers status and error shapes.
#[tokio::test]
async fn ordered_tree_runtime_routes_create_move_query_and_delete_with_typed_failures() {
    let (app, _database_url) = test_app_with_database_url().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let model_id = create_ordered_tree_model(&app, &cookie, &csrf, "route_tree").await;
    create_text_field(&app, &cookie, &csrf, &model_id, "title").await;

    let (rank_status, rank_error) = tree_request(
        &app,
        &cookie,
        Some(&csrf),
        "POST",
        "/api/runtime/models/route_tree/create".to_owned(),
        Some(json!({ "title": "invalid", "sibling_rank": "client-rank" })),
    )
    .await;
    assert_eq!(rank_status, StatusCode::BAD_REQUEST);
    assert_eq!(rank_error["code"], "runtime_operation_field");

    let (root_status, root) = tree_request(
        &app,
        &cookie,
        Some(&csrf),
        "POST",
        "/api/runtime/models/route_tree/create".to_owned(),
        Some(json!({ "title": "root" })),
    )
    .await;
    assert_eq!(root_status, StatusCode::CREATED);
    let root_id = root["data"]["id"].as_str().unwrap();

    let (child_status, child) = tree_request(
        &app,
        &cookie,
        Some(&csrf),
        "POST",
        "/api/runtime/models/route_tree/create".to_owned(),
        Some(json!({ "title": "child", "parent_id": root_id })),
    )
    .await;
    assert_eq!(child_status, StatusCode::CREATED);
    let child_id = child["data"]["id"].as_str().unwrap();

    let (grandchild_status, grandchild) = tree_request(
        &app,
        &cookie,
        Some(&csrf),
        "POST",
        "/api/runtime/models/route_tree/create".to_owned(),
        Some(json!({ "title": "grandchild", "parent_id": child_id })),
    )
    .await;
    assert_eq!(grandchild_status, StatusCode::CREATED);
    let grandchild_id = grandchild["data"]["id"].as_str().unwrap();

    let (descendants_status, descendants) = tree_request(
        &app,
        &cookie,
        None,
        "GET",
        format!("/api/runtime/models/route_tree/tree/descendants/{root_id}?max_depth=1&limit=10"),
        None,
    )
    .await;
    assert_eq!(descendants_status, StatusCode::OK);
    assert_eq!(descendants["data"].as_array().unwrap().len(), 1);
    assert_eq!(descendants["data"][0]["depth"], 1);
    assert_eq!(descendants["data"][0]["path"], serde_json::Value::Null);

    let (leaf_status, leaf_error) = tree_request(
        &app,
        &cookie,
        Some(&csrf),
        "DELETE",
        format!("/api/runtime/models/route_tree/delete/{child_id}"),
        None,
    )
    .await;
    assert_eq!(leaf_status, StatusCode::CONFLICT);
    assert_eq!(leaf_error["code"], "tree_node_has_children");

    let (move_status, moved) = tree_request(
        &app,
        &cookie,
        Some(&csrf),
        "POST",
        format!("/api/runtime/models/route_tree/tree/move/{grandchild_id}"),
        Some(json!({ "new_parent_id": root_id, "before_id": child_id })),
    )
    .await;
    assert_eq!(move_status, StatusCode::OK);
    assert_eq!(moved["data"]["moved"], true);

    let (stale_status, stale_error) = tree_request(
        &app,
        &cookie,
        Some(&csrf),
        "POST",
        format!("/api/runtime/models/route_tree/tree/delete-subtree/{root_id}"),
        Some(json!({ "expected_affected_count": 2 })),
    )
    .await;
    assert_eq!(stale_status, StatusCode::CONFLICT);
    assert_eq!(stale_error["code"], "tree_subtree_changed");

    let (delete_status, deleted) = tree_request(
        &app,
        &cookie,
        Some(&csrf),
        "POST",
        format!("/api/runtime/models/route_tree/tree/delete-subtree/{root_id}"),
        Some(json!({ "expected_affected_count": 3 })),
    )
    .await;
    assert_eq!(delete_status, StatusCode::OK);
    assert_eq!(deleted["data"]["deleted_count"], 3);
}
