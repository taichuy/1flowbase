use super::*;

#[tokio::test]
async fn frontstage_bootstrap_resolves_landing_and_enforces_two_layer_block_and_code_acl() {
    let app = test_app().await;
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let workspace_id = current_workspace_id(&app, &root_cookie).await;

    let (status, created_page) = send_json(
        &app,
        "POST",
        "/api/console/frontstage/pages",
        &root_cookie,
        &root_csrf,
        json!({
            "title": "AI Dashboard",
            "placement": "topbar",
            "slug": "ai-dashboard"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{created_page}");
    let page_id = created_page["data"]["page"]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let tab_id = created_page["data"]["default_tab"]["id"]
        .as_str()
        .unwrap()
        .to_owned();

    let initial_source = "export default function ReportBlock() { return <div>v1</div>; }";
    let (block_status, block_res) = send_json(
        &app,
        "POST",
        &format!("/api/console/frontstage/pages/{page_id}/blocks"),
        &root_cookie,
        &root_csrf,
        json!({
            "tab_id": tab_id,
            "title": "Hero metric",
            "presentation": "inline",
            "source_code": initial_source
        }),
    )
    .await;
    assert_eq!(block_status, StatusCode::CREATED, "{block_res}");
    let block_id = block_res["data"]["block_id"].as_str().unwrap().to_owned();

    // 1. Root 1-RTT bootstrap returns pages + tabs + page_detail + block_roots + live raw source_code
    let raw_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/console/frontstage/bootstrap?slug=ai-dashboard")
                .header("cookie", &root_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(raw_response.status(), StatusCode::OK);
    assert_eq!(
        raw_response.headers().get("cache-control").unwrap(),
        "private, no-cache"
    );
    let root_payload: Value = serde_json::from_slice(
        &to_bytes(raw_response.into_body(), 1024 * 1024)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(root_payload["data"]["resolved_page_id"], json!(page_id));
    assert_eq!(root_payload["data"]["resolved_tab_id"], json!(tab_id));
    assert_eq!(
        root_payload["data"]["page_detail"]["page"]["id"],
        json!(page_id)
    );
    assert_eq!(
        root_payload["data"]["block_roots"][0]["block_id"],
        json!(block_id)
    );
    assert_eq!(
        root_payload["data"]["block_codes"][0]["source_code"],
        json!(initial_source)
    );

    // 2. AI/author updating database source_code immediately reflects in bootstrap without stale precompilation
    let ai_updated_source = "export default function ReportBlock() { return <div>ai-v2</div>; }";
    let (save_status, save_body) = send_json(
        &app,
        "PUT",
        &format!("/api/console/frontstage/pages/{page_id}/blocks/{block_id}/code"),
        &root_cookie,
        &root_csrf,
        json!({ "source_code": ai_updated_source }),
    )
    .await;
    assert_eq!(save_status, StatusCode::OK, "{save_body}");
    let (recheck_status, recheck_body) = send_json(
        &app,
        "GET",
        "/api/console/frontstage/bootstrap?slug=ai-dashboard",
        &root_cookie,
        &root_csrf,
        json!({}),
    )
    .await;
    assert_eq!(recheck_status, StatusCode::OK, "{recheck_body}");
    assert_eq!(
        recheck_body["data"]["block_codes"][0]["source_code"],
        json!(ai_updated_source)
    );

    // 3. Non-root member WITHOUT frontstage.blocks.view / frontstage.blocks.code.view gets pages/tabs/detail ONLY (no ACL leak)
    let member_id = create_member(
        &app,
        &root_cookie,
        &root_csrf,
        "bootstrap-viewer",
        "temp-pass",
    )
    .await;
    create_role(&app, &root_cookie, &root_csrf, "bootstrap_viewer").await;
    replace_role_permissions(&app, &root_cookie, &root_csrf, "bootstrap_viewer", &[]).await;
    replace_member_roles(
        &app,
        &root_cookie,
        &root_csrf,
        &member_id,
        &["bootstrap_viewer"],
    )
    .await;
    let (vis_status, vis_body) = send_json(
        &app,
        "PUT",
        "/api/console/settings/roles/bootstrap_viewer/frontstage-routes",
        &root_cookie,
        &root_csrf,
        json!({ "page_ids": [page_id], "tab_ids": [tab_id] }),
    )
    .await;
    assert_eq!(vis_status, StatusCode::NO_CONTENT, "{vis_body}");

    let (viewer_cookie, viewer_csrf) =
        login_and_capture_cookie(&app, "bootstrap-viewer", "temp-pass").await;
    let (viewer_status, viewer_body) = send_json(
        &app,
        "GET",
        "/api/console/frontstage/bootstrap?slug=ai-dashboard",
        &viewer_cookie,
        &viewer_csrf,
        json!({}),
    )
    .await;
    assert_eq!(viewer_status, StatusCode::OK, "{viewer_body}");
    assert_eq!(viewer_body["data"]["resolved_page_id"], json!(page_id));
    assert!(viewer_body["data"]["page_detail"].is_object());
    assert!(
        viewer_body["data"]["block_roots"].is_null(),
        "must not return block_roots without frontstage.blocks.view: {viewer_body}"
    );
    assert!(
        viewer_body["data"]["block_codes"].is_null(),
        "must not return block_codes without frontstage.blocks.code.view: {viewer_body}"
    );

    // 4. Granting list_frontstage_block_roots WITHOUT get_frontstage_block_node_code returns block_roots ONLY
    let (cat_status, catalog) = send_json(
        &app,
        "GET",
        "/api/console/settings/roles/console-policy-catalog",
        &root_cookie,
        &root_csrf,
        json!({}),
    )
    .await;
    assert_eq!(cat_status, StatusCode::OK, "{catalog}");
    let ui_group = catalog["data"]["groups"]
        .as_array()
        .unwrap()
        .iter()
        .find(|group| group["group_id"] == "system.ui-management")
        .unwrap();
    let operations = ui_group["operations"].as_array().unwrap();
    let blocks_op_id = operations
        .iter()
        .find(|op| {
            op["route"]["method"] == "GET"
                && op["route"]["path"] == "/api/console/frontstage/pages/:page_id/blocks"
        })
        .unwrap()["operation_id"]
        .clone();
    let codes_op_id = operations
        .iter()
        .find(|op| {
            op["route"]["method"] == "GET"
                && op["route"]["path"]
                    == "/api/console/frontstage/pages/:page_id/blocks/:block_id/code"
        })
        .unwrap()["operation_id"]
        .clone();
    let _ = workspace_id;
    ui_management::set_ui_policy(
        &app,
        &root_cookie,
        &root_csrf,
        "bootstrap_viewer",
        true,
        "custom",
        json!([{ "kind": "simple", "operation_id": blocks_op_id, "enabled": true }]),
    )
    .await;

    let (roots_only_status, roots_only_body) = send_json(
        &app,
        "GET",
        "/api/console/frontstage/bootstrap?slug=ai-dashboard",
        &viewer_cookie,
        &viewer_csrf,
        json!({}),
    )
    .await;
    assert_eq!(roots_only_status, StatusCode::OK, "{roots_only_body}");
    assert_eq!(
        roots_only_body["data"]["block_roots"][0]["block_id"],
        json!(block_id)
    );
    assert!(
        roots_only_body["data"]["block_codes"].is_null(),
        "must still withhold block_codes when frontstage.blocks.code.view is disabled: {roots_only_body}"
    );

    // 5. Granting BOTH list_frontstage_block_roots AND get_frontstage_block_node_code returns live block_codes
    ui_management::set_ui_policy(
        &app,
        &root_cookie,
        &root_csrf,
        "bootstrap_viewer",
        true,
        "custom",
        json!([
            { "kind": "simple", "operation_id": blocks_op_id, "enabled": true },
            { "kind": "simple", "operation_id": codes_op_id, "enabled": true }
        ]),
    )
    .await;
    let (both_status, both_body) = send_json(
        &app,
        "GET",
        "/api/console/frontstage/bootstrap?slug=ai-dashboard",
        &viewer_cookie,
        &viewer_csrf,
        json!({}),
    )
    .await;
    assert_eq!(both_status, StatusCode::OK, "{both_body}");
    assert_eq!(
        both_body["data"]["block_codes"][0]["source_code"],
        json!(ai_updated_source)
    );

    // 6. Explicit invisible ?page_id=<secret_page_id> fail-closes even when both block & code operations are granted
    let (secret_status, secret_page) = send_json(
        &app,
        "POST",
        "/api/console/frontstage/pages",
        &root_cookie,
        &root_csrf,
        json!({
            "title": "Secret Admin Page",
            "placement": "topbar",
            "slug": "secret-admin-page"
        }),
    )
    .await;
    assert_eq!(secret_status, StatusCode::CREATED, "{secret_page}");
    let secret_page_id = secret_page["data"]["page"]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let secret_tab_id = secret_page["data"]["default_tab"]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let (secret_block_status, secret_block) = send_json(
        &app,
        "POST",
        &format!("/api/console/frontstage/pages/{secret_page_id}/blocks"),
        &root_cookie,
        &root_csrf,
        json!({
            "tab_id": secret_tab_id,
            "title": "Top Secret Block",
            "presentation": "inline",
            "source_code": "export default function Secret() { return <div>TOP_SECRET</div>; }"
        }),
    )
    .await;
    assert_eq!(secret_block_status, StatusCode::CREATED, "{secret_block}");

    let (invisible_status, invisible_body) = send_json(
        &app,
        "GET",
        &format!("/api/console/frontstage/bootstrap?page_id={secret_page_id}"),
        &viewer_cookie,
        &viewer_csrf,
        json!({}),
    )
    .await;
    assert_eq!(invisible_status, StatusCode::OK, "{invisible_body}");
    assert!(
        invisible_body["data"]["resolved_page_id"].is_null(),
        "must not resolve invisible page_id: {invisible_body}"
    );
    assert_eq!(invisible_body["data"]["tabs"], json!([]));
    assert!(invisible_body["data"]["page_detail"].is_null());
    assert!(invisible_body["data"]["block_roots"].is_null());
    assert!(invisible_body["data"]["block_codes"].is_null());
    let visible_ids: Vec<&str> = invisible_body["data"]["pages"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|node| node["id"].as_str())
        .collect();
    assert!(!visible_ids.contains(&secret_page_id.as_str()));
}

#[tokio::test]
async fn frontstage_bootstrap_supports_etag_304_viewport_code_limit_and_gzip_compression() {
    use std::io::Read;

    let app = test_app().await;
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;

    let (status, created_page) = send_json(
        &app,
        "POST",
        "/api/console/frontstage/pages",
        &root_cookie,
        &root_csrf,
        json!({
            "title": "Viewport Funnel Page",
            "placement": "topbar",
            "slug": "viewport-funnel"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{created_page}");
    let page_id = created_page["data"]["page"]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let tab_id = created_page["data"]["default_tab"]["id"]
        .as_str()
        .unwrap()
        .to_owned();

    // Create offscreen block first (y = 32), then top-of-viewport block second (y = 0)
    let (b1_status, b1_res) = send_json(
        &app,
        "POST",
        &format!("/api/console/frontstage/pages/{page_id}/blocks"),
        &root_cookie,
        &root_csrf,
        json!({
            "tab_id": tab_id,
            "title": "Offscreen Bottom Block",
            "presentation": "page",
            "source_code": format!("export default function BottomBlock() {{ return <div>{}</div>; }}", "B".repeat(900)),
            "runtime_descriptor": {
                "x-layout": { "order": 0, "x": 0, "y": 32, "w": 12, "h": 8, "lg": { "x": 0, "y": 32, "w": 12, "h": 8 } }
            }
        }),
    )
    .await;
    assert_eq!(b1_status, StatusCode::CREATED, "{b1_res}");
    let bottom_block_id = b1_res["data"]["block_id"].as_str().unwrap().to_owned();

    let (b2_status, b2_res) = send_json(
        &app,
        "POST",
        &format!("/api/console/frontstage/pages/{page_id}/blocks"),
        &root_cookie,
        &root_csrf,
        json!({
            "tab_id": tab_id,
            "title": "Visible Top Block",
            "presentation": "page",
            "source_code": format!("export default function TopBlock() {{ return <div>{}</div>; }}", "T".repeat(900)),
            "runtime_descriptor": {
                "x-layout": { "order": 1, "x": 0, "y": 0, "w": 12, "h": 8, "lg": { "x": 0, "y": 0, "w": 12, "h": 8 } }
            }
        }),
    )
    .await;
    assert_eq!(b2_status, StatusCode::CREATED, "{b2_res}");
    let top_block_id = b2_res["data"]["block_id"].as_str().unwrap().to_owned();

    // 1. Request with code_limit=1 and Accept-Encoding: gzip -> gzipped 200 OK with ETag,
    //    all 2 block_roots with source_sha256, and ONLY the visually top-most block's source_code!
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/console/frontstage/bootstrap?slug=viewport-funnel&code_limit=1")
                .header("cookie", &root_cookie)
                .header("accept-encoding", "br, gzip")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.headers().get("content-encoding").unwrap(), "gzip");
    let etag = res
        .headers()
        .get("etag")
        .unwrap()
        .to_str()
        .unwrap()
        .to_owned();
    assert!(etag.starts_with('"') && etag.ends_with('"'));

    let compressed = to_bytes(res.into_body(), 1024 * 1024).await.unwrap();
    let mut decompressed = Vec::new();
    flate2::read::GzDecoder::new(&compressed[..])
        .read_to_end(&mut decompressed)
        .unwrap();
    let payload: Value = serde_json::from_slice(&decompressed).unwrap();
    let roots = payload["data"]["block_roots"].as_array().unwrap();
    assert_eq!(roots.len(), 2);
    assert!(roots
        .iter()
        .all(|node| node["source_sha256"].as_str().is_some()));

    let codes = payload["data"]["block_codes"].as_array().unwrap();
    assert_eq!(codes.len(), 1);
    assert_eq!(codes[0]["block_id"], json!(top_block_id));
    assert_ne!(codes[0]["block_id"], json!(bottom_block_id));

    // 2. Conditional If-None-Match returns 304 Not Modified with empty body
    let res_304 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/console/frontstage/bootstrap?slug=viewport-funnel&code_limit=1")
                .header("cookie", &root_cookie)
                .header("if-none-match", &etag)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_304.status(), StatusCode::NOT_MODIFIED);
    assert_eq!(res_304.headers().get("etag").unwrap(), etag.as_str());
    let empty_304 = to_bytes(res_304.into_body(), 1024 * 1024).await.unwrap();
    assert!(empty_304.is_empty());

    // 3. Updating source_code invalidates ETag immediately -> 200 OK with new ETag
    let (save_status, _) = send_json(
        &app,
        "PUT",
        &format!("/api/console/frontstage/pages/{page_id}/blocks/{top_block_id}/code"),
        &root_cookie,
        &root_csrf,
        json!({
            "source_code": "export default function TopBlock() { return <div>UPDATED_LIVE</div>; }"
        }),
    )
    .await;
    assert_eq!(save_status, StatusCode::OK);

    let res_after_edit = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/console/frontstage/bootstrap?slug=viewport-funnel&code_limit=1")
                .header("cookie", &root_cookie)
                .header("if-none-match", &etag)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_after_edit.status(), StatusCode::OK);
    let new_etag = res_after_edit
        .headers()
        .get("etag")
        .unwrap()
        .to_str()
        .unwrap()
        .to_owned();
    assert_ne!(new_etag, etag);
}
