use crate::_tests::support::{
    create_member, create_role, login_and_capture_cookie, replace_member_roles,
    replace_role_permissions, test_app,
};
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use serde_json::{json, Value};
use tower::ServiceExt;

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
            .unwrap_or(Value::Null);
    (status, payload)
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
            .unwrap_or(Value::Null);
    (status, payload)
}

async fn current_workspace_id(app: &axum::Router, cookie: &str) -> String {
    let (status, payload) = get_json(app, "/api/console/session", cookie).await;
    assert_eq!(status, StatusCode::OK);
    payload["data"]["session"]["current_workspace_id"]
        .as_str()
        .unwrap()
        .to_string()
}

async fn create_page(
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
        json!({ "title": "Data capability page", "rank": "a" }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    (
        payload["data"]["id"].as_str().unwrap().to_string(),
        payload["data"]["default_tab"]["id"]
            .as_str()
            .unwrap()
            .to_string(),
    )
}

async fn create_published_model(
    app: &axum::Router,
    cookie: &str,
    csrf: &str,
    code: &str,
) -> String {
    let (status, payload) = send_json(
        app,
        "POST",
        "/api/console/settings/data-models/model-definitions",
        cookie,
        csrf,
        json!({ "scope_kind": "workspace", "code": code, "title": code }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let model_id = payload["data"]["id"].as_str().unwrap().to_string();

    let (field_status, _) = send_json(
        app,
        "POST",
        &format!("/api/console/settings/data-models/model-definitions/{model_id}/fields"),
        cookie,
        csrf,
        json!({
            "code": "title",
            "title": "title",
            "field_kind": "text",
            "is_required": true,
            "is_unique": false,
            "display_options": {}
        }),
    )
    .await;
    assert_eq!(field_status, StatusCode::CREATED);
    model_id
}

#[allow(
    clippy::too_many_arguments,
    reason = "the integration-test helper mirrors the route identity and request fields explicitly"
)]
async fn dispatch(
    app: &axum::Router,
    cookie: &str,
    csrf: &str,
    workspace_id: &str,
    page_id: &str,
    tab_id: &str,
    kind: &str,
    id_field: &str,
    capability_id: &str,
    params: Value,
) -> (StatusCode, Value) {
    send_json(
        app,
        "POST",
        &format!(
            "/api/console/frontstage/{workspace_id}/pages/{page_id}/tabs/{tab_id}/{kind}/dispatch"
        ),
        cookie,
        csrf,
        json!({ id_field: capability_id, "params": params }),
    )
    .await
}

async fn callable_catalog(
    app: &axum::Router,
    cookie: &str,
    workspace_id: &str,
    path_query: &str,
) -> Value {
    let (status, payload) = get_json(
        app,
        &format!(
            "/api/console/frontstage/{workspace_id}/interface-capabilities?path_query={path_query}"
        ),
        cookie,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{payload}");
    payload["data"]["items"].clone()
}

async fn callable_detail(
    app: &axum::Router,
    cookie: &str,
    workspace_id: &str,
    interface_id: &str,
) -> Value {
    let (status, payload) = get_json(
        app,
        &format!("/api/console/frontstage/{workspace_id}/interface-capabilities/{interface_id}"),
        cookie,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{payload}");
    payload["data"].clone()
}

#[tokio::test]
async fn callable_catalog_exposes_runtime_model_crud_and_keeps_filter_string() {
    // Root AC-013: the original Runtime route remains the authorization owner.
    let app = test_app().await;
    let (cookie, _) = login_and_capture_cookie(&app, "root", "change-me").await;
    let workspace_id = current_workspace_id(&app, &cookie).await;
    let entries = callable_catalog(&app, &cookie, &workspace_id, "application_conversations").await;
    let conversations = entries
        .as_array()
        .unwrap()
        .iter()
        .filter(|entry| {
            entry["path"]
                .as_str()
                .is_some_and(|path| path.contains("/application_conversations/"))
        })
        .collect::<Vec<_>>();
    assert_eq!(conversations.len(), 5, "{entries}");
    assert!(conversations
        .iter()
        .any(|entry| entry["path"].as_str().unwrap().ends_with("/list")));
    assert!(conversations
        .iter()
        .any(|entry| entry["path"].as_str().unwrap().contains("/get/{id}")));
    let list = conversations
        .iter()
        .find(|entry| entry["path"].as_str().unwrap().ends_with("/list"))
        .unwrap();
    let list = callable_detail(
        &app,
        &cookie,
        &workspace_id,
        list["interface_id"].as_str().unwrap(),
    )
    .await;
    assert_eq!(
        list["parameter_schema"]["properties"]["query"]["properties"]["filter"]["type"],
        json!("string")
    );
}

#[tokio::test]
async fn callable_catalog_requires_frontstage_design_permission() {
    let app = test_app().await;
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let workspace_id = current_workspace_id(&app, &root_cookie).await;
    let member_id = create_member(
        &app,
        &root_cookie,
        &root_csrf,
        "callable-viewer",
        "temp-pass",
    )
    .await;
    create_role(&app, &root_cookie, &root_csrf, "callable_viewer").await;
    replace_role_permissions(&app, &root_cookie, &root_csrf, "callable_viewer", &[]).await;
    replace_member_roles(
        &app,
        &root_cookie,
        &root_csrf,
        &member_id,
        &["callable_viewer"],
    )
    .await;

    let (cookie, _) = login_and_capture_cookie(&app, "callable-viewer", "temp-pass").await;
    let (status, _) = get_json(
        &app,
        &format!("/api/console/frontstage/{workspace_id}/interface-capabilities"),
        &cookie,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn callable_catalog_filters_one_or_many_path_prefixes_before_pagination() {
    // #1444 AC-012/013: no scope means all callable interfaces; scoped queries use backend OR filtering.
    let app = test_app().await;
    let (cookie, _) = login_and_capture_cookie(&app, "root", "change-me").await;
    let workspace_id = current_workspace_id(&app, &cookie).await;

    let (all_status, all) = get_json(
        &app,
        &format!("/api/console/frontstage/{workspace_id}/interface-capabilities?limit=20"),
        &cookie,
    )
    .await;
    assert_eq!(all_status, StatusCode::OK, "{all}");

    let (public_status, public) = get_json(
        &app,
        &format!(
            "/api/console/frontstage/{workspace_id}/interface-capabilities?path_prefixes=%2Fapi%2Fpublic%2F&limit=2"
        ),
        &cookie,
    )
    .await;
    assert_eq!(public_status, StatusCode::OK, "{public}");
    let public_items = public["data"]["items"].as_array().unwrap();
    assert_eq!(public_items.len(), 2, "{public}");
    assert!(public_items.iter().all(|entry| entry["path"]
        .as_str()
        .is_some_and(|path| path.starts_with("/api/public/"))));
    let public_detail = callable_detail(
        &app,
        &cookie,
        &workspace_id,
        public_items[0]["interface_id"].as_str().unwrap(),
    )
    .await;
    assert!(public_detail["path"]
        .as_str()
        .is_some_and(|path| path.starts_with("/api/public/")));
    assert!(public["data"]["total"].as_u64().unwrap() > 2, "{public}");
    assert_eq!(public["data"]["has_more"], json!(true));
    assert_eq!(public["data"]["next_offset"], json!(2));
    assert!(all["data"]["total"].as_u64().unwrap() > public["data"]["total"].as_u64().unwrap());

    let prefixes = "%2Fapi%2Fpublic%2F%2C%2Fapi%2Fconsole%2Fsettings%2Fauth-center%2F";
    let (many_status, many) = get_json(
        &app,
        &format!(
            "/api/console/frontstage/{workspace_id}/interface-capabilities?path_prefixes={prefixes}&limit=20"
        ),
        &cookie,
    )
    .await;
    assert_eq!(many_status, StatusCode::OK, "{many}");
    let many_items = many["data"]["items"].as_array().unwrap();
    assert!(many_items.iter().any(|entry| entry["path"]
        .as_str()
        .is_some_and(|path| path.starts_with("/api/public/"))));
    assert!(many_items.iter().any(|entry| entry["path"]
        .as_str()
        .is_some_and(|path| path.starts_with("/api/console/settings/auth-center/"))));
    assert!(many_items.iter().all(|entry| entry["path"]
        .as_str()
        .is_some_and(|path| path.starts_with("/api/public/")
            || path.starts_with("/api/console/settings/auth-center/"))));

    let (invalid_status, _) = get_json(
        &app,
        &format!(
            "/api/console/frontstage/{workspace_id}/interface-capabilities?path_prefixes=api%2Fpublic"
        ),
        &cookie,
    )
    .await;
    assert_eq!(invalid_status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn data_capability_catalog_lists_descriptors_and_published_models() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let workspace_id = current_workspace_id(&app, &cookie).await;
    create_published_model(&app, &cookie, &csrf, "cap_orders").await;

    let (status, payload) = get_json(
        &app,
        &format!("/api/console/frontstage/{workspace_id}/data-capabilities"),
        &cookie,
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let queries: Vec<&str> = payload["data"]["queries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["id"].as_str().unwrap())
        .collect();
    assert!(queries.contains(&"frontstage.data_model.record.list"));
    assert!(queries.contains(&"frontstage.data_model.record.get"));

    let actions: Vec<&str> = payload["data"]["actions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["id"].as_str().unwrap())
        .collect();
    assert!(actions.contains(&"frontstage.data_model.record.create"));
    assert!(actions.contains(&"frontstage.data_model.record.update"));
    assert!(actions.contains(&"frontstage.data_model.record.delete"));

    let model = payload["data"]["models"]
        .as_array()
        .unwrap()
        .iter()
        .find(|model| model["code"] == "cap_orders")
        .expect("published model should be listed");
    let field_codes: Vec<&str> = model["fields"]
        .as_array()
        .unwrap()
        .iter()
        .map(|field| field["code"].as_str().unwrap())
        .collect();
    assert!(field_codes.contains(&"title"));

    for query in payload["data"]["queries"].as_array().unwrap() {
        assert!(query["params_schema"]["properties"]["model"].is_object());
        assert!(query["result_schema"].is_object());
    }
}

#[tokio::test]
async fn data_capability_catalog_requires_session() {
    let app = test_app().await;
    let (cookie, _) = login_and_capture_cookie(&app, "root", "change-me").await;
    let workspace_id = current_workspace_id(&app, &cookie).await;
    let (status, _) = get_json(
        &app,
        &format!("/api/console/frontstage/{workspace_id}/data-capabilities"),
        "cookie=missing",
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn data_capability_dispatch_round_trips_record_crud() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let workspace_id = current_workspace_id(&app, &cookie).await;
    let (page_id, tab_id) = create_page(&app, &cookie, &csrf, &workspace_id).await;
    create_published_model(&app, &cookie, &csrf, "cap_tasks").await;

    let (create_status, create_payload) = dispatch(
        &app,
        &cookie,
        &csrf,
        &workspace_id,
        &page_id,
        &tab_id,
        "actions",
        "action_id",
        "frontstage.data_model.record.create",
        json!({ "model": "cap_tasks", "values": { "title": "First task" } }),
    )
    .await;
    assert_eq!(create_status, StatusCode::OK, "{create_payload}");
    let record_id = create_payload["data"]["record"]["id"]
        .as_str()
        .unwrap()
        .to_string();

    let (list_status, list_payload) = dispatch(
        &app,
        &cookie,
        &csrf,
        &workspace_id,
        &page_id,
        &tab_id,
        "queries",
        "query_id",
        "frontstage.data_model.record.list",
        json!({
            "model": "cap_tasks",
            "filter": { "title": { "$eq": "First task" } },
            "sort": { "field": "title", "direction": "asc" },
            "page": 1,
            "page_size": 10
        }),
    )
    .await;
    assert_eq!(list_status, StatusCode::OK, "{list_payload}");
    assert_eq!(list_payload["data"]["total"], json!(1));
    assert_eq!(
        list_payload["data"]["items"][0]["title"],
        json!("First task")
    );

    let (get_status, get_payload) = dispatch(
        &app,
        &cookie,
        &csrf,
        &workspace_id,
        &page_id,
        &tab_id,
        "queries",
        "query_id",
        "frontstage.data_model.record.get",
        json!({ "model": "cap_tasks", "record_id": record_id }),
    )
    .await;
    assert_eq!(get_status, StatusCode::OK, "{get_payload}");
    assert_eq!(get_payload["data"]["record"]["title"], json!("First task"));

    let (update_status, update_payload) = dispatch(
        &app,
        &cookie,
        &csrf,
        &workspace_id,
        &page_id,
        &tab_id,
        "actions",
        "action_id",
        "frontstage.data_model.record.update",
        json!({
            "model": "cap_tasks",
            "record_id": record_id,
            "values": { "title": "Renamed task" }
        }),
    )
    .await;
    assert_eq!(update_status, StatusCode::OK, "{update_payload}");
    assert_eq!(
        update_payload["data"]["record"]["title"],
        json!("Renamed task")
    );

    let (delete_status, delete_payload) = dispatch(
        &app,
        &cookie,
        &csrf,
        &workspace_id,
        &page_id,
        &tab_id,
        "actions",
        "action_id",
        "frontstage.data_model.record.delete",
        json!({ "model": "cap_tasks", "record_id": record_id }),
    )
    .await;
    assert_eq!(delete_status, StatusCode::OK, "{delete_payload}");

    let (empty_status, empty_payload) = dispatch(
        &app,
        &cookie,
        &csrf,
        &workspace_id,
        &page_id,
        &tab_id,
        "queries",
        "query_id",
        "frontstage.data_model.record.list",
        json!({ "model": "cap_tasks" }),
    )
    .await;
    assert_eq!(empty_status, StatusCode::OK);
    assert_eq!(empty_payload["data"]["total"], json!(0));
}

#[tokio::test]
async fn data_capability_dispatch_rejects_bad_params_and_unknown_model() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let workspace_id = current_workspace_id(&app, &cookie).await;
    let (page_id, tab_id) = create_page(&app, &cookie, &csrf, &workspace_id).await;

    let (missing_model_status, missing_model_payload) = dispatch(
        &app,
        &cookie,
        &csrf,
        &workspace_id,
        &page_id,
        &tab_id,
        "queries",
        "query_id",
        "frontstage.data_model.record.list",
        json!({}),
    )
    .await;
    assert_eq!(missing_model_status, StatusCode::BAD_REQUEST);
    assert_eq!(
        missing_model_payload["code"],
        json!("frontstage_data_capability_params")
    );

    let (unknown_model_status, unknown_model_payload) = dispatch(
        &app,
        &cookie,
        &csrf,
        &workspace_id,
        &page_id,
        &tab_id,
        "queries",
        "query_id",
        "frontstage.data_model.record.list",
        json!({ "model": "missing_model" }),
    )
    .await;
    assert_eq!(unknown_model_status, StatusCode::CONFLICT);
    assert_eq!(
        unknown_model_payload["code"],
        json!("runtime_model_unavailable")
    );

    let (missing_values_status, missing_values_payload) = dispatch(
        &app,
        &cookie,
        &csrf,
        &workspace_id,
        &page_id,
        &tab_id,
        "actions",
        "action_id",
        "frontstage.data_model.record.create",
        json!({ "model": "missing_model" }),
    )
    .await;
    assert_eq!(missing_values_status, StatusCode::BAD_REQUEST);
    assert_eq!(missing_values_payload["code"], json!("values"));

    let (missing_record_status, missing_record_payload) = dispatch(
        &app,
        &cookie,
        &csrf,
        &workspace_id,
        &page_id,
        &tab_id,
        "queries",
        "query_id",
        "frontstage.data_model.record.get",
        json!({ "model": "missing_model" }),
    )
    .await;
    assert_eq!(missing_record_status, StatusCode::BAD_REQUEST);
    assert_eq!(missing_record_payload["code"], json!("record_id"));
}

#[tokio::test]
async fn data_capability_dispatch_requires_tab_scope() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let workspace_id = current_workspace_id(&app, &cookie).await;
    let (page_id, _tab_id) = create_page(&app, &cookie, &csrf, &workspace_id).await;
    create_published_model(&app, &cookie, &csrf, "cap_scope").await;

    let missing_tab = uuid::Uuid::new_v4().to_string();
    let (status, payload) = dispatch(
        &app,
        &cookie,
        &csrf,
        &workspace_id,
        &page_id,
        &missing_tab,
        "queries",
        "query_id",
        "frontstage.data_model.record.list",
        json!({ "model": "cap_scope" }),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{payload}");
}
