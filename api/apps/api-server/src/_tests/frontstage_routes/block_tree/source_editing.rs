use super::*;

// #2027 AC-002..007: real authenticated HTTP entry, exact persistence and rejection.
#[tokio::test]
async fn issue_2027_source_search_and_exact_edits_are_bounded_atomic_and_revision_checked() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let workspace = current_workspace_id(&app, &cookie).await;
    let (page, tab) = create_block_page(&app, &cookie, &csrf, &workspace).await;
    let source = "header\r\n订单😀 notice\r\nnotice again\r\nfooter";
    let (status, block) = create_block(
        &app,
        &cookie,
        &csrf,
        &workspace,
        &page,
        Some(&tab),
        "Source editing",
        None,
        source,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{block}");
    let id = block["data"]["block_id"].as_str().unwrap();
    let path = format!("/api/console/frontstage/pages/{page}/blocks/{id}/code");
    let (status, found) = get_json(
        &app,
        &format!("{path}/search?query=notice&limit=1"),
        &cookie,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{found}");
    let found = &found["data"];
    assert_eq!(found["matches"].as_array().unwrap().len(), 1);
    assert_eq!(found["matches"][0]["start_line"], 2);
    assert_eq!(found["matches"][0]["start_column"], 5);
    assert_eq!(found["truncated"], true);
    let revision = found["source_revision"].as_str().unwrap();
    for (edits, expected_code) in [
        (
            json!([{"old_text":"header", "new_text":"changed"}, {"old_text":"absent", "new_text":"bad"}]),
            "source_text_not_found",
        ),
        (
            json!([{"old_text":"notice", "new_text":"bad"}]),
            "source_text_ambiguous",
        ),
        (
            json!([{"old_text":"notice again", "new_text":"bad"}, {"old_text":"again", "new_text":"bad"}]),
            "source_edit_overlap",
        ),
    ] {
        let (status, error) = send_json(
            &app,
            "POST",
            &format!("{path}/replace"),
            &cookie,
            &csrf,
            json!({"expected_source_revision": revision, "edits": edits}),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{error}");
        assert_eq!(error["code"], expected_code);
        assert!(error["details"]["edit_index"].is_number(), "{error}");
        let (_, unchanged) = get_json(&app, &path, &cookie).await;
        assert_eq!(unchanged["data"]["source_code"], source);
    }
    let edits = json!([
        {"old_text":"header", "new_text":"header\r\ninserted"},
        {"old_text":"订单😀 notice", "new_text":"订单✅ message"},
        {"old_text":"notice again\r\n", "new_text":""}
    ]);
    let (status, saved) = send_json(
        &app,
        "POST",
        &format!("{path}/replace"),
        &cookie,
        &csrf,
        json!({"expected_source_revision": revision, "edits": edits}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    assert_eq!(saved["data"]["applied_edits"], 3);
    assert!(saved["data"].get("source_code").is_none());
    assert_eq!(saved["data"]["diff_truncated"], false);
    assert_ne!(saved["data"]["source_revision"], revision);
    let (_, current) = get_json(&app, &path, &cookie).await;
    assert_eq!(
        current["data"]["source_code"],
        "header\r\ninserted\r\n订单✅ message\r\nfooter"
    );
    let (status, stale) = send_json(
        &app,
        "POST",
        &format!("{path}/replace"),
        &cookie,
        &csrf,
        json!({"expected_source_revision": revision, "edits": edits}),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{stale}");
    assert_eq!(
        stale["details"]["current_source_revision"],
        saved["data"]["source_revision"]
    );
}

// #2027 AC-001: catalog and mounted endpoints must expose the same executable operations.
#[test]
fn issue_2027_source_editing_routes_are_mounted_and_documented() {
    let assembly = crate::routes::frontstage::route_assembly();
    let openapi = crate::openapi::ApiDoc::openapi();
    for (method, suffix) in [("GET", "search"), ("POST", "replace")] {
        let path = format!("/api/console/frontstage/pages/:page_id/blocks/:block_id/code/{suffix}");
        assert!(
            assembly
                .bindings()
                .iter()
                .any(|binding| binding.route.method == method && binding.route.path == path),
            "missing {method} {path}"
        );
        let documented = path
            .replace(":page_id", "{page_id}")
            .replace(":block_id", "{block_id}");
        assert!(openapi.paths.paths.contains_key(&documented));
    }
}
