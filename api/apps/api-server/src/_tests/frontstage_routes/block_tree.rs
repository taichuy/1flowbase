use super::*;

#[allow(clippy::too_many_arguments)]
async fn create_block(
    app: &axum::Router,
    cookie: &str,
    csrf: &str,
    workspace_id: &str,
    page_id: &str,
    tab_id: Option<&str>,
    title: &str,
    parent_block_id: Option<&str>,
    code: &str,
    runtime_descriptor: Option<Value>,
) -> (StatusCode, Value) {
    let mut body = json!({
        "title": title,
        "description": format!("Description for {title}"),
        "presentation": "inline",
        "parent_block_id": parent_block_id,
        "runtime_descriptor": runtime_descriptor,
    });
    if let Some(tab_id) = tab_id {
        body["tab_id"] = json!(tab_id);
    }
    body.as_object_mut()
        .expect("block body must be an object")
        .extend(
            ready_executable_payload(code)
                .as_object()
                .expect("executable payload must be an object")
                .clone(),
        );
    send_json(
        app,
        "POST",
        &format!("/api/console/frontstage/{workspace_id}/pages/{page_id}/blocks"),
        cookie,
        csrf,
        body,
    )
    .await
}

async fn create_block_page(
    app: &axum::Router,
    cookie: &str,
    csrf: &str,
    workspace_id: &str,
) -> (String, String) {
    let (status, payload) = send_json(
        app,
        "POST",
        &format!("/api/console/frontstage/{workspace_id}/pages"),
        cookie,
        csrf,
        json!({
            "title": "Block tree",
            "rank": "a",
            "placement": "topbar",
            "slug": "block-tree"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{payload}");
    (
        payload["data"]["page"]["id"].as_str().unwrap().to_owned(),
        payload["data"]["default_tab"]["id"]
            .as_str()
            .unwrap()
            .to_owned(),
    )
}

fn assert_error(payload: &Value, code: &str) {
    assert_eq!(payload["code"], json!(code), "{payload}");
}

#[tokio::test]
async fn canonical_block_tree_supports_public_projection_traversal_code_and_guarded_deletion() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let workspace_id = current_workspace_id(&app, &cookie).await;
    let (page_id, tab_id) = create_block_page(&app, &cookie, &csrf, &workspace_id).await;
    let blocks_path = format!("/api/console/frontstage/{workspace_id}/pages/{page_id}/blocks");

    let (root_status, root_payload) = create_block(
        &app,
        &cookie,
        &csrf,
        &workspace_id,
        &page_id,
        Some(&tab_id),
        "Root block",
        None,
        "export default 'root';",
        Some(json!({
            "id": "caller-controlled-id",
            "codeRef": "caller-controlled-code-ref",
            "customRendererOption": true,
        })),
    )
    .await;
    assert_eq!(root_status, StatusCode::CREATED, "{root_payload}");
    let root = &root_payload["data"];
    let root_id = root["block_id"].as_str().unwrap();
    uuid::Uuid::parse_str(root_id).expect("server should generate a public UUID block id");
    for internal_field in ["id", "model_code", "physical_table_name"] {
        assert!(
            root.get(internal_field).is_none(),
            "leaked {internal_field}: {root}"
        );
    }
    assert_eq!(root["input_mapping"], json!({}));
    assert_eq!(root["output_mapping"], json!({}));
    assert_eq!(root["schema_version"], json!(1));
    assert_eq!(root["description"], json!("Description for Root block"));
    assert_eq!(root["runtime_descriptor"]["id"], json!(root_id));
    assert_eq!(
        root["runtime_descriptor"]["codeRef"],
        json!(format!("frontstage.block.{root_id}"))
    );
    assert_eq!(root["runtime_descriptor"]["rendererVersion"], json!("v1"));
    assert_eq!(
        root["code_ref"],
        json!(format!("frontstage.block.{root_id}"))
    );
    assert_eq!(
        root["runtime_descriptor"]["customRendererOption"],
        json!(true)
    );

    let (open_status, open_payload) =
        get_json(&app, &format!("{blocks_path}/{root_id}/open"), &cookie).await;
    assert_eq!(open_status, StatusCode::OK, "{open_payload}");
    assert_eq!(
        open_payload["data"]["canonical_url"],
        json!(format!("/block-tree/pages/{page_id}/blocks/{root_id}"))
    );

    let (descriptor_status, descriptor_payload) = send_json(
        &app,
        "PUT",
        &format!(
            "/api/console/frontstage/{workspace_id}/pages/{page_id}/tabs/{tab_id}/block-descriptors"
        ),
        &cookie,
        &csrf,
        json!({
            "updates": [{
                "block_id": root_id,
                "runtime_descriptor": {
                    "x-layout": { "order": 7 },
                    "customRendererOption": true
                }
            }]
        }),
    )
    .await;
    assert_eq!(descriptor_status, StatusCode::OK, "{descriptor_payload}");
    assert_eq!(
        descriptor_payload["data"][0]["runtime_descriptor"]["x-layout"]["order"],
        json!(7)
    );
    assert_eq!(
        descriptor_payload["data"][0]["runtime_descriptor"]["id"],
        json!(root_id)
    );

    let (_, child_payload) = create_block(
        &app,
        &cookie,
        &csrf,
        &workspace_id,
        &page_id,
        None,
        "Searchable child",
        Some(root_id),
        "export default 'child';",
        None,
    )
    .await;
    let child_id = child_payload["data"]["block_id"]
        .as_str()
        .unwrap()
        .to_owned();
    let (_, grandchild_payload) = create_block(
        &app,
        &cookie,
        &csrf,
        &workspace_id,
        &page_id,
        None,
        "Searchable grandchild",
        Some(&child_id),
        "export default 'grandchild';",
        None,
    )
    .await;
    let grandchild_id = grandchild_payload["data"]["block_id"]
        .as_str()
        .unwrap()
        .to_owned();

    let (roots_status, roots_payload) =
        get_json(&app, &format!("{blocks_path}?tab_id={tab_id}"), &cookie).await;
    assert_eq!(roots_status, StatusCode::OK);
    assert_eq!(roots_payload["data"].as_array().unwrap().len(), 1);
    assert_eq!(roots_payload["data"][0]["block_id"], json!(root_id));

    let (children_status, children_payload) =
        get_json(&app, &format!("{blocks_path}/{root_id}/children"), &cookie).await;
    assert_eq!(children_status, StatusCode::OK);
    assert_eq!(children_payload["data"][0]["block_id"], json!(child_id));

    let (search_status, search_payload) = get_json(
        &app,
        &format!("{blocks_path}/search?tab_id={tab_id}&query=Searchable%20grand"),
        &cookie,
    )
    .await;
    assert_eq!(search_status, StatusCode::OK);
    assert_eq!(
        search_payload["data"][0]["node"]["block_id"],
        json!(grandchild_id)
    );
    assert_eq!(
        search_payload["data"][0]["ancestors"][0]["block_id"],
        json!(root_id)
    );
    assert_eq!(
        search_payload["data"][0]["ancestors"][1]["block_id"],
        json!(child_id)
    );

    let (descendants_status, descendants_payload) = get_json(
        &app,
        &format!("{blocks_path}/{root_id}/descendants"),
        &cookie,
    )
    .await;
    assert_eq!(descendants_status, StatusCode::OK);
    assert_eq!(descendants_payload["data"][0]["depth"], json!(1));
    assert_eq!(descendants_payload["data"][0]["has_children"], json!(true));
    assert_eq!(
        descendants_payload["data"][0]["path"],
        json!([root_id, child_id])
    );
    assert_eq!(descendants_payload["data"][1]["depth"], json!(2));
    assert_eq!(descendants_payload["data"][1]["has_children"], json!(false));
    assert_eq!(
        descendants_payload["data"][1]["path"],
        json!([root_id, child_id, grandchild_id])
    );

    let (impact_status, impact_payload) = get_json(
        &app,
        &format!("{blocks_path}/{root_id}/delete-impact"),
        &cookie,
    )
    .await;
    assert_eq!(impact_status, StatusCode::OK);
    assert_eq!(impact_payload["data"]["affected_count"], json!(3));

    let (cycle_status, cycle_payload) = send_json(
        &app,
        "POST",
        &format!("{blocks_path}/{root_id}/move"),
        &cookie,
        &csrf,
        json!({ "parent_block_id": grandchild_id }),
    )
    .await;
    assert_eq!(cycle_status, StatusCode::CONFLICT);
    assert_error(&cycle_payload, "block_tree_cycle");

    let (leaf_status, leaf_payload) = send_json(
        &app,
        "DELETE",
        &format!("{blocks_path}/{root_id}"),
        &cookie,
        &csrf,
        json!({}),
    )
    .await;
    assert_eq!(leaf_status, StatusCode::CONFLICT);
    assert_error(&leaf_payload, "block_node_has_children");

    let (stale_status, stale_payload) = send_json(
        &app,
        "POST",
        &format!("{blocks_path}/{root_id}/delete-subtree"),
        &cookie,
        &csrf,
        json!({ "expected_affected_count": 2 }),
    )
    .await;
    assert_eq!(stale_status, StatusCode::CONFLICT);
    assert_error(&stale_payload, "block_subtree_changed");

    let code_path = format!("{blocks_path}/{child_id}/code");
    let (initial_code_status, initial_code_payload) = get_json(&app, &code_path, &cookie).await;
    assert_eq!(initial_code_status, StatusCode::OK);
    assert_eq!(initial_code_payload["data"]["block_id"], json!(child_id));
    assert_eq!(
        initial_code_payload["data"]["source_code"],
        json!("export default 'child';")
    );
    assert!(initial_code_payload["data"].get("code_ref").is_none());
    let initial_hash = initial_code_payload["data"]["source_sha256"].clone();

    let (save_code_status, save_code_payload) = send_json(
        &app,
        "PUT",
        &code_path,
        &cookie,
        &csrf,
        ready_executable_payload("export default 'changed';"),
    )
    .await;
    assert_eq!(save_code_status, StatusCode::OK);
    assert_eq!(
        save_code_payload["data"]["source_code"],
        json!("export default 'changed';")
    );
    assert_ne!(save_code_payload["data"]["source_sha256"], initial_hash);
    let (update_status, update_payload) = send_json(
        &app,
        "PATCH",
        &format!("{blocks_path}/{child_id}"),
        &cookie,
        &csrf,
        json!({ "description": "Updated child description" }),
    )
    .await;
    assert_eq!(update_status, StatusCode::OK, "{update_payload}");
    assert_eq!(
        update_payload["data"]["description"],
        json!("Updated child description")
    );
    let (_, child_detail) = get_json(&app, &format!("{blocks_path}/{child_id}"), &cookie).await;
    assert_eq!(child_detail["data"]["title"], json!("Searchable child"));
    assert_eq!(
        child_detail["data"]["description"],
        json!("Updated child description")
    );
    let (clear_status, clear_payload) = send_json(
        &app,
        "PATCH",
        &format!("{blocks_path}/{child_id}"),
        &cookie,
        &csrf,
        json!({ "description": "   " }),
    )
    .await;
    assert_eq!(clear_status, StatusCode::OK, "{clear_payload}");
    assert_eq!(clear_payload["data"]["description"], Value::Null);

    let (delete_status, delete_payload) = send_json(
        &app,
        "POST",
        &format!("{blocks_path}/{root_id}/delete-subtree"),
        &cookie,
        &csrf,
        json!({ "expected_affected_count": 3 }),
    )
    .await;
    assert_eq!(delete_status, StatusCode::OK, "{delete_payload}");
    assert_eq!(delete_payload["data"]["deleted_count"], json!(3));

    let (missing_status, missing_payload) =
        get_json(&app, &format!("{blocks_path}/{root_id}"), &cookie).await;
    assert_eq!(missing_status, StatusCode::NOT_FOUND);
    assert_error(&missing_payload, "block_node_not_found");
}

#[tokio::test]
async fn block_tree_writes_require_csrf_and_bulk_routes_require_design_permission() {
    let app = test_app().await;
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let workspace_id = current_workspace_id(&app, &root_cookie).await;
    let (page_id, tab_id) = create_block_page(&app, &root_cookie, &root_csrf, &workspace_id).await;
    let blocks_path = format!("/api/console/frontstage/{workspace_id}/pages/{page_id}/blocks");
    let (_, block_payload) = create_block(
        &app,
        &root_cookie,
        &root_csrf,
        &workspace_id,
        &page_id,
        Some(&tab_id),
        "Protected block",
        None,
        "export default 1;",
        None,
    )
    .await;
    let block_id = block_payload["data"]["block_id"].as_str().unwrap();

    let no_csrf_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("{blocks_path}/{block_id}"))
                .header("cookie", &root_cookie)
                .header("content-type", "application/json")
                .body(Body::from(json!({ "title": "Denied" }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(no_csrf_response.status(), StatusCode::UNAUTHORIZED);

    let member_id = create_member(
        &app,
        &root_cookie,
        &root_csrf,
        "block-tree-viewer",
        "temp-pass",
    )
    .await;
    create_role(&app, &root_cookie, &root_csrf, "block_tree_viewer").await;
    replace_role_permissions(&app, &root_cookie, &root_csrf, "block_tree_viewer", &[]).await;
    replace_member_roles(
        &app,
        &root_cookie,
        &root_csrf,
        &member_id,
        &["block_tree_viewer"],
    )
    .await;
    let (viewer_cookie, viewer_csrf) =
        login_and_capture_cookie(&app, "block-tree-viewer", "temp-pass").await;

    let (roots_status, _) = get_json(
        &app,
        &format!("{blocks_path}?tab_id={tab_id}"),
        &viewer_cookie,
    )
    .await;
    assert_eq!(roots_status, StatusCode::FORBIDDEN);
    let (search_status, _) = get_json(
        &app,
        &format!("{blocks_path}/search?tab_id={tab_id}&query=Protected"),
        &viewer_cookie,
    )
    .await;
    assert_eq!(search_status, StatusCode::FORBIDDEN);
    let (write_status, _) = create_block(
        &app,
        &viewer_cookie,
        &viewer_csrf,
        &workspace_id,
        &page_id,
        Some(&tab_id),
        "Denied",
        None,
        "",
        None,
    )
    .await;
    assert_eq!(write_status, StatusCode::FORBIDDEN);

    for runtime_path in [
        format!("{blocks_path}/{block_id}"),
        format!("{blocks_path}/{block_id}/ancestors"),
        format!("{blocks_path}/{block_id}/code"),
        format!("{blocks_path}/{block_id}/open"),
    ] {
        let (status, payload) = get_json(&app, &runtime_path, &viewer_cookie).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{runtime_path}: {payload}");
        assert_error(&payload, "block_node_not_found");
    }
}
