use super::*;

#[tokio::test]
async fn list_frontstage_pages_route_returns_empty_tree_for_accessible_workspace() {
    let app = test_app().await;
    let (cookie, _) = login_and_capture_cookie(&app, "root", "change-me").await;
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/console/frontstage/pages"))
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
async fn list_frontstage_pages_legacy_workspace_path_is_rejected_as_unregistered_console_operation()
{
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

    // The compiled authorization registry rejects the removed operation before route matching.
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let payload: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(
        payload["code"],
        json!("console_route_unregistered"),
        "{payload}"
    );
}

#[tokio::test]
async fn list_frontstage_pages_legacy_workspace_segment_is_rejected_as_unregistered_console_operation(
) {
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

    // This is an authorization-layer rejection, not evidence that a legacy route remains mounted.
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let payload: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(
        payload["code"],
        json!("console_route_unregistered"),
        "{payload}"
    );
}

#[tokio::test]
async fn list_frontstage_pages_route_requires_session() {
    let app = test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/console/frontstage/pages")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}
