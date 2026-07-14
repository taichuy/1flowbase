use super::*;

#[tokio::test]
async fn runtime_model_routes_create_fetch_update_delete_and_filter_records() {
    let (app, database_url) = test_app_with_database_url().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let model_id = create_orders_model(&app, &cookie, &csrf).await;
    create_text_field(&app, &cookie, &csrf, &model_id, "title").await;
    create_enum_field(&app, &cookie, &csrf, &model_id, "status").await;

    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/runtime/models/orders/create")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "title": "A-001", "status": "draft" }).to_string(),
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
    let record_id = created["data"]["id"].as_str().unwrap().to_string();

    let list_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/runtime/models/orders/list?filter=%7B%22status%22%3A%7B%22%24eq%22%3A%22draft%22%7D%7D&sort=title:desc")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(list_response.status(), StatusCode::OK);

    let invalid_filter_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/runtime/models/orders/list?filter=%7B%22status%22%3A%7B%22%24startsWith%22%3A%22dra%22%7D%7D")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(invalid_filter_response.status(), StatusCode::BAD_REQUEST);

    let get_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/runtime/models/orders/get/{record_id}"))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(get_response.status(), StatusCode::OK);

    let update_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/api/runtime/models/orders/update/{record_id}"))
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "title": "A-001-UPDATED", "status": "draft" }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(update_response.status(), StatusCode::OK);

    for old_route_request in [
        Request::builder()
            .method("GET")
            .uri("/api/runtime/models/orders/records")
            .header("cookie", &cookie)
            .body(Body::empty())
            .unwrap(),
        Request::builder()
            .method("POST")
            .uri("/api/runtime/models/orders/records")
            .header("cookie", &cookie)
            .header("x-csrf-token", &csrf)
            .header("content-type", "application/json")
            .body(Body::from(
                json!({ "title": "legacy", "status": "draft" }).to_string(),
            ))
            .unwrap(),
        Request::builder()
            .method("GET")
            .uri(format!("/api/runtime/models/orders/records/{record_id}"))
            .header("cookie", &cookie)
            .body(Body::empty())
            .unwrap(),
        Request::builder()
            .method("PATCH")
            .uri(format!("/api/runtime/models/orders/records/{record_id}"))
            .header("cookie", &cookie)
            .header("x-csrf-token", &csrf)
            .header("content-type", "application/json")
            .body(Body::from(
                json!({ "title": "legacy", "status": "draft" }).to_string(),
            ))
            .unwrap(),
        Request::builder()
            .method("DELETE")
            .uri(format!("/api/runtime/models/orders/records/{record_id}"))
            .header("cookie", &cookie)
            .header("x-csrf-token", &csrf)
            .body(Body::empty())
            .unwrap(),
    ] {
        let old_route_response = app.clone().oneshot(old_route_request).await.unwrap();
        assert_eq!(old_route_response.status(), StatusCode::NOT_FOUND);
    }

    let delete_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/runtime/models/orders/delete/{record_id}"))
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(delete_response.status(), StatusCode::OK);

    drop_runtime_table(&database_url, &model_id).await;

    let unavailable_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/runtime/models/orders/list")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(unavailable_response.status(), StatusCode::CONFLICT);
    let unavailable_payload: serde_json::Value = serde_json::from_slice(
        &to_bytes(unavailable_response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        unavailable_payload["code"],
        json!("runtime_model_unavailable")
    );
}

#[tokio::test]
async fn runtime_model_routes_cache_main_source_reads_and_invalidate_after_writes() {
    let (app, database_url) = test_app_with_database_url().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let model_id = create_model_with_status(&app, &cookie, &csrf, "cache_orders", None).await;
    create_text_field(&app, &cookie, &csrf, &model_id, "title").await;

    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/runtime/models/cache_orders/create")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(json!({ "title": "cached-title" }).to_string()))
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
    let record_id = created["data"]["id"].as_str().unwrap().to_string();

    let first_list = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/runtime/models/cache_orders/list")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(first_list.status(), StatusCode::OK);
    let first_get = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/runtime/models/cache_orders/get/{record_id}"))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(first_get.status(), StatusCode::OK);

    update_runtime_record_title_directly(&database_url, &model_id, &record_id, "db-bypass-title")
        .await;

    let cached_list = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/runtime/models/cache_orders/list")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let cached_list_payload: serde_json::Value =
        serde_json::from_slice(&to_bytes(cached_list.into_body(), usize::MAX).await.unwrap())
            .unwrap();
    assert_eq!(
        cached_list_payload["data"]["items"][0]["title"],
        json!("cached-title")
    );

    let cached_get = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/runtime/models/cache_orders/get/{record_id}"))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let cached_get_payload: serde_json::Value =
        serde_json::from_slice(&to_bytes(cached_get.into_body(), usize::MAX).await.unwrap())
            .unwrap();
    assert_eq!(cached_get_payload["data"]["title"], json!("cached-title"));

    let patch_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!(
                    "/api/runtime/models/cache_orders/update/{record_id}"
                ))
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "title": "api-updated-title" }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(patch_response.status(), StatusCode::OK);

    let invalidated_get = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/runtime/models/cache_orders/get/{record_id}"))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let invalidated_get_payload: serde_json::Value = serde_json::from_slice(
        &to_bytes(invalidated_get.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        invalidated_get_payload["data"]["title"],
        json!("api-updated-title")
    );
}

#[tokio::test]
async fn runtime_model_routes_dispatch_external_source_crud_to_data_source_runtime() {
    let package = TempDataSourcePackage::new();
    write_external_runtime_package(&package);
    let (app, database_url) = test_app_with_database_url().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let data_source_instance_id = seed_runtime_data_source_instance(&database_url, &package).await;

    let create_model_response = app
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
                        "data_source_instance_id": data_source_instance_id,
                        "external_resource_key": "contacts",
                        "code": "external_runtime_contacts",
                        "title": "External Runtime Contacts",
                        "status": "published"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_model_response.status(), StatusCode::CREATED);
    let model_payload: serde_json::Value = serde_json::from_slice(
        &to_bytes(create_model_response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    let model_id = model_payload["data"]["id"].as_str().unwrap().to_string();
    assert_eq!(
        model_payload["data"]["source_kind"],
        json!("external_source")
    );

    create_text_field_with_external_key(&app, &cookie, &csrf, &model_id, "email", "email_address")
        .await;
    create_text_field_with_external_key(
        &app,
        &cookie,
        &csrf,
        &model_id,
        "token_echo",
        "secret_echo",
    )
    .await;

    let list_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/runtime/models/external_runtime_contacts/list")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let list_status = list_response.status();
    let list_payload: serde_json::Value = serde_json::from_slice(
        &to_bytes(list_response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        list_status,
        StatusCode::OK,
        "unexpected list payload: {list_payload}"
    );
    assert_eq!(list_payload["data"]["total"], json!(1));
    assert_eq!(
        list_payload["data"]["items"][0]["email"],
        json!("list@example.com")
    );
    assert_eq!(
        list_payload["data"]["items"][0]["token_echo"],
        json!("Bearer ***")
    );
    assert!(!list_payload.to_string().contains("route-runtime-secret"));

    let get_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/runtime/models/external_runtime_contacts/get/contact-1")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(get_response.status(), StatusCode::OK);
    let get_payload: serde_json::Value = serde_json::from_slice(
        &to_bytes(get_response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(get_payload["data"]["email"], json!("get@example.com"));
    assert_eq!(get_payload["data"]["token_echo"], json!("Bearer ***"));

    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/runtime/models/external_runtime_contacts/create")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "email": "created@example.com" }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_response.status(), StatusCode::CREATED);
    let create_payload: serde_json::Value = serde_json::from_slice(
        &to_bytes(create_response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(create_payload["data"]["id"], json!("contact-created"));
    assert_eq!(
        create_payload["data"]["email"],
        json!("created@example.com")
    );
    assert_eq!(create_payload["data"]["token_echo"], json!("Bearer ***"));

    let update_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/api/runtime/models/external_runtime_contacts/update/contact-1")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "email": "updated@example.com" }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(update_response.status(), StatusCode::OK);
    let update_payload: serde_json::Value = serde_json::from_slice(
        &to_bytes(update_response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        update_payload["data"]["email"],
        json!("updated@example.com")
    );
    assert_eq!(update_payload["data"]["token_echo"], json!("Bearer ***"));

    let delete_response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/runtime/models/external_runtime_contacts/delete/contact-1")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(delete_response.status(), StatusCode::OK);
    let delete_payload: serde_json::Value = serde_json::from_slice(
        &to_bytes(delete_response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(delete_payload["data"]["deleted"], json!(true));
}

#[tokio::test]
async fn runtime_model_routes_external_source_runtime_blocks_unassigned_or_unavailable_installations(
) {
    let cases = [
        (
            "instance_not_ready",
            RuntimeDataSourceSeedOptions {
                provider_code: "fixture_external_data_source_instance_not_ready",
                source_code: "fixture_external_data_source_instance_not_ready",
                instance_status: "draft",
                ..RuntimeDataSourceSeedOptions::default()
            },
            StatusCode::CONFLICT,
            "data_source_instance_not_ready",
        ),
        (
            "unassigned",
            RuntimeDataSourceSeedOptions {
                provider_code: "fixture_external_data_source_unassigned",
                source_code: "fixture_external_data_source_unassigned",
                assign: false,
                ..RuntimeDataSourceSeedOptions::default()
            },
            StatusCode::CONFLICT,
            "plugin_assignment_required",
        ),
        (
            "disabled",
            RuntimeDataSourceSeedOptions {
                provider_code: "fixture_external_data_source_disabled",
                source_code: "fixture_external_data_source_disabled",
                desired_state: "disabled",
                runtime_status: "active",
                availability_status: "disabled",
                ..RuntimeDataSourceSeedOptions::default()
            },
            StatusCode::CONFLICT,
            "plugin_installation_unavailable",
        ),
        (
            "load_failed",
            RuntimeDataSourceSeedOptions {
                provider_code: "fixture_external_data_source_load_failed",
                source_code: "fixture_external_data_source_load_failed",
                desired_state: "active_requested",
                runtime_status: "load_failed",
                availability_status: "load_failed",
                ..RuntimeDataSourceSeedOptions::default()
            },
            StatusCode::CONFLICT,
            "plugin_installation_unavailable",
        ),
        (
            "artifact_missing",
            RuntimeDataSourceSeedOptions {
                provider_code: "fixture_external_data_source_artifact_missing",
                source_code: "fixture_external_data_source_artifact_missing",
                installed_path: Some(
                    "/tmp/1flowbase-plan-d-runtime-artifact-missing-does-not-exist",
                ),
                ..RuntimeDataSourceSeedOptions::default()
            },
            StatusCode::CONFLICT,
            "plugin_installation_unavailable",
        ),
        (
            "contract_mismatch",
            RuntimeDataSourceSeedOptions {
                provider_code: "fixture_external_data_source_contract_mismatch",
                source_code: "fixture_external_data_source_contract_mismatch",
                contract_version: "1flowbase.model_provider/v1",
                ..RuntimeDataSourceSeedOptions::default()
            },
            StatusCode::BAD_REQUEST,
            "plugin_installation",
        ),
        (
            "source_code_mismatch",
            RuntimeDataSourceSeedOptions {
                provider_code: "fixture_external_data_source_source_mismatch",
                source_code: "fixture_external_data_source_other",
                ..RuntimeDataSourceSeedOptions::default()
            },
            StatusCode::BAD_REQUEST,
            "source_code",
        ),
    ];

    for (case_name, options, expected_status, expected_code) in cases {
        let package = TempDataSourcePackage::new();
        write_external_runtime_package(&package);
        let (app, database_url) = test_app_with_database_url().await;
        let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
        let data_source_instance_id =
            seed_runtime_data_source_instance_with_options(&database_url, &package, options).await;
        let model_code = format!("external_runtime_blocked_{case_name}");

        let create_model_response = app
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
                            "data_source_instance_id": data_source_instance_id,
                            "external_resource_key": "contacts",
                            "code": model_code,
                            "title": format!("External Runtime Blocked {case_name}"),
                            "status": "published"
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(create_model_response.status(), StatusCode::CREATED);
        let model_payload: serde_json::Value = serde_json::from_slice(
            &to_bytes(create_model_response.into_body(), usize::MAX)
                .await
                .unwrap(),
        )
        .unwrap();
        let model_id = model_payload["data"]["id"].as_str().unwrap().to_string();
        create_text_field_with_external_key(&app, &cookie, &csrf, &model_id, "email", "email")
            .await;

        let list_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/api/runtime/models/{model_code}/list"))
                    .header("cookie", &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = list_response.status();
        let payload: serde_json::Value = serde_json::from_slice(
            &to_bytes(list_response.into_body(), usize::MAX)
                .await
                .unwrap(),
        )
        .unwrap();

        assert_eq!(
            status, expected_status,
            "unexpected status for {case_name}: {payload}"
        );
        assert_eq!(payload["code"], json!(expected_code));
        assert!(
            !payload.to_string().contains("list@example.com"),
            "runtime fixture was called for {case_name}: {payload}"
        );
    }
}
