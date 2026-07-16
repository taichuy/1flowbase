use crate::_tests::support::{login_and_capture_cookie, test_app};
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
