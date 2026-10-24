use super::*;

// Exercise the real canonical runtime adapter and its shared cache through HTTP. Reading before every write is essential: cold reads hide this bug.
#[tokio::test]
async fn runtime_ordered_tree_writes_invalidate_warmed_record_and_list_caches() {
    let fixture = fixture().await;
    let app = &fixture.app;
    let (cookie, csrf) = login(app, "root", "change-me").await;
    let definitions = "/api/console/settings/data-models/model-definitions";
    let (status, created) = json_request(
        app,
        "POST",
        definitions,
        &cookie,
        &csrf,
        json!({
            "scope_kind":"workspace", "template_provider":"core", "template_code":"ordered_tree",
            "template_version":"v1", "code":"cache_tree", "title":"Cache Tree", "status":"draft"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    let model_id = created["data"]["id"].as_str().unwrap();
    let (status, field) = json_request(
        app,
        "POST",
        &format!("{definitions}/{model_id}/fields"),
        &cookie,
        &csrf,
        json!({"code":"title","title":"Title","field_kind":"string"}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{field}");
    let (status, published) = json_request(
        app,
        "PATCH",
        &format!("{definitions}/{model_id}"),
        &cookie,
        &csrf,
        json!({"status":"published"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{published}");
    let base = "/api/runtime/models/cache_tree";
    let (status, empty) = get(app, &format!("{base}/list"), &cookie).await;
    assert_eq!(status, StatusCode::OK, "{empty}");
    assert_eq!(empty["data"]["total"], 0);
    let (status, created) = json_request(
        app,
        "POST",
        &format!("{base}/create"),
        &cookie,
        &csrf,
        json!({"title":"before"}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    let id = created["data"]["id"].as_str().unwrap();
    let (_, list) = get(app, &format!("{base}/list"), &cookie).await;
    assert_eq!(
        list["data"]["total"], 1,
        "create must invalidate a warmed list"
    );
    let record_path = format!("{base}/get/{id}");
    let (_, before) = get(app, &record_path, &cookie).await;
    assert_eq!(before["data"]["title"], "before");
    let (status, updated) = json_request(
        app,
        "PATCH",
        &format!("{base}/update/{id}"),
        &cookie,
        &csrf,
        json!({"title":"after"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{updated}");
    let (_, after) = get(app, &record_path, &cookie).await;
    assert_eq!(
        after["data"]["title"], "after",
        "update must invalidate a warmed get"
    );
    let (_, list) = get(app, &format!("{base}/list"), &cookie).await;
    assert_eq!(list["data"]["items"][0]["title"], "after");
    let (status, parent) = json_request(
        app,
        "POST",
        &format!("{base}/create"),
        &cookie,
        &csrf,
        json!({"title":"parent"}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{parent}");
    let parent_id = parent["data"]["id"].as_str().unwrap();
    // Warm the record with parent_id = null, then change structure through tree/move.
    let (_, before_move) = get(app, &record_path, &cookie).await;
    assert!(before_move["data"]["parent_id"].is_null());
    let (status, moved) = json_request(
        app,
        "POST",
        &format!("{base}/tree/move/{id}"),
        &cookie,
        &csrf,
        json!({"new_parent_id":parent_id}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{moved}");
    let (_, after_move) = get(app, &record_path, &cookie).await;
    assert_eq!(after_move["data"]["parent_id"], parent_id);
    let (_, list) = get(app, &format!("{base}/list"), &cookie).await;
    assert_eq!(list["data"]["total"], 2);
    let (status, rejected) = json_request(
        app,
        "POST",
        &format!("{base}/tree/move/{parent_id}"),
        &cookie,
        &csrf,
        json!({"new_parent_id":id}),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{rejected}");
    let (_, unchanged) = get(app, &record_path, &cookie).await;
    assert_eq!(unchanged["data"], after_move["data"]);
    let (status, deleted) = json_request(
        app,
        "POST",
        &format!("{base}/tree/delete-subtree/{parent_id}"),
        &cookie,
        &csrf,
        json!({"expected_affected_count":2}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{deleted}");
    assert_eq!(
        get(app, &record_path, &cookie).await.0,
        StatusCode::NOT_FOUND
    );
    let (_, empty) = get(app, &format!("{base}/list"), &cookie).await;
    assert_eq!(empty["data"]["total"], 0);
}
