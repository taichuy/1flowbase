use super::*;

#[tokio::test]
async fn create_model_route_persists_draft_status_atomically_without_manage_permission() {
    let app = test_app().await;
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;

    create_role(&app, &root_cookie, &root_csrf, "model_creator").await;
    replace_role_permissions(
        &app,
        &root_cookie,
        &root_csrf,
        "model_creator",
        &["state_model.create.all", "state_model.view.all"],
    )
    .await;
    let creator_member_id =
        create_member(&app, &root_cookie, &root_csrf, "draft-creator", "temp-pass").await;
    replace_member_roles(
        &app,
        &root_cookie,
        &root_csrf,
        &creator_member_id,
        &["model_creator"],
    )
    .await;
    let (creator_cookie, creator_csrf) =
        login_and_capture_cookie(&app, "draft-creator", "temp-pass").await;

    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/models")
                .header("cookie", &creator_cookie)
                .header("x-csrf-token", &creator_csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "scope_kind": "workspace",
                        "code": "atomic_draft_orders",
                        "title": "Atomic Draft Orders",
                        "status": "draft",
                        "api_exposure_status": "api_exposed_ready"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(create_response.status(), StatusCode::CREATED);
    let created: serde_json::Value = serde_json::from_slice(
        &to_bytes(create_response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(created["data"]["status"], json!("draft"));
    assert_eq!(created["data"]["api_exposure_status"], json!("draft"));
    assert_eq!(
        created["data"]["runtime_availability"],
        json!("not_published")
    );

    for request in [
        Request::builder()
            .method("POST")
            .uri("/api/runtime/models/atomic_draft_orders/create")
            .header("cookie", &root_cookie)
            .header("x-csrf-token", &root_csrf)
            .header("content-type", "application/json")
            .body(Body::from(json!({}).to_string()))
            .unwrap(),
        Request::builder()
            .method("GET")
            .uri("/api/runtime/models/atomic_draft_orders/list")
            .header("cookie", &root_cookie)
            .body(Body::empty())
            .unwrap(),
    ] {
        let response = app.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::CONFLICT);
        let error: serde_json::Value =
            serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap())
                .unwrap();
        assert_eq!(error["code"], json!("model_not_published"));
    }
}

#[tokio::test]
async fn create_model_route_rejects_invalid_status_without_creating_model() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;

    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/models")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "scope_kind": "workspace",
                        "code": "invalid_status_orders",
                        "title": "Invalid Status Orders",
                        "status": "api_exposed_ready"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(create_response.status(), StatusCode::BAD_REQUEST);
    let error: serde_json::Value = serde_json::from_slice(
        &to_bytes(create_response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(error["code"], json!("status"));

    let list_response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/console/models")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(list_response.status(), StatusCode::OK);
    let listed: serde_json::Value = serde_json::from_slice(
        &to_bytes(list_response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    let models = listed["data"].as_array().unwrap();
    assert!(!models
        .iter()
        .any(|model| model["code"] == json!("invalid_status_orders")));
}

#[tokio::test]
async fn model_definition_routes_derive_published_exposure_ready() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;

    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/models")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "scope_kind": "workspace",
                        "code": "ready_fact_orders",
                        "title": "Ready Fact Orders"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_response.status(), StatusCode::CREATED);
    let created: serde_json::Value = serde_json::from_slice(
        &to_bytes(create_response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    let model_id = created["data"]["id"].as_str().unwrap().to_string();
    assert_eq!(
        created["data"]["api_exposure_status"],
        json!("api_exposed_ready")
    );

    let get_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/console/models/{model_id}"))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(get_response.status(), StatusCode::OK);
    let ready: serde_json::Value = serde_json::from_slice(
        &to_bytes(get_response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        ready["data"]["api_exposure_status"],
        json!("api_exposed_ready")
    );
}

#[tokio::test]
async fn model_definition_routes_persist_explicit_api_exposure_on_status_update() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;

    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/models")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "scope_kind": "workspace",
                        "code": "raw_update_ready_orders",
                        "title": "Raw Update Ready Orders",
                        "status": "draft"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_response.status(), StatusCode::CREATED);
    let created: serde_json::Value = serde_json::from_slice(
        &to_bytes(create_response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    let model_id = created["data"]["id"].as_str().unwrap().to_string();

    let update_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/api/console/models/{model_id}"))
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "status": "published",
                        "api_exposure_status": "api_exposed_ready"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(update_response.status(), StatusCode::OK);
    let updated: serde_json::Value = serde_json::from_slice(
        &to_bytes(update_response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(updated["data"]["status"], json!("published"));
    assert_eq!(
        updated["data"]["api_exposure_status"],
        json!("api_exposed_ready")
    );
}

#[tokio::test]
async fn model_definition_routes_hide_api_exposure_patch_behind_published_status() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;

    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/models")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "scope_kind": "workspace",
                        "code": "api_request_only_orders",
                        "title": "API Request Only Orders"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_response.status(), StatusCode::CREATED);
    let created: serde_json::Value = serde_json::from_slice(
        &to_bytes(create_response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    let model_id = created["data"]["id"].as_str().unwrap().to_string();

    let update_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/api/console/models/{model_id}"))
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "api_exposure_status": "api_exposed_no_permission"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(update_response.status(), StatusCode::OK);
    let updated: serde_json::Value = serde_json::from_slice(
        &to_bytes(update_response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(updated["data"]["status"], json!("published"));
    assert_eq!(
        updated["data"]["api_exposure_status"],
        json!("api_exposed_ready")
    );
}
