use crate::_tests::support::{
    create_member, create_role, login_and_capture_cookie, replace_member_roles,
    replace_role_permissions, seed_workspace, test_app, test_app_with_database_url,
};
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use serde_json::json;
use serde_json::Value;
use tower::ServiceExt;

async fn current_workspace_id(app: &axum::Router, cookie: &str) -> String {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/console/session")
                .header("cookie", cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let payload: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();

    payload["data"]["session"]["current_workspace_id"]
        .as_str()
        .unwrap()
        .to_string()
}

async fn create_group(
    app: &axum::Router,
    cookie: &str,
    csrf: &str,
    workspace_id: &str,
    title: Option<&str>,
    rank: &str,
) -> (StatusCode, Value) {
    send_json(
        app,
        "POST",
        &format!("/api/console/frontstage/{workspace_id}/pages/groups"),
        cookie,
        csrf,
        json!({
            "title": title,
            "rank": rank
        }),
    )
    .await
}

async fn create_page(
    app: &axum::Router,
    cookie: &str,
    csrf: &str,
    workspace_id: &str,
    title: Option<&str>,
    parent_id: Option<&str>,
    rank: &str,
) -> (StatusCode, Value) {
    send_json(
        app,
        "POST",
        &format!("/api/console/frontstage/{workspace_id}/pages"),
        cookie,
        csrf,
        json!({
            "title": title,
            "parent_id": parent_id,
            "rank": rank
        }),
    )
    .await
}

async fn send_json(
    app: &axum::Router,
    method: &str,
    path: &str,
    cookie: &str,
    csrf: &str,
    body: Value,
) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(method)
                .uri(path)
                .header("cookie", cookie)
                .header("x-csrf-token", csrf)
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let payload: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap())
            .unwrap_or_else(|_| json!({}));
    (status, payload)
}

async fn delete_node(
    app: &axum::Router,
    cookie: &str,
    csrf: &str,
    workspace_id: &str,
    page_id: &str,
) -> StatusCode {
    app.clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!(
                    "/api/console/frontstage/{workspace_id}/pages/{page_id}"
                ))
                .header("cookie", cookie)
                .header("x-csrf-token", csrf)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap()
        .status()
}

async fn get_json(app: &axum::Router, path: &str, cookie: &str) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(path)
                .header("cookie", cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let payload: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap())
            .unwrap_or_else(|_| json!({}));
    (status, payload)
}

async fn save_page_content(
    app: &axum::Router,
    cookie: &str,
    csrf: &str,
    workspace_id: &str,
    page_id: &str,
    schema_payload: Value,
    root_payload: Value,
) -> (StatusCode, Value) {
    send_json(
        app,
        "PUT",
        &format!("/api/console/frontstage/{workspace_id}/pages/{page_id}/content"),
        cookie,
        csrf,
        json!({
            "schema": {
                "payload": schema_payload
            },
            "root": {
                "payload": root_payload
            }
        }),
    )
    .await
}

#[tokio::test]
async fn list_frontstage_pages_route_returns_empty_tree_for_accessible_workspace() {
    let app = test_app().await;
    let (cookie, _) = login_and_capture_cookie(&app, "root", "change-me").await;
    let workspace_id = current_workspace_id(&app, &cookie).await;

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
    let pages = payload["data"]
        .as_array()
        .expect("frontstage pages should return array");
    assert!(pages.is_empty());
}

#[tokio::test]
async fn list_frontstage_pages_route_rejects_inaccessible_workspace() {
    let (app, database_url) = test_app_with_database_url().await;
    let no_access_workspace_id = seed_workspace(&database_url, "No Access Workspace").await;
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;

    create_member(
        &app,
        &root_cookie,
        &root_csrf,
        "frontstage-visitor",
        "temp-pass",
    )
    .await;

    let (visitor_cookie, _) =
        login_and_capture_cookie(&app, "frontstage-visitor", "temp-pass").await;

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!(
                    "/api/console/frontstage/{no_access_workspace_id}/pages"
                ))
                .header("cookie", &visitor_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn list_frontstage_pages_route_rejects_invalid_workspace_id() {
    let app = test_app().await;
    let (cookie, _) = login_and_capture_cookie(&app, "root", "change-me").await;

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/console/frontstage/not-a-uuid/pages")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn list_frontstage_pages_route_requires_session() {
    let app = test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/console/frontstage/00000000-0000-0000-0000-000000000001/pages")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn root_can_create_group_and_page() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let workspace_id = current_workspace_id(&app, &cookie).await;

    let (group_status, group_payload) =
        create_group(&app, &cookie, &csrf, &workspace_id, Some("Landing"), "a").await;
    assert_eq!(group_status, StatusCode::CREATED);
    let group_id = group_payload["data"]["id"].as_str().unwrap();

    let (page_status, page_payload) = create_page(
        &app,
        &cookie,
        &csrf,
        &workspace_id,
        Some("Home"),
        Some(group_id),
        "a",
    )
    .await;
    assert_eq!(page_status, StatusCode::CREATED);
    assert_eq!(page_payload["data"]["kind"], json!("page"));

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
    assert_eq!(payload["data"][0]["id"], json!(group_id));
    assert_eq!(payload["data"][0]["children"][0]["title"], json!("Home"));
}

#[tokio::test]
async fn manager_can_create_group_and_page() {
    let app = test_app().await;
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    create_member(
        &app,
        &root_cookie,
        &root_csrf,
        "frontstage-manager",
        "temp-pass",
    )
    .await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "frontstage-manager", "temp-pass").await;
    let workspace_id = current_workspace_id(&app, &cookie).await;

    let (group_status, _) =
        create_group(&app, &cookie, &csrf, &workspace_id, Some("Group"), "a").await;
    assert_eq!(group_status, StatusCode::CREATED);

    let (page_status, _) =
        create_page(&app, &cookie, &csrf, &workspace_id, Some("Page"), None, "b").await;
    assert_eq!(page_status, StatusCode::CREATED);
}

#[tokio::test]
async fn workspace_member_without_design_permission_cannot_write() {
    let app = test_app().await;
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let member_id = create_member(
        &app,
        &root_cookie,
        &root_csrf,
        "frontstage-viewer",
        "temp-pass",
    )
    .await;
    create_role(&app, &root_cookie, &root_csrf, "frontstage_viewer").await;
    replace_role_permissions(&app, &root_cookie, &root_csrf, "frontstage_viewer", &[]).await;
    replace_member_roles(
        &app,
        &root_cookie,
        &root_csrf,
        &member_id,
        &["frontstage_viewer"],
    )
    .await;

    let (cookie, csrf) = login_and_capture_cookie(&app, "frontstage-viewer", "temp-pass").await;
    let workspace_id = current_workspace_id(&app, &cookie).await;
    let (status, _) = create_group(&app, &cookie, &csrf, &workspace_id, Some("Group"), "a").await;

    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn rename_allows_empty_title() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let workspace_id = current_workspace_id(&app, &cookie).await;
    let (status, payload) = create_page(
        &app,
        &cookie,
        &csrf,
        &workspace_id,
        Some("Named"),
        None,
        "a",
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let page_id = payload["data"]["id"].as_str().unwrap();

    let (rename_status, rename_payload) = send_json(
        &app,
        "PATCH",
        &format!("/api/console/frontstage/{workspace_id}/pages/{page_id}"),
        &cookie,
        &csrf,
        json!({ "title": "" }),
    )
    .await;

    assert_eq!(rename_status, StatusCode::OK);
    assert_eq!(rename_payload["data"]["title"], json!(""));
}

#[tokio::test]
async fn patch_page_metadata_persists_tooltip_and_hidden_state() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let workspace_id = current_workspace_id(&app, &cookie).await;
    let (status, payload) = create_page(
        &app,
        &cookie,
        &csrf,
        &workspace_id,
        Some("Named"),
        None,
        "a",
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let page_id = payload["data"]["id"].as_str().unwrap();

    let (patch_status, patch_payload) = send_json(
        &app,
        "PATCH",
        &format!("/api/console/frontstage/{workspace_id}/pages/{page_id}"),
        &cookie,
        &csrf,
        json!({
            "icon": "FileTextOutlined",
            "tooltip": "展示在页面树",
            "is_hidden": true
        }),
    )
    .await;

    assert_eq!(patch_status, StatusCode::OK);
    assert_eq!(patch_payload["data"]["title"], json!("Named"));
    assert_eq!(patch_payload["data"]["icon"], json!("FileTextOutlined"));
    assert_eq!(patch_payload["data"]["tooltip"], json!("展示在页面树"));
    assert_eq!(patch_payload["data"]["is_hidden"], json!(true));

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
    assert_eq!(payload["data"][0]["title"], json!("Named"));
    assert_eq!(payload["data"][0]["icon"], json!("FileTextOutlined"));
    assert_eq!(payload["data"][0]["tooltip"], json!("展示在页面树"));
    assert_eq!(payload["data"][0]["is_hidden"], json!(true));
}

#[tokio::test]
async fn group_under_group_is_rejected() {
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

    assert_eq!(nested_status, StatusCode::BAD_REQUEST);
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
    let schema_root_uid = page_payload["data"]["schema_root_uid"].as_str().unwrap();

    let (detail_status, detail_payload) = get_json(
        &app,
        &format!("/api/console/frontstage/{workspace_id}/pages/{page_id}"),
        &cookie,
    )
    .await;
    assert_eq!(detail_status, StatusCode::OK);
    assert_eq!(detail_payload["data"]["page"]["id"], json!(page_id));
    assert_eq!(
        detail_payload["data"]["schema"]["root_uid"],
        json!(schema_root_uid)
    );
    assert_eq!(
        detail_payload["data"]["root"]["uid"],
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

    let (overwrite_status, _) = send_json(
        &app,
        "PUT",
        &code_path,
        &cookie,
        &csrf,
        json!({ "code": "export default function Hero() { return 'v2'; }" }),
    )
    .await;
    assert_eq!(overwrite_status, StatusCode::OK);

    let (read_status, read_payload) = get_json(&app, &code_path, &cookie).await;
    assert_eq!(read_status, StatusCode::OK);
    assert_eq!(read_payload["data"]["code_ref"], json!("hero"));
    assert_eq!(
        read_payload["data"]["code"],
        json!("export default function Hero() { return 'v2'; }")
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
    let schema_root_uid = page_payload["data"]["schema_root_uid"].as_str().unwrap();

    let schema_payload = json!({
        "version": 1,
        "root_uid": schema_root_uid,
        "nodes": [
            {
                "uid": "hero-1",
                "type": "official.hero"
            }
        ]
    });
    let root_payload = json!({
        "uid": schema_root_uid,
        "kind": "frontstage.page.root",
        "children": ["hero-1"],
        "x-layout": {
            "columns": 12
        }
    });

    let (save_status, save_payload) = save_page_content(
        &app,
        &cookie,
        &csrf,
        &workspace_id,
        page_id,
        schema_payload.clone(),
        root_payload.clone(),
    )
    .await;
    assert_eq!(save_status, StatusCode::OK);
    assert_eq!(save_payload["data"]["page"]["id"], json!(page_id));
    assert_eq!(
        save_payload["data"]["schema"]["payload"],
        schema_payload.clone()
    );
    assert_eq!(
        save_payload["data"]["root"]["payload"],
        root_payload.clone()
    );

    let (detail_status, detail_payload) = get_json(
        &app,
        &format!("/api/console/frontstage/{workspace_id}/pages/{page_id}"),
        &cookie,
    )
    .await;
    assert_eq!(detail_status, StatusCode::OK);
    assert_eq!(
        detail_payload["data"]["schema"]["payload"],
        schema_payload.clone()
    );
    assert_eq!(
        detail_payload["data"]["root"]["payload"],
        root_payload.clone()
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
    let (sibling_detail_status, sibling_detail_payload) = get_json(
        &app,
        &format!("/api/console/frontstage/{workspace_id}/pages/{sibling_page_id}"),
        &cookie,
    )
    .await;
    assert_eq!(sibling_detail_status, StatusCode::OK);
    assert_eq!(
        sibling_detail_payload["data"]["schema"]["payload"]["nodes"],
        json!([])
    );

    let (_, other_page_payload) = create_page(
        &app,
        &cookie,
        &csrf,
        &other_workspace_id.to_string(),
        Some("Other Workspace Page"),
        None,
        "a",
    )
    .await;
    let other_page_id = other_page_payload["data"]["id"].as_str().unwrap();
    let (cross_workspace_status, _) = save_page_content(
        &app,
        &cookie,
        &csrf,
        &workspace_id,
        other_page_id,
        json!({ "version": 1, "nodes": [] }),
        json!({ "children": [] }),
    )
    .await;
    assert_eq!(cross_workspace_status, StatusCode::NOT_FOUND);
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
        json!({ "version": 1 }),
        json!({ "children": [] }),
    )
    .await;

    assert_eq!(save_status, StatusCode::NOT_FOUND);
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
        &format!("/api/console/frontstage/{workspace_id}/pages/{page_id}"),
        &viewer_cookie,
    )
    .await;
    assert_eq!(detail_status, StatusCode::FORBIDDEN);

    let (read_status, _) = get_json(&app, &code_path, &viewer_cookie).await;
    assert_eq!(read_status, StatusCode::FORBIDDEN);

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
        json!({ "version": 1, "nodes": [] }),
        json!({ "children": [] }),
    )
    .await;
    assert_eq!(content_write_status, StatusCode::OK);
}
