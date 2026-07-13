use super::*;

fn assert_no_model_level_api_exposure(payload: &serde_json::Value) {
    assert!(!payload["data"]
        .as_object()
        .expect("data response object")
        .contains_key("api_exposure_status"));
}

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
        &["settings_feature.access.system.data-models"],
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
                .uri("/api/console/settings/data-models/model-definitions")
                .header("cookie", &creator_cookie)
                .header("x-csrf-token", &creator_csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "scope_kind": "workspace",
                        "code": "atomic_draft_orders",
                        "title": "Atomic Draft Orders",
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
    assert_eq!(created["data"]["status"], json!("draft"));
    assert_no_model_level_api_exposure(&created);
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
                .uri("/api/console/settings/data-models/model-definitions")
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
                .uri("/api/console/settings/data-models/model-definitions")
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
async fn model_definition_routes_return_status_without_model_level_api_exposure() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;

    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/settings/data-models/model-definitions")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "scope_kind": "workspace",
                        "code": "status_only_fact_orders",
                        "title": "Status Only Fact Orders"
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
    assert_eq!(created["data"]["status"], json!("published"));
    assert_no_model_level_api_exposure(&created);

    let list_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/console/settings/data-models/model-definitions")
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
    let ready = listed["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|model| model["id"].as_str() == Some(&model_id))
        .unwrap();
    assert_eq!(ready["status"], json!("published"));
    assert!(!ready
        .as_object()
        .unwrap()
        .contains_key("api_exposure_status"));
}

#[tokio::test]
async fn status_patch_opens_and_closes_runtime_api() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;

    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/settings/data-models/model-definitions")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "scope_kind": "workspace",
                        "code": "status_runtime_orders",
                        "title": "Status Runtime Orders",
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

    let publish_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!(
                    "/api/console/settings/data-models/model-definitions/{model_id}"
                ))
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(json!({ "status": "published" }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(publish_response.status(), StatusCode::OK);
    let published: serde_json::Value = serde_json::from_slice(
        &to_bytes(publish_response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(published["data"]["status"], json!("published"));
    assert_eq!(
        published["data"]["runtime_availability"],
        json!("available")
    );
    assert_no_model_level_api_exposure(&published);

    let list_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/runtime/models/status_runtime_orders/list")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(list_response.status(), StatusCode::OK);

    let unpublish_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!(
                    "/api/console/settings/data-models/model-definitions/{model_id}"
                ))
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(json!({ "status": "draft" }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(unpublish_response.status(), StatusCode::OK);
    let unpublished: serde_json::Value = serde_json::from_slice(
        &to_bytes(unpublish_response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(unpublished["data"]["status"], json!("draft"));
    assert_eq!(
        unpublished["data"]["runtime_availability"],
        json!("not_published")
    );
    assert_no_model_level_api_exposure(&unpublished);

    let blocked_response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/runtime/models/status_runtime_orders/list")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(blocked_response.status(), StatusCode::CONFLICT);
    let blocked: serde_json::Value = serde_json::from_slice(
        &to_bytes(blocked_response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(blocked["code"], json!("model_not_published"));
}
