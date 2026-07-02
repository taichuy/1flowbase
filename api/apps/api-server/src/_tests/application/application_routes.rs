use crate::_tests::support::{login_and_capture_cookie, test_app};
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use serde_json::{json, Value};
use tower::ServiceExt;

async fn response_json(response: axum::response::Response) -> Value {
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    serde_json::from_slice(&body).unwrap()
}

#[tokio::test]
async fn application_routes_create_list_and_detail() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;

    let create = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/applications")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "application_type": "agent_flow",
                        "name": "Agent Support",
                        "description": "support app",
                        "icon": "RobotOutlined",
                        "icon_type": "iconfont",
                        "icon_background": "#E6F7F2"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create.status(), StatusCode::CREATED);

    let payload: Value =
        serde_json::from_slice(&to_bytes(create.into_body(), usize::MAX).await.unwrap()).unwrap();
    let application_id = payload["data"]["id"].as_str().unwrap();
    assert_eq!(
        payload["data"].get("workflow_trigger_type"),
        Some(&Value::Null)
    );

    let list = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/console/applications")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(list.status(), StatusCode::OK);

    let detail = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/console/applications/{application_id}"))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(detail.status(), StatusCode::OK);
    let detail_payload = response_json(detail).await;
    assert_eq!(
        detail_payload["data"].get("workflow_trigger_type"),
        Some(&Value::Null)
    );
}

#[tokio::test]
async fn application_routes_persist_workflow_trigger_type() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;

    let create = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/applications")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "application_type": "workflow",
                        "workflow_trigger_type": "manual",
                        "name": "Manual Workflow",
                        "description": "manual workflow",
                        "icon": "RobotOutlined",
                        "icon_type": "iconfont",
                        "icon_background": "#E6F7F2"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create.status(), StatusCode::CREATED);
    let create_payload = response_json(create).await;
    let application_id = create_payload["data"]["id"].as_str().unwrap();
    assert_eq!(
        create_payload["data"]["workflow_trigger_type"].as_str(),
        Some("manual")
    );

    let detail = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/console/applications/{application_id}"))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(detail.status(), StatusCode::OK);
    let detail_payload = response_json(detail).await;
    assert_eq!(
        detail_payload["data"]["workflow_trigger_type"].as_str(),
        Some("manual")
    );
}

#[tokio::test]
async fn application_routes_reject_invalid_workflow_trigger_type() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;

    let create = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/applications")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "application_type": "workflow",
                        "workflow_trigger_type": "webhook",
                        "name": "Invalid Workflow",
                        "description": "invalid workflow",
                        "icon": null,
                        "icon_type": null,
                        "icon_background": null
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(create.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn application_routes_support_catalog_tags_and_patching_metadata() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;

    let create = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/applications")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "application_type": "agent_flow",
                        "name": "Agent Support",
                        "description": "support app",
                        "icon": "RobotOutlined",
                        "icon_type": "iconfont",
                        "icon_background": "#E6F7F2"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create.status(), StatusCode::CREATED);
    let create_body = to_bytes(create.into_body(), usize::MAX).await.unwrap();
    let create_payload: Value = serde_json::from_slice(&create_body).unwrap();
    let application_id = create_payload["data"]["id"].as_str().unwrap().to_string();

    let catalog = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/console/applications/catalog")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(catalog.status(), StatusCode::OK);
    let catalog_body = to_bytes(catalog.into_body(), usize::MAX).await.unwrap();
    let catalog_payload: Value = serde_json::from_slice(&catalog_body).unwrap();
    assert_eq!(
        catalog_payload["data"]["types"].as_array().unwrap().len(),
        2
    );
    assert_eq!(catalog_payload["data"]["tags"].as_array().unwrap().len(), 0);

    let create_tag = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/applications/tags")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "客服"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_tag.status(), StatusCode::CREATED);
    let create_tag_body = to_bytes(create_tag.into_body(), usize::MAX).await.unwrap();
    let create_tag_payload: Value = serde_json::from_slice(&create_tag_body).unwrap();
    let tag_id = create_tag_payload["data"]["id"]
        .as_str()
        .unwrap()
        .to_string();

    let patch = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/api/console/applications/{application_id}"))
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "Agent Support Updated",
                        "description": "updated support app",
                        "tag_ids": [tag_id]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(patch.status(), StatusCode::OK);
    let patch_body = to_bytes(patch.into_body(), usize::MAX).await.unwrap();
    let patch_payload: Value = serde_json::from_slice(&patch_body).unwrap();
    assert_eq!(
        patch_payload["data"]["name"].as_str(),
        Some("Agent Support Updated")
    );
    assert_eq!(
        patch_payload["data"]["description"].as_str(),
        Some("updated support app")
    );
    assert_eq!(
        patch_payload["data"]["tags"][0]["name"].as_str(),
        Some("客服")
    );

    let list = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/console/applications")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(list.status(), StatusCode::OK);
    let list_body = to_bytes(list.into_body(), usize::MAX).await.unwrap();
    let list_payload: Value = serde_json::from_slice(&list_body).unwrap();
    assert_eq!(
        list_payload["data"][0]["name"].as_str(),
        Some("Agent Support Updated")
    );
    assert_eq!(
        list_payload["data"][0]["tags"][0]["name"].as_str(),
        Some("客服")
    );
}

#[tokio::test]
async fn application_routes_manage_plain_environment_variables() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;

    let create = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/applications")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "application_type": "agent_flow",
                        "name": "Agent Support",
                        "description": "support app",
                        "icon": "RobotOutlined",
                        "icon_type": "iconfont",
                        "icon_background": "#E6F7F2"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create.status(), StatusCode::CREATED);
    let create_body = to_bytes(create.into_body(), usize::MAX).await.unwrap();
    let create_payload: Value = serde_json::from_slice(&create_body).unwrap();
    let application_id = create_payload["data"]["id"].as_str().unwrap().to_string();

    let replace = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!(
                    "/api/console/applications/{application_id}/environment-variables"
                ))
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "variables": [
                            {
                                "name": "ApiBaseUrl",
                                "value_type": "string",
                                "value": "https://api.example.com",
                                "description": "当前应用 API 地址"
                            },
                            {
                                "name": "MaxRetry3",
                                "value_type": "number",
                                "value": 3,
                                "description": ""
                            }
                        ]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(replace.status(), StatusCode::OK);
    let replace_body = to_bytes(replace.into_body(), usize::MAX).await.unwrap();
    let replace_payload: Value = serde_json::from_slice(&replace_body).unwrap();
    assert_eq!(
        replace_payload["data"][0]["name"].as_str(),
        Some("ApiBaseUrl")
    );
    assert_eq!(
        replace_payload["data"][0]["value"].as_str(),
        Some("https://api.example.com")
    );
    assert_eq!(replace_payload["data"][1]["value"].as_i64(), Some(3));

    let list = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/applications/{application_id}/environment-variables"
                ))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(list.status(), StatusCode::OK);
    let list_body = to_bytes(list.into_body(), usize::MAX).await.unwrap();
    let list_payload: Value = serde_json::from_slice(&list_body).unwrap();
    assert_eq!(list_payload["data"].as_array().unwrap().len(), 2);

    let invalid = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!(
                    "/api/console/applications/{application_id}/environment-variables"
                ))
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "variables": [
                            {
                                "name": "API_KEY",
                                "value_type": "string",
                                "value": "not allowed",
                                "description": ""
                            }
                        ]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(invalid.status(), StatusCode::BAD_REQUEST);
}
