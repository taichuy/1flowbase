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
    // AC-001: AgentFlow exposes its Application API Key route family.
    assert_eq!(
        detail_payload["data"]["sections"]["api"]["credential_kind"],
        json!("application_api_key")
    );
    assert_eq!(
        detail_payload["data"]["sections"]["api"]["invoke_path_template"],
        json!("/api/agent/v1/runs")
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
                        "workflow_trigger_type": "schedule",
                        "name": "Scheduled Workflow",
                        "description": "scheduled workflow",
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
        Some("schedule")
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
        Some("schedule")
    );
    // AC-001: schedules do not advertise an HTTP API capability.
    assert_eq!(
        detail_payload["data"]["sections"]["api"]["status"],
        json!("unavailable")
    );
    assert_eq!(
        detail_payload["data"]["sections"]["api"]["invoke_path_template"],
        Value::Null
    );
}

#[tokio::test]
async fn application_routes_create_schedule_trigger_disabled_by_default() {
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
                        "workflow_trigger_type": "schedule",
                        "workflow_trigger_config": {
                            "cron": "0 9 * * 1-5",
                            "timezone": "Asia/Shanghai",
                            "input_payload": {"report": "daily"}
                        },
                        "name": "Scheduled Workflow",
                        "description": "scheduled workflow",
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

    let trigger = app
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/applications/{application_id}/workflow-schedule-trigger"
                ))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(trigger.status(), StatusCode::OK);
    let trigger_payload = response_json(trigger).await;
    assert_eq!(trigger_payload["data"]["enabled"], json!(false));
    assert_eq!(trigger_payload["data"]["cron"], json!("0 9 * * 1-5"));
    assert_eq!(trigger_payload["data"]["timezone"], json!("Asia/Shanghai"));
    assert_eq!(
        trigger_payload["data"]["input_payload"],
        json!({"report": "daily"})
    );
}

#[tokio::test]
async fn application_routes_create_extension_mapping_draft() {
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
                        "workflow_trigger_type": "extension",
                        "workflow_trigger_config": {
                            "subpath": "orders/create",
                            "http_method": "POST",
                            "access_policy": "user_api_key",
                            "response_mode": "sync"
                        },
                        "name": "Order Workflow",
                        "description": "order workflow",
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
    let create_status = create.status();
    let create_payload = response_json(create).await;
    assert_eq!(create_status, StatusCode::CREATED, "{create_payload}");
    // AC-001: extensions expose a published workflow operation, not AgentFlow keys.
    assert_eq!(
        create_payload["data"]["sections"]["api"]["status"],
        json!("available")
    );
    assert_eq!(
        create_payload["data"]["sections"]["api"]["credential_kind"],
        json!("user_or_public")
    );
    assert_eq!(
        create_payload["data"]["sections"]["api"]["invoke_routing_mode"],
        json!("published_workflow_operation")
    );
    let application_id = create_payload["data"]["id"].as_str().unwrap().to_string();
    let mapping = app
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/applications/{application_id}/api-mapping"
                ))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(mapping.status(), StatusCode::OK);
    let payload = response_json(mapping).await;
    assert_eq!(payload["data"]["extension"]["slug"], json!("orders/create"));
    assert_eq!(payload["data"]["extension"]["method"], json!("POST"));
    assert_eq!(payload["data"]["extension"]["response_mode"], json!("sync"));
}

#[tokio::test]
async fn application_routes_reject_manual_workflow_trigger_type() {
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
                        "icon": "ApiOutlined",
                        "icon_type": "iconfont-v2",
                        "icon_background": "#123456",
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
    assert_eq!(patch_payload["data"]["icon"].as_str(), Some("ApiOutlined"));
    assert_eq!(
        patch_payload["data"]["icon_type"].as_str(),
        Some("iconfont-v2")
    );
    assert_eq!(
        patch_payload["data"]["icon_background"].as_str(),
        Some("#123456")
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
