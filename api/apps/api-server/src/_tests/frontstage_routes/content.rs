use super::*;

#[tokio::test]
async fn page_detail_and_block_code_round_trip_are_persisted_by_page_scope() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let workspace_id = current_workspace_id(&app, &cookie).await;
    let (_, group_payload) =
        create_group(&app, &cookie, &csrf, &workspace_id, Some("Group"), "a").await;
    let group_id = group_payload["data"]["id"].as_str().unwrap();
    let (page_status, page_payload) = create_page(
        &app,
        &cookie,
        &csrf,
        &workspace_id,
        Some("Child"),
        Some(group_id),
        "a",
    )
    .await;
    assert_eq!(page_status, StatusCode::CREATED);
    let page_id = page_payload["data"]["id"].as_str().unwrap();
    let tab_id = page_payload["data"]["default_tab"]["id"].as_str().unwrap();
    let schema_root_uid = page_payload["data"]["default_tab"]["document_root_uid"]
        .as_str()
        .unwrap();

    let (detail_status, detail_payload) = get_json(
        &app,
        &format!("/api/console/frontstage/{workspace_id}/pages/{page_id}/tabs/{tab_id}"),
        &cookie,
    )
    .await;
    assert_eq!(detail_status, StatusCode::OK);
    assert_eq!(detail_payload["data"]["page"]["id"], json!(page_id));
    assert_eq!(
        detail_payload["data"]["document"]["root_uid"],
        json!(schema_root_uid)
    );

    let code_path =
        format!("/api/console/frontstage/{workspace_id}/pages/{page_id}/block-codes/hero");
    let (save_status, save_payload) = send_json(
        &app,
        "PUT",
        &code_path,
        &cookie,
        &csrf,
        json!({ "code": "export default function Hero() { return 'v1'; }" }),
    )
    .await;
    assert_eq!(save_status, StatusCode::OK);
    assert_eq!(
        save_payload["data"]["code"],
        json!("export default function Hero() { return 'v1'; }")
    );
    assert_eq!(
        save_payload["data"]["source_sha256"],
        json!("ab3463e9dfd1db406efe272f203d2d1c5e4ce7b39fdef237c7c0ae1769b3a46e")
    );

    let (overwrite_status, overwrite_payload) = send_json(
        &app,
        "PUT",
        &code_path,
        &cookie,
        &csrf,
        json!({ "code": "export default function Hero() { return 'v1'; }\n" }),
    )
    .await;
    assert_eq!(overwrite_status, StatusCode::OK);
    assert_eq!(
        overwrite_payload["data"]["source_sha256"],
        json!("8a4cd977e632b3c538e8aa30fa97b8d0e3a4f220ec246bd29c89a427ebe3e599")
    );
    assert_ne!(
        save_payload["data"]["source_sha256"],
        overwrite_payload["data"]["source_sha256"]
    );

    let (read_status, read_payload) = get_json(&app, &code_path, &cookie).await;
    assert_eq!(read_status, StatusCode::OK);
    assert_eq!(read_payload["data"]["code_ref"], json!("hero"));
    assert_eq!(
        read_payload["data"]["code"],
        json!("export default function Hero() { return 'v1'; }\n")
    );
    assert_eq!(
        read_payload["data"]["source_sha256"],
        overwrite_payload["data"]["source_sha256"]
    );

    let (_, other_page_payload) = create_page(
        &app,
        &cookie,
        &csrf,
        &workspace_id,
        Some("Sibling"),
        Some(group_id),
        "b",
    )
    .await;
    let other_page_id = other_page_payload["data"]["id"].as_str().unwrap();
    let (other_page_code_status, _) = get_json(
        &app,
        &format!("/api/console/frontstage/{workspace_id}/pages/{other_page_id}/block-codes/hero"),
        &cookie,
    )
    .await;
    assert_eq!(other_page_code_status, StatusCode::NOT_FOUND);

    let delete_status = delete_node(&app, &cookie, &csrf, &workspace_id, group_id).await;
    assert_eq!(delete_status, StatusCode::NO_CONTENT);
    let (deleted_code_status, _) = get_json(&app, &code_path, &cookie).await;
    assert_eq!(deleted_code_status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn page_content_save_round_trip_is_persisted_by_page_scope() {
    let (app, database_url) = test_app_with_database_url().await;
    let other_workspace_id = seed_workspace(&database_url, "Other Content Workspace").await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let workspace_id = current_workspace_id(&app, &cookie).await;
    let (_, page_payload) = create_page(
        &app,
        &cookie,
        &csrf,
        &workspace_id,
        Some("Editable"),
        None,
        "a",
    )
    .await;
    let page_id = page_payload["data"]["id"].as_str().unwrap();
    let tab_id = page_payload["data"]["default_tab"]["id"].as_str().unwrap();
    let schema_root_uid = page_payload["data"]["default_tab"]["document_root_uid"]
        .as_str()
        .unwrap();

    let document_payload = json!({
        "version": 1,
        "root_uid": schema_root_uid,
        "blocks": [
            {
                "id": "hero-1",
                "codeRef": "hero",
                "renderer_version": "v1"
            }
        ]
    });

    let (save_status, save_payload) = save_page_content(
        &app,
        &cookie,
        &csrf,
        &workspace_id,
        page_id,
        tab_id,
        document_payload.clone(),
    )
    .await;
    assert_eq!(save_status, StatusCode::OK);
    assert_eq!(save_payload["data"]["page"]["id"], json!(page_id));
    assert_eq!(
        save_payload["data"]["document"]["payload"],
        document_payload.clone()
    );

    let (detail_status, detail_payload) = get_json(
        &app,
        &format!("/api/console/frontstage/{workspace_id}/pages/{page_id}/tabs/{tab_id}"),
        &cookie,
    )
    .await;
    assert_eq!(detail_status, StatusCode::OK);
    assert_eq!(
        detail_payload["data"]["document"]["payload"],
        document_payload.clone()
    );

    let (_, sibling_payload) = create_page(
        &app,
        &cookie,
        &csrf,
        &workspace_id,
        Some("Sibling"),
        None,
        "b",
    )
    .await;
    let sibling_page_id = sibling_payload["data"]["id"].as_str().unwrap();
    let sibling_tab_id = sibling_payload["data"]["default_tab"]["id"]
        .as_str()
        .unwrap();
    let (sibling_detail_status, sibling_detail_payload) = get_json(
        &app,
        &format!(
            "/api/console/frontstage/{workspace_id}/pages/{sibling_page_id}/tabs/{sibling_tab_id}"
        ),
        &cookie,
    )
    .await;
    assert_eq!(sibling_detail_status, StatusCode::OK);
    assert_eq!(
        sibling_detail_payload["data"]["document"]["payload"]["blocks"],
        json!([])
    );

    let other_csrf = switch_workspace(&app, &cookie, &csrf, &other_workspace_id.to_string()).await;
    let (_, other_page_payload) = create_page(
        &app,
        &cookie,
        &other_csrf,
        &other_workspace_id.to_string(),
        Some("Other Workspace Page"),
        None,
        "a",
    )
    .await;
    let other_page_id = other_page_payload["data"]["id"].as_str().unwrap();
    let other_tab_id = other_page_payload["data"]["default_tab"]["id"]
        .as_str()
        .unwrap();
    let csrf = switch_workspace(&app, &cookie, &other_csrf, &workspace_id).await;
    let (cross_workspace_status, _) = save_page_content(
        &app,
        &cookie,
        &csrf,
        &workspace_id,
        other_page_id,
        other_tab_id,
        json!({ "version": 1, "blocks": [] }),
    )
    .await;
    assert_eq!(cross_workspace_status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn page_content_save_rejects_missing_or_unsupported_block_renderer_versions() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let workspace_id = current_workspace_id(&app, &cookie).await;
    let (_, page_payload) = create_page(
        &app,
        &cookie,
        &csrf,
        &workspace_id,
        Some("Versioned Blocks"),
        None,
        "a",
    )
    .await;
    let page_id = page_payload["data"]["id"].as_str().unwrap();
    let tab_id = page_payload["data"]["default_tab"]["id"].as_str().unwrap();

    let (missing_status, missing_payload) = save_page_content(
        &app,
        &cookie,
        &csrf,
        &workspace_id,
        page_id,
        tab_id,
        json!({
            "version": 1,
            "blocks": [{ "id": "hero", "codeRef": "hero-code" }]
        }),
    )
    .await;
    assert_eq!(missing_status, StatusCode::BAD_REQUEST);
    assert_eq!(
        missing_payload["code"],
        json!("frontstage_block_renderer_version_missing")
    );

    let (unsupported_status, unsupported_payload) = save_page_content(
        &app,
        &cookie,
        &csrf,
        &workspace_id,
        page_id,
        tab_id,
        json!({
            "version": 1,
            "blocks": [{
                "id": "future",
                "codeRef": "future-code",
                "renderer_version": "v2"
            }]
        }),
    )
    .await;
    assert_eq!(unsupported_status, StatusCode::BAD_REQUEST);
    assert_eq!(
        unsupported_payload["code"],
        json!("frontstage_block_renderer_version_unsupported")
    );

    let (accepted_status, accepted_payload) = save_page_content(
        &app,
        &cookie,
        &csrf,
        &workspace_id,
        page_id,
        tab_id,
        json!({
            "version": 1,
            "blocks": [{
                "id": "hero",
                "codeRef": "hero-code",
                "renderer_version": "v1"
            }]
        }),
    )
    .await;
    assert_eq!(accepted_status, StatusCode::OK);
    assert_eq!(
        accepted_payload["data"]["document"]["payload"]["blocks"][0]["renderer_version"],
        json!("v1")
    );
}

#[tokio::test]
async fn page_content_save_rejects_group_nodes() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let workspace_id = current_workspace_id(&app, &cookie).await;
    let (group_status, group_payload) =
        create_group(&app, &cookie, &csrf, &workspace_id, Some("Group"), "a").await;
    assert_eq!(group_status, StatusCode::CREATED);
    let group_id = group_payload["data"]["id"].as_str().unwrap();

    let (save_status, _) = save_page_content(
        &app,
        &cookie,
        &csrf,
        &workspace_id,
        group_id,
        group_id,
        json!({ "version": 1 }),
    )
    .await;

    assert_eq!(save_status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn page_tabs_keep_documents_isolated_and_reject_last_tab_deletion() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let workspace_id = current_workspace_id(&app, &cookie).await;
    let (page_status, page_payload) = create_page(
        &app,
        &cookie,
        &csrf,
        &workspace_id,
        Some("Tabbed"),
        None,
        "a",
    )
    .await;
    assert_eq!(page_status, StatusCode::CREATED);
    let page_id = page_payload["data"]["id"].as_str().unwrap();
    let default_tab_id = page_payload["data"]["default_tab"]["id"].as_str().unwrap();
    assert_eq!(
        page_payload["data"]["default_tab"]["is_default"],
        json!(true)
    );

    let (last_delete_status, _) = send_json(
        &app,
        "DELETE",
        &format!("/api/console/frontstage/{workspace_id}/pages/{page_id}/tabs/{default_tab_id}"),
        &cookie,
        &csrf,
        json!({}),
    )
    .await;
    assert_eq!(last_delete_status, StatusCode::CONFLICT);

    let (presentation_status, _) = send_json(
        &app,
        "PATCH",
        &format!("/api/console/frontstage/{workspace_id}/pages/{page_id}"),
        &cookie,
        &csrf,
        json!({ "content_presentation": "tabs" }),
    )
    .await;
    assert_eq!(presentation_status, StatusCode::OK);

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
    let second_tab_id = tab_payload["data"]["id"].as_str().unwrap();
    let second_root_uid = tab_payload["data"]["document_root_uid"].as_str().unwrap();

    let second_document = json!({
        "version": 1,
        "root_uid": second_root_uid,
        "blocks": [{
            "id": "second",
            "codeRef": "second-code",
            "renderer_version": "v1"
        }]
    });
    let (save_status, _) = save_page_content(
        &app,
        &cookie,
        &csrf,
        &workspace_id,
        page_id,
        second_tab_id,
        second_document.clone(),
    )
    .await;
    assert_eq!(save_status, StatusCode::OK);

    let (_, default_detail) = get_json(
        &app,
        &format!("/api/console/frontstage/{workspace_id}/pages/{page_id}/tabs/{default_tab_id}"),
        &cookie,
    )
    .await;
    assert_eq!(
        default_detail["data"]["document"]["payload"]["blocks"],
        json!([])
    );
    let (_, second_detail) = get_json(
        &app,
        &format!("/api/console/frontstage/{workspace_id}/pages/{page_id}/tabs/{second_tab_id}"),
        &cookie,
    )
    .await;
    assert_eq!(
        second_detail["data"]["document"]["payload"],
        second_document
    );
}
