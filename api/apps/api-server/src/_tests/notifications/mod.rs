use crate::_tests::support::{
    create_member, get_json, login_and_capture_cookie, read_first_sse_frame, test_app,
};
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    Router,
};
use serde_json::{json, Value};
use tower::ServiceExt;

async fn request(
    app: &Router,
    cookie: &str,
    csrf: &str,
    method: &str,
    path: &str,
    body: Value,
) -> (StatusCode, Value) {
    let mut builder = Request::builder()
        .method(method)
        .uri(path)
        .header("cookie", cookie)
        .header("content-type", "application/json");
    if !csrf.is_empty() {
        builder = builder.header("x-csrf-token", csrf);
    }
    let response = app
        .clone()
        .oneshot(builder.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap())
}

#[tokio::test]
async fn notification_http_crud_send_inbox_and_operations_are_authorized() {
    let app = test_app().await;
    let (status, _) = request(
        &app,
        "",
        "",
        "GET",
        "/api/console/communications/types",
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let first_id = create_member(
        &app,
        &root_cookie,
        &root_csrf,
        "notification-first",
        "temp-pass",
    )
    .await;
    let second_id = create_member(
        &app,
        &root_cookie,
        &root_csrf,
        "notification-second",
        "temp-pass",
    )
    .await;
    let (first_cookie, first_csrf) =
        login_and_capture_cookie(&app, "notification-first", "temp-pass").await;
    let (second_cookie, second_csrf) =
        login_and_capture_cookie(&app, "notification-second", "temp-pass").await;
    let types = get_json(&app, "/api/console/communications/types", &first_cookie).await;
    assert_eq!(types["data"][0]["type_code"], "system");
    assert!(types["data"][0]["input_form"]["fields"][0]["options"]
        .as_array()
        .unwrap()
        .iter()
        .any(|option| option["value"] == first_id));
    for (method, path, body) in [
        ("GET", "/api/console/communications/instances", Value::Null),
        (
            "POST",
            "/api/console/communications/instances",
            json!({"type_code":"system","installation_id":null,"display_name":"denied","config_json":{}}),
        ),
    ] {
        let (status, error) = request(&app, &first_cookie, &first_csrf, method, path, body).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{error}");
    }
    let instances = get_json(&app, "/api/console/communications/instances", &root_cookie).await;
    assert_eq!(instances["data"].as_array().unwrap().len(), 1);
    let csrf_rejections = [
        ("", StatusCode::UNAUTHORIZED),
        ("incorrect-csrf", StatusCode::FORBIDDEN),
    ];
    for (csrf, expected_status) in csrf_rejections {
        let (status, denied) = request(
            &app,
            &root_cookie,
            csrf,
            "POST",
            "/api/console/communications/instances",
            json!({"type_code":"system","installation_id":null,"display_name":"CSRF rejected","config_json":{}}),
        )
        .await;
        assert_eq!(status, expected_status, "{denied}");
    }
    assert_eq!(
        get_json(&app, "/api/console/communications/instances", &root_cookie).await["data"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let (status, created) = request(&app, &root_cookie, &root_csrf, "POST", "/api/console/communications/instances", json!({"type_code":"system","installation_id":null,"display_name":"Workflow notices","config_json":{}})).await;
    assert_eq!(status, StatusCode::OK, "{created}");
    let id = created["data"]["id"].as_str().unwrap();
    let instance_path = format!("/api/console/communications/instances/{id}");
    let send_path = format!("{instance_path}/send");
    for (csrf, expected_status) in csrf_rejections {
        for (method, body) in [
            (
                "PATCH",
                json!({"display_name":"CSRF rejected","status":"disabled"}),
            ),
            ("DELETE", Value::Null),
        ] {
            let (status, denied) =
                request(&app, &root_cookie, csrf, method, &instance_path, body).await;
            assert_eq!(status, expected_status, "{denied}");
        }
    }
    let unchanged = get_json(&app, &instance_path, &root_cookie).await;
    assert_eq!(unchanged["data"]["display_name"], "Workflow notices");
    assert_eq!(unchanged["data"]["status"], "enabled");
    let input = json!({"input":{"user_ids":[first_id,second_id],"title":"Ready","content":"Stored before refresh"}});
    let (status, denied) = request(
        &app,
        &first_cookie,
        &first_csrf,
        "POST",
        &send_path,
        input.clone(),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{denied}");
    for (csrf, expected_status) in csrf_rejections {
        let (status, denied) =
            request(&app, &root_cookie, csrf, "POST", &send_path, input.clone()).await;
        assert_eq!(status, expected_status, "{denied}");
    }
    let (status, sent) = request(&app, &root_cookie, &root_csrf, "POST", &send_path, input).await;
    assert_eq!(status, StatusCode::OK, "{sent}");
    assert_eq!(sent["data"]["instance_id"], id);
    assert_eq!(sent["data"]["result"]["recipient_count"], 2);
    let first = get_json(
        &app,
        "/api/console/notifications/messages?page_size=1",
        &first_cookie,
    )
    .await;
    let second = get_json(&app, "/api/console/notifications/messages", &second_cookie).await;
    assert_eq!(first["data"]["unread_count"], 1);
    assert_eq!(first["data"]["items"][0]["recipient_user_id"], first_id);
    assert_eq!(second["data"]["items"][0]["recipient_user_id"], second_id);
    let read_path = format!(
        "/api/console/notifications/messages/{}/read",
        first["data"]["items"][0]["id"].as_str().unwrap()
    );
    for (csrf, expected_status) in csrf_rejections {
        let (status, denied) =
            request(&app, &first_cookie, csrf, "POST", &read_path, Value::Null).await;
        assert_eq!(status, expected_status, "{denied}");
        let (status, denied) = request(
            &app,
            &second_cookie,
            csrf,
            "POST",
            "/api/console/notifications/messages/read-all",
            Value::Null,
        )
        .await;
        assert_eq!(status, expected_status, "{denied}");
    }
    assert!(
        get_json(&app, "/api/console/notifications/messages", &first_cookie).await["data"]["items"]
            [0]["read_at"]
            .is_null()
    );
    assert!(
        get_json(&app, "/api/console/notifications/messages", &second_cookie).await["data"]
            ["items"][0]["read_at"]
            .is_null()
    );
    let (status, denied) = request(
        &app,
        &second_cookie,
        &second_csrf,
        "POST",
        &read_path,
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{denied}");
    let (status, read) = request(
        &app,
        &first_cookie,
        &first_csrf,
        "POST",
        &read_path,
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{read}");
    assert!(read["data"]["read_at"].is_string());
    assert_eq!(
        get_json(
            &app,
            "/api/console/notifications/messages/unread-count",
            &first_cookie
        )
        .await["data"]["unread_count"],
        0
    );
    assert_eq!(
        get_json(
            &app,
            "/api/console/notifications/messages/unread-count",
            &second_cookie
        )
        .await["data"]["unread_count"],
        1
    );
    let (status, all) = request(
        &app,
        &second_cookie,
        &second_csrf,
        "POST",
        "/api/console/notifications/messages/read-all",
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{all}");
    assert_eq!(all["data"]["updated_count"], 1);
    let (status, patched) = request(
        &app,
        &root_cookie,
        &root_csrf,
        "PATCH",
        &instance_path,
        json!({"display_name":"Saved notice","status":"disabled"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{patched}");
    assert_eq!(
        get_json(&app, &instance_path, &root_cookie).await["data"]["display_name"],
        "Saved notice"
    );
    let (status, unavailable) = request(
        &app,
        &root_cookie,
        &root_csrf,
        "POST",
        &send_path,
        json!({"input":{"user_ids":[first_id],"title":"disabled","content":"no send"}}),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{unavailable}");
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/console/notifications/events")
                .header("cookie", &first_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert!(read_first_sse_frame(response)
        .await
        .contains("event: notification.changed"));
    let (status, deleted) = request(
        &app,
        &root_cookie,
        &root_csrf,
        "DELETE",
        &instance_path,
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{deleted}");
    let retained = get_json(&app, "/api/console/notifications/messages", &first_cookie).await;
    assert_eq!(retained["data"]["items"][0]["instance_id"], id);
    assert!(retained["data"]["items"][0]["read_at"].is_string());
}
