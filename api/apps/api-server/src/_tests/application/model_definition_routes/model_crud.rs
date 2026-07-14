use super::*;

async fn create_model(
    app: &axum::Router,
    cookie: &str,
    csrf: &str,
    code: &str,
    title: &str,
) -> String {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/settings/data-models/model-definitions")
                .header("cookie", cookie)
                .header("x-csrf-token", csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "scope_kind": "workspace",
                        "code": code,
                        "title": title
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
    let payload: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
    payload["data"]["id"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn model_definition_routes_manage_models_and_fields_without_publish() {
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
                        "code": "orders",
                        "title": "Orders"
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
    assert_eq!(created["data"]["status"], json!("published"));
    assert!(
        !created["data"]
            .as_object()
            .unwrap()
            .contains_key("api_exposure_status")
    );
    assert_eq!(created["data"]["runtime_availability"], json!("available"));
    assert_eq!(created["data"]["builtin_kind"], serde_json::Value::Null);
    assert_eq!(created["data"]["capabilities"]["can_delete"], json!(true));
    assert_eq!(
        created["data"]["capabilities"]["can_add_user_field"],
        json!(true)
    );
    assert_eq!(
        created["data"]["capabilities"]["record"]["can_create"],
        json!(true)
    );
    let model_id = created["data"]["id"].as_str().unwrap().to_string();

    let list_main_source_models = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/console/settings/data-models/model-definitions?data_source_id=main")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(list_main_source_models.status(), StatusCode::OK);
    let list_main_source_payload: serde_json::Value = serde_json::from_slice(
        &to_bytes(list_main_source_models.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    let models = list_main_source_payload["data"].as_array().unwrap();
    let model_codes = models
        .iter()
        .filter_map(|model| model["code"].as_str())
        .collect::<Vec<_>>();
    assert!(model_codes.contains(&"attachments"));
    assert!(model_codes.contains(&"users"));
    assert!(model_codes.contains(&"roles"));
    assert!(model_codes.contains(&"application_run_log_summaries"));
    assert!(models.iter().any(|model| {
        model["id"].as_str() == Some(&model_id)
            && model["source_kind"].as_str() == Some("main_source")
    }));
    assert!(models.iter().all(|model| {
        model["data_source_id"].as_str() == Some("main")
            && model["source_kind"].as_str() == Some("main_source")
    }));
    let users_model = models
        .iter()
        .find(|model| model["code"] == json!("users"))
        .expect("users builtin model should be listed");
    assert_eq!(users_model["builtin_kind"], json!("core"));
    assert_eq!(users_model["capabilities"]["can_delete"], json!(false));
    assert_eq!(
        users_model["capabilities"]["can_add_user_field"],
        json!(true)
    );
    assert_eq!(
        users_model["capabilities"]["can_update_lifecycle_status"],
        json!(false)
    );
    let account_field = users_model["fields"]
        .as_array()
        .unwrap()
        .iter()
        .find(|field| field["code"] == json!("account"))
        .expect("users.account field should be listed");
    assert_eq!(account_field["description"], serde_json::Value::Null);
    assert_eq!(
        account_field["capabilities"]["ownership"],
        json!("system_owned")
    );
    assert_eq!(
        account_field["capabilities"]["can_update_presentation_metadata"],
        json!(true)
    );
    assert_eq!(
        account_field["capabilities"]["can_update_physical_metadata"],
        json!(false)
    );
    assert_eq!(account_field["capabilities"]["can_delete"], json!(false));

    let runtime_logs_model = models
        .iter()
        .find(|model| model["code"] == json!("application_run_log_summaries"))
        .expect("runtime read model should be listed");
    assert_eq!(runtime_logs_model["builtin_kind"], json!("runtime_read"));
    assert_eq!(
        runtime_logs_model["capabilities"]["record"]["can_create"],
        json!(false)
    );
    let runtime_logs_model_id = runtime_logs_model["id"].as_str().unwrap();
    let delete_runtime_logs = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!(
                    "/api/console/settings/data-models/model-definitions/{runtime_logs_model_id}?confirmed=true"
                ))
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(delete_runtime_logs.status(), StatusCode::BAD_REQUEST);
    let delete_runtime_logs_payload: serde_json::Value = serde_json::from_slice(
        &to_bytes(delete_runtime_logs.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        delete_runtime_logs_payload["code"],
        json!("builtin_data_model")
    );

    let field_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/console/settings/data-models/model-definitions/{model_id}/fields"
                ))
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "code": "status",
                        "title": "Status",
                        "description": "Payment lifecycle state",
                        "field_kind": "enum",
                        "is_required": true,
                        "api_required": true,
                        "is_unique": false,
                        "display_interface": "select",
                        "display_options": { "options": ["draft", "paid"] }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(field_response.status(), StatusCode::CREATED);
    let created_field: serde_json::Value = serde_json::from_slice(
        &to_bytes(field_response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    let field_id = created_field["data"]["id"].as_str().unwrap().to_string();
    assert_eq!(
        created_field["data"]["description"],
        json!("Payment lifecycle state")
    );
    assert_eq!(created_field["data"]["api_required"], json!(true));
    assert_eq!(
        created_field["data"]["capabilities"]["ownership"],
        json!("user_added")
    );
    assert_eq!(
        created_field["data"]["capabilities"]["can_update_physical_metadata"],
        json!(true)
    );

    let agent_flow_options_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/console/models/agent-flow-options")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(agent_flow_options_response.status(), StatusCode::OK);
    let agent_flow_options_payload: serde_json::Value = serde_json::from_slice(
        &to_bytes(agent_flow_options_response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    let agent_flow_options = agent_flow_options_payload["data"].as_array().unwrap();
    let orders_option = agent_flow_options
        .iter()
        .find(|option| option["modelId"].as_str() == Some(&model_id))
        .unwrap();
    assert_eq!(orders_option["value"], json!("orders"));
    assert_eq!(orders_option["label"], json!("Orders"));
    assert_eq!(orders_option["state"], json!("enabled"));
    assert_eq!(orders_option["disabled"], json!(false));
    assert!(orders_option["disabledReason"].is_null());
    let status_field = orders_option["fields"]
        .as_array()
        .unwrap()
        .iter()
        .find(|field| field["code"] == json!("status"))
        .unwrap();
    assert_eq!(status_field["valueType"], json!("enum"));
    assert_eq!(status_field["required"], json!(true));
    assert_eq!(status_field["writable"], json!(true));

    let missing_api_required_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/runtime/models/orders/create")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(json!({}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        missing_api_required_response.status(),
        StatusCode::BAD_REQUEST
    );
    let missing_api_required_payload: serde_json::Value = serde_json::from_slice(
        &to_bytes(missing_api_required_response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(missing_api_required_payload["code"], json!("api_required"));

    let create_runtime_record = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/runtime/models/orders/create")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(json!({ "status": "draft" }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(create_runtime_record.status(), StatusCode::CREATED);

    let update_model_response = app
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
                .body(Body::from(
                    json!({
                        "title": "Orders V2"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(update_model_response.status(), StatusCode::OK);

    let create_after_model_update = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/runtime/models/orders/create")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(json!({ "status": "paid" }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(create_after_model_update.status(), StatusCode::CREATED);

    let update_field_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/api/console/settings/data-models/model-definitions/{model_id}/fields/{field_id}"))
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "title": "Lifecycle Status",
                        "description": "Shown on payment forms",
                        "is_required": true,
                        "api_required": false,
                        "is_unique": false,
                        "default_value": "draft",
                        "display_interface": "select",
                        "display_options": { "options": ["draft", "paid"] },
                        "relation_options": {}
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(update_field_response.status(), StatusCode::OK);
    let updated_field: serde_json::Value = serde_json::from_slice(
        &to_bytes(update_field_response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        updated_field["data"]["description"],
        json!("Shown on payment forms")
    );
    assert_eq!(updated_field["data"]["api_required"], json!(false));

    let create_after_field_update = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/runtime/models/orders/create")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(json!({ "status": "draft" }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(create_after_field_update.status(), StatusCode::CREATED);

    let second_field_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/console/settings/data-models/model-definitions/{model_id}/fields"
                ))
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "code": "note",
                        "title": "Note",
                        "field_kind": "text",
                        "is_required": false,
                        "is_unique": false
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(second_field_response.status(), StatusCode::CREATED);
    let second_field: serde_json::Value = serde_json::from_slice(
        &to_bytes(second_field_response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    let second_field_id = second_field["data"]["id"].as_str().unwrap().to_string();

    let delete_field_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!(
                    "/api/console/settings/data-models/model-definitions/{model_id}/fields/{second_field_id}?confirmed=true"
                ))
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(delete_field_response.status(), StatusCode::OK);

    let create_after_field_delete = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/runtime/models/orders/create")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(json!({ "status": "paid" }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(create_after_field_delete.status(), StatusCode::CREATED);

    let list_runtime_records = app
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

    assert_eq!(list_runtime_records.status(), StatusCode::OK);
    let listed_records: serde_json::Value = serde_json::from_slice(
        &to_bytes(list_runtime_records.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(listed_records["data"]["total"], json!(4));
}

#[tokio::test]
async fn model_definition_routes_filter_models_by_title_code_or_table_id() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;

    create_model(&app, &cookie, &csrf, "orders", "Orders").await;
    create_model(
        &app,
        &cookie,
        &csrf,
        "customer_profiles",
        "Customer Profiles",
    )
    .await;

    let filter_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/console/settings/data-models/model-definitions?data_source_id=main&filter=%7B%22code%22%3A%7B%22%24includes%22%3A%22customer%22%7D%7D")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(filter_response.status(), StatusCode::OK);
    let payload: serde_json::Value = serde_json::from_slice(
        &to_bytes(filter_response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    let codes = payload["data"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|model| model["code"].as_str())
        .collect::<Vec<_>>();

    assert_eq!(codes, vec!["customer_profiles"]);
}

#[tokio::test]
async fn model_definition_routes_batch_delete_models_from_action_endpoint() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let orders_id = create_model(&app, &cookie, &csrf, "orders", "Orders").await;
    let invoices_id = create_model(&app, &cookie, &csrf, "invoices", "Invoices").await;

    let unconfirmed_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/settings/data-models/model-definitions:batchDelete")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "filterByTk": [orders_id, invoices_id],
                        "confirmed": false
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(unconfirmed_response.status(), StatusCode::BAD_REQUEST);

    let delete_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/settings/data-models/model-definitions:batchDelete")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "filterByTk": [orders_id, invoices_id],
                        "confirmed": true
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(delete_response.status(), StatusCode::OK);
    let payload: serde_json::Value = serde_json::from_slice(
        &to_bytes(delete_response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(payload["data"]["deleted_count"], json!(2));

    let list_response = app
        .oneshot(
            Request::builder()
                .uri("/api/console/settings/data-models/model-definitions?data_source_id=main&filter=%7B%22code%22%3A%7B%22%24includes%22%3A%22orders%22%7D%7D")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let list_payload: serde_json::Value = serde_json::from_slice(
        &to_bytes(list_response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert!(list_payload["data"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn ac_005_generic_model_route_rejects_external_mapping_fields() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;

    let create_with_external_resource_key = app
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
                        "external_resource_key": "contacts",
                        "code": "main_source_with_external_key",
                        "title": "Main Source With External Key"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        create_with_external_resource_key.status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );

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
                        "code": "main_source_field_external_key",
                        "title": "Main Source Field External Key"
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
    let model_id = created["data"]["id"].as_str().unwrap();

    let field_with_external_key = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/console/settings/data-models/model-definitions/{model_id}/fields"
                ))
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "code": "email",
                        "title": "Email",
                        "external_field_key": "properties.email",
                        "field_kind": "string"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(field_with_external_key.status(), StatusCode::BAD_REQUEST);
    let error: serde_json::Value = serde_json::from_slice(
        &to_bytes(field_with_external_key.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(error["code"], json!("external_field_key"));
    assert_eq!(error["message"], json!("invalid input: external_field_key"));
}
