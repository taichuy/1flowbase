use super::*;

#[tokio::test]
async fn runtime_model_routes_apply_persisted_scope_all_grant_for_session_actors() {
    let app = test_app().await;
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let model_id = create_orders_model(&app, &root_cookie, &root_csrf).await;
    create_text_field(&app, &root_cookie, &root_csrf, &model_id, "title").await;
    create_enum_field(&app, &root_cookie, &root_csrf, &model_id, "status").await;

    let _manager_member_id =
        create_member(&app, &root_cookie, &root_csrf, "manager-acl", "temp-pass").await;
    let admin_member_id =
        create_member(&app, &root_cookie, &root_csrf, "admin-acl", "temp-pass").await;
    replace_member_roles(&app, &root_cookie, &root_csrf, &admin_member_id, &["admin"]).await;

    let (manager_cookie, manager_csrf) =
        login_and_capture_cookie(&app, "manager-acl", "temp-pass").await;
    let (admin_cookie, admin_csrf) = login_and_capture_cookie(&app, "admin-acl", "temp-pass").await;

    let manager_create = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/runtime/models/orders/create")
                .header("cookie", &manager_cookie)
                .header("x-csrf-token", &manager_csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "title": "manager-order", "status": "draft" }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(manager_create.status(), StatusCode::CREATED);
    let manager_record_body = to_bytes(manager_create.into_body(), usize::MAX)
        .await
        .unwrap();
    let manager_record: serde_json::Value = serde_json::from_slice(&manager_record_body).unwrap();
    let manager_record_id = manager_record["data"]["id"].as_str().unwrap().to_string();

    let admin_create = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/runtime/models/orders/create")
                .header("cookie", &admin_cookie)
                .header("x-csrf-token", &admin_csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "title": "admin-order", "status": "paid" }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(admin_create.status(), StatusCode::CREATED);
    let admin_record_body = to_bytes(admin_create.into_body(), usize::MAX)
        .await
        .unwrap();
    let admin_record: serde_json::Value = serde_json::from_slice(&admin_record_body).unwrap();
    let admin_record_id = admin_record["data"]["id"].as_str().unwrap().to_string();

    let root_create = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/runtime/models/orders/create")
                .header("cookie", &root_cookie)
                .header("x-csrf-token", &root_csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "title": "root-order", "status": "draft" }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(root_create.status(), StatusCode::CREATED);

    let manager_list = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/runtime/models/orders/list")
                .header("cookie", &manager_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(manager_list.status(), StatusCode::OK);
    let manager_list_body = to_bytes(manager_list.into_body(), usize::MAX)
        .await
        .unwrap();
    let manager_list_payload: serde_json::Value =
        serde_json::from_slice(&manager_list_body).unwrap();
    assert_eq!(manager_list_payload["data"]["total"], json!(3));

    let admin_list = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/runtime/models/orders/list")
                .header("cookie", &admin_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(admin_list.status(), StatusCode::OK);
    let admin_list_body = to_bytes(admin_list.into_body(), usize::MAX).await.unwrap();
    let admin_list_payload: serde_json::Value = serde_json::from_slice(&admin_list_body).unwrap();
    assert_eq!(admin_list_payload["data"]["total"], json!(3));

    let root_list = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/runtime/models/orders/list")
                .header("cookie", &root_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(root_list.status(), StatusCode::OK);
    let root_list_body = to_bytes(root_list.into_body(), usize::MAX).await.unwrap();
    let root_list_payload: serde_json::Value = serde_json::from_slice(&root_list_body).unwrap();
    assert_eq!(root_list_payload["data"]["total"], json!(3));

    let manager_get = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/runtime/models/orders/get/{admin_record_id}"))
                .header("cookie", &manager_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(manager_get.status(), StatusCode::OK);

    let admin_get = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!(
                    "/api/runtime/models/orders/get/{manager_record_id}"
                ))
                .header("cookie", &admin_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(admin_get.status(), StatusCode::OK);

    let root_get = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/runtime/models/orders/get/{admin_record_id}"))
                .header("cookie", &root_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(root_get.status(), StatusCode::OK);
}

#[tokio::test]
async fn runtime_model_routes_gate_crud_by_model_status_changes() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let model_code = "status_route_orders";
    let model_id = create_model_with_status(&app, &cookie, &csrf, model_code, Some("draft")).await;
    create_text_field(&app, &cookie, &csrf, &model_id, "title").await;

    assert_runtime_crud_blocked(
        &app,
        &cookie,
        &csrf,
        model_code,
        StatusCode::CONFLICT,
        "model_not_published",
    )
    .await;

    let published = update_model_status(&app, &cookie, &csrf, &model_id, "published").await;
    assert_eq!(published["data"]["status"], json!("published"));
    assert_eq!(
        published["data"]["runtime_availability"],
        json!("available")
    );

    let created = create_runtime_record(&app, &cookie, &csrf, model_code, "created").await;
    let record_id = created["data"]["id"].as_str().unwrap().to_string();

    let get_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/runtime/models/{model_code}/get/{record_id}"))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(get_response.status(), StatusCode::OK);

    let disabled = update_model_status(&app, &cookie, &csrf, &model_id, "disabled").await;
    assert_eq!(disabled["data"]["status"], json!("disabled"));
    assert_eq!(disabled["data"]["runtime_availability"], json!("disabled"));

    assert_runtime_crud_blocked(
        &app,
        &cookie,
        &csrf,
        model_code,
        StatusCode::CONFLICT,
        "model_disabled",
    )
    .await;

    let broken = update_model_status(&app, &cookie, &csrf, &model_id, "broken").await;
    assert_eq!(broken["data"]["status"], json!("broken"));
    assert_eq!(broken["data"]["runtime_availability"], json!("broken"));

    assert_runtime_crud_blocked(
        &app,
        &cookie,
        &csrf,
        model_code,
        StatusCode::CONFLICT,
        "model_broken",
    )
    .await;
}

#[tokio::test]
async fn runtime_model_routes_use_default_scope_id_for_workspace_model_crud() {
    let (app, database_url) = test_app_with_database_url().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let model_code = "default_route_orders";
    let model_id =
        create_model_with_status(&app, &cookie, &csrf, model_code, Some("published")).await;
    create_text_field(&app, &cookie, &csrf, &model_id, "title").await;

    create_runtime_record(&app, &cookie, &csrf, model_code, "default scoped").await;

    let durable = storage_durable::build_main_durable_postgres(&database_url)
        .await
        .unwrap();
    let pool = durable.store;
    let model_id = uuid::Uuid::parse_str(&model_id).unwrap();
    let (metadata_scope_kind, metadata_scope_id, physical_table_name): (
        String,
        uuid::Uuid,
        String,
    ) = sqlx::query_as(
        r#"
        select scope_kind, scope_id, physical_table_name
        from model_definitions
        where id = $1
        "#,
    )
    .bind(model_id)
    .fetch_one(pool.pool())
    .await
    .unwrap();
    assert_eq!(metadata_scope_kind, "system");
    assert_eq!(metadata_scope_id, domain::SYSTEM_SCOPE_ID);

    let grant_scope_id: uuid::Uuid = sqlx::query_scalar(
        r#"
        select scope_id
        from scope_data_model_grants
        where data_model_id = $1
          and scope_kind = 'workspace'
        "#,
    )
    .bind(model_id)
    .fetch_one(pool.pool())
    .await
    .unwrap();
    assert_eq!(grant_scope_id, domain::DEFAULT_SCOPE_ID);
    assert_ne!(grant_scope_id, domain::SYSTEM_SCOPE_ID);

    let scope_id: uuid::Uuid = sqlx::query_scalar(&format!(
        "select scope_id from \"{physical_table_name}\" limit 1"
    ))
    .fetch_one(pool.pool())
    .await
    .unwrap();

    assert_eq!(scope_id, grant_scope_id);
    assert_eq!(scope_id, domain::DEFAULT_SCOPE_ID);
    assert_ne!(scope_id, domain::SYSTEM_SCOPE_ID);
}

#[tokio::test]
async fn runtime_model_routes_return_403_when_scope_grant_is_missing() {
    let (app, database_url) = test_app_with_database_url().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let model_code = "ungranted_route_orders";
    let model_id =
        create_model_with_status(&app, &cookie, &csrf, model_code, Some("published")).await;
    create_text_field(&app, &cookie, &csrf, &model_id, "title").await;
    revoke_model_grant(&database_url, &model_id).await;

    assert_runtime_crud_blocked(
        &app,
        &cookie,
        &csrf,
        model_code,
        StatusCode::FORBIDDEN,
        "data_model_scope_not_granted",
    )
    .await;
}
