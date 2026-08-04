use super::*;

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
