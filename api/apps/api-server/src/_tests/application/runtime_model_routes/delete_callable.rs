use super::*;

// Same runtime CRUD handler through the Frontstage ctx.api protocol projection.
#[tokio::test]
async fn runtime_delete_callable_preserves_json_result_and_committed_effect() {
    let (app, database_url) = test_app_with_database_url().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let model_id = create_orders_model(&app, &cookie, &csrf).await;
    create_text_field(&app, &cookie, &csrf, &model_id, "title").await;
    let request = |method: &str, path: &str, body: serde_json::Value, token: &str| {
        Request::builder()
            .method(method)
            .uri(path)
            .header("cookie", &cookie)
            .header("x-csrf-token", token)
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap()
    };
    let response = app
        .clone()
        .oneshot(request(
            "POST",
            "/api/runtime/models/orders/create",
            json!({"title":"delete target"}),
            &csrf,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let created: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
    let id = created["data"]["id"].as_str().unwrap();
    let response = app
        .clone()
        .oneshot(request(
            "POST",
            "/api/console/frontstage/pages",
            json!({"title":"Delete test","rank":"a","placement":"topbar","slug":"delete-test"}),
            &csrf,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let page: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
    let page_id = page["data"]["page"]["id"].as_str().unwrap();
    let tab_id = page["data"]["default_tab"]["id"].as_str().unwrap();
    let response = app.clone().oneshot(request("POST", &format!("/api/console/frontstage/pages/{page_id}/blocks"), json!({"tab_id":tab_id,"title":"Delete","presentation":"inline","source_code":"export default function Block() { return null; }"}), &csrf)).await.unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let block: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
    let path = format!(
        "/api/console/frontstage/pages/{page_id}/tabs/{tab_id}/callable-interfaces/dispatch"
    );
    let invocation = json!({"block_id":block["data"]["block_id"],"method":"DELETE","path":"/api/runtime/models/orders/delete/{id}","request":{"path":{"id":id}}});
    let denied = app
        .clone()
        .oneshot(request("POST", &path, invocation.clone(), ""))
        .await
        .unwrap();
    assert_eq!(denied.status(), StatusCode::FORBIDDEN);
    let get_path = format!("/api/runtime/models/orders/get/{id}");
    assert_eq!(
        app.clone()
            .oneshot(request("GET", &get_path, json!(null), &csrf))
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    let response = app
        .clone()
        .oneshot(request("POST", &path, invocation, &csrf))
        .await
        .unwrap();
    let status = response.status();
    let payload: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(status, StatusCode::OK, "{payload}");
    assert_eq!(payload["data"], json!({"deleted":true}));
    assert_eq!(
        app.clone()
            .oneshot(request("GET", &get_path, json!(null), &csrf))
            .await
            .unwrap()
            .status(),
        StatusCode::NOT_FOUND
    );
    drop_runtime_table(&database_url, &model_id).await;
}
