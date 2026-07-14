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

async fn response_json(response: axum::response::Response) -> Value {
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    serde_json::from_slice(&body).unwrap()
}

async fn create_application(
    app: &axum::Router,
    cookie: &str,
    csrf: &str,
    application_type: &str,
    workflow_trigger_type: Option<&str>,
    name: &str,
) -> String {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/applications")
                .header("cookie", cookie)
                .header("x-csrf-token", csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "application_type": application_type,
                        "workflow_trigger_type": workflow_trigger_type,
                        "name": name,
                        "description": format!("{name} description"),
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
    assert_eq!(response.status(), StatusCode::CREATED);
    response_json(response).await["data"]["id"]
        .as_str()
        .unwrap()
        .to_string()
}

#[tokio::test]
async fn ac_004_005_application_management_route_filters_pages_and_returns_backend_truth() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    create_application(&app, &cookie, &csrf, "agent_flow", None, "Support Agent").await;
    create_application(
        &app,
        &cookie,
        &csrf,
        "workflow",
        Some("schedule"),
        "Daily Report",
    )
    .await;

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/console/settings/applications?page=1&page_size=1&filter=%7B%22application_type%22%3A%22workflow%22%7D&sort=updated_at%3Adesc")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let status = response.status();
    let payload = response_json(response).await;
    assert_eq!(status, StatusCode::OK, "{payload}");
    assert_eq!(payload["data"]["total"], json!(1));
    assert_eq!(payload["data"]["page"], json!(1));
    assert_eq!(payload["data"]["page_size"], json!(1));
    let items = payload["data"]["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["application_type"], json!("workflow"));
    assert_eq!(items[0]["workflow_trigger_type"], json!("schedule"));
    assert_eq!(items[0]["publication_status"], json!("unpublished"));
    assert!(items[0]["created_by_display_name"]
        .as_str()
        .is_some_and(|value| !value.is_empty()));
    assert!(items[0]["created_at"].as_str().is_some());
    assert!(items[0]["updated_at"].as_str().is_some());
}

#[tokio::test]
async fn ac_002_008_own_viewer_keeps_workbench_access_without_settings_management_access() {
    let app = test_app().await;
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let member_id = create_member(
        &app,
        &root_cookie,
        &root_csrf,
        "application-own-viewer",
        "temp-pass",
    )
    .await;
    create_role(&app, &root_cookie, &root_csrf, "application_own_viewer").await;
    replace_role_permissions(
        &app,
        &root_cookie,
        &root_csrf,
        "application_own_viewer",
        &["application.view.own", "application.create.all"],
    )
    .await;
    replace_member_roles(
        &app,
        &root_cookie,
        &root_csrf,
        &member_id,
        &["application_own_viewer"],
    )
    .await;
    let (member_cookie, member_csrf) =
        login_and_capture_cookie(&app, "application-own-viewer", "temp-pass").await;
    create_application(
        &app,
        &member_cookie,
        &member_csrf,
        "agent_flow",
        None,
        "Owned Agent",
    )
    .await;

    let workbench = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/console/applications")
                .header("cookie", &member_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(workbench.status(), StatusCode::OK);
    assert_eq!(
        response_json(workbench).await["data"]
            .as_array()
            .unwrap()
            .len(),
        1
    );

    let management = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/console/settings/applications")
                .header("cookie", &member_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(management.status(), StatusCode::FORBIDDEN);

    let navigation = app
        .oneshot(
            Request::builder()
                .uri("/api/console/navigation")
                .header("cookie", &member_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(navigation.status(), StatusCode::OK);
    let navigation = response_json(navigation).await;
    assert!(navigation["data"]["navigation_items"]
        .as_array()
        .unwrap()
        .iter()
        .all(|item| item["route_id"] != "settings.applications"));
}
