use super::*;

#[tokio::test]
async fn issue_2032_gateway_logs_use_console_auth_and_bounded_parent_queries() {
    let app = test_app().await;
    let uri = format!("/api/console/applications/{}/logs/gateway", Uuid::now_v7());
    let response = app
        .clone()
        .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let provider = create_ready_provider_instance(&app, &cookie, &csrf).await;
    let application_id = seed_agent_flow_application(&app, &cookie, &csrf, &provider).await;
    let value = get_console_json(
        &app,
        &cookie,
        format!("/api/console/applications/{application_id}/logs/gateway?page_size=10000"),
    )
    .await;
    assert_eq!(value["data"]["page_size"], 50);
    assert_eq!(value["data"]["total"], 0);
    let invalid=app.clone().oneshot(Request::builder().uri(format!("/api/console/applications/{application_id}/logs/gateway?conversation_id={}&turn_id={}",Uuid::now_v7(),Uuid::now_v7())).header("cookie",&cookie).body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(invalid.status(), StatusCode::BAD_REQUEST);
    let absent = app
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/applications/{}/logs/gateway",
                    Uuid::now_v7()
                ))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(absent.status(), StatusCode::NOT_FOUND);
}
