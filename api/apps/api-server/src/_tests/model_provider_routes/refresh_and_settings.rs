use super::*;

#[tokio::test]
async fn model_provider_routes_refresh_models_keeps_enabled_model_ids_unchanged() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let installation_id = install_enable_assign(&app, &cookie, &csrf).await;

    let create = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/settings/model-providers/instances")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "installation_id": installation_id,
                        "display_name": "Fixture Ready",
                        "configured_models": [
                            { "model_id": "fixture_chat", "enabled": true, "context_window_override_tokens": 128000 },
                            { "model_id": "custom-refresh", "enabled": false, "context_window_override_tokens": null }
                        ],
                        "config": {
                            "base_url": "https://api.example.com",
                            "api_key": "super-secret"
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create.status(), StatusCode::CREATED);
    let create_payload: Value =
        serde_json::from_slice(&to_bytes(create.into_body(), usize::MAX).await.unwrap()).unwrap();
    let instance_id = create_payload["data"]["id"].as_str().unwrap().to_string();

    let refresh = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/console/settings/model-providers/instances/{instance_id}/models/refresh"
                ))
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(refresh.status(), StatusCode::OK);
    let refresh_payload: Value =
        serde_json::from_slice(&to_bytes(refresh.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(
        refresh_payload["data"]["refresh_status"].as_str(),
        Some("ready")
    );
    assert_eq!(
        refresh_payload["data"]["models"][0]["model_id"].as_str(),
        Some("fixture_chat")
    );

    let list = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/console/settings/model-providers/instances")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(list.status(), StatusCode::OK);
    let list_payload: Value =
        serde_json::from_slice(&to_bytes(list.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(
        list_payload["data"][0]["configured_models"],
        json!([
            { "model_id": "fixture_chat", "enabled": true, "context_window_override_tokens": 128000 },
            { "model_id": "custom-refresh", "enabled": false, "context_window_override_tokens": null }
        ])
    );
    assert_eq!(
        list_payload["data"][0]["enabled_model_ids"],
        json!(["fixture_chat"])
    );
    assert!(list_payload["data"][0].get("validation_model_id").is_none());
    assert_eq!(list_payload["data"][0]["model_count"].as_u64(), Some(1));
}

#[tokio::test]
async fn model_provider_routes_main_instance_settings_drive_inclusion_and_grouped_options() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let installation_id = install_enable_assign(&app, &cookie, &csrf).await;

    let openapi = openapi_payload().await;
    let paths = openapi["paths"].as_object().unwrap();
    assert!(paths.contains_key(
        "/api/console/settings/model-providers/providers/{provider_code}/main-instance"
    ));
    assert!(!paths.contains_key("/api/console/model-providers/providers/{provider_code}/routing"));
    assert!(
        paths["/api/console/settings/model-providers/providers/{provider_code}/main-instance"]
            .get("get")
            .is_some()
    );
    assert!(
        paths["/api/console/settings/model-providers/providers/{provider_code}/main-instance"]
            ["get"]["responses"]
            .get("404")
            .is_some()
    );
    assert!(paths
        .get("/api/console/model-providers/{id}/balance")
        .and_then(|path| path.get("get"))
        .is_some());
    let main_instance_operation = &paths
        ["/api/console/settings/model-providers/providers/{provider_code}/main-instance"]["put"];
    assert!(main_instance_operation["responses"].get("404").is_some());
    assert!(main_instance_operation["responses"].get("409").is_some());
    let request_schema_name = main_instance_operation["requestBody"]["content"]["application/json"]
        ["schema"]["$ref"]
        .as_str()
        .and_then(|value| value.split('/').next_back())
        .expect("main-instance request schema ref");
    let schemas = openapi["components"]["schemas"].as_object().unwrap();
    assert_eq!(
        schemas[request_schema_name]["properties"]["auto_include_new_instances"]["type"].as_str(),
        Some("boolean")
    );
    assert!(schemas[request_schema_name]["properties"]
        .get("model_routing_policies")
        .is_some());
    assert!(schemas[request_schema_name]["properties"]
        .get("expected_revision")
        .is_some());
    assert!(schemas[request_schema_name]
        .get("properties")
        .and_then(|properties| properties.get("routing_mode"))
        .is_none());
    assert_eq!(
        schemas["ModelProviderInstanceResponse"]["properties"]["included_in_main"]["type"].as_str(),
        Some("boolean")
    );
    assert!(schemas
        .get("ModelProviderBalanceResponse")
        .and_then(|schema| schema.get("properties"))
        .and_then(|properties| properties.get("balance_infos"))
        .is_some());
    assert!(schemas["ModelProviderInstanceResponse"]
        .get("properties")
        .and_then(|properties| properties.get("is_primary"))
        .is_none());
    assert_eq!(
        schemas["ModelProviderOptionResponse"]["properties"]["main_instance"]["$ref"]
            .as_str()
            .and_then(|value| value.split('/').next_back()),
        Some("ModelProviderMainInstanceSummaryResponse")
    );
    assert_eq!(
        schemas["ModelProviderOptionResponse"]["properties"]["model_groups"]["items"]["$ref"]
            .as_str()
            .and_then(|value| value.split('/').next_back()),
        Some("ModelProviderOptionGroupResponse")
    );
    assert!(schemas["ModelProviderOptionGroupResponse"]["properties"]
        .get("distribution_rule")
        .is_some());
    assert_eq!(
        schema_ref_name(&schemas["ModelProviderOptionResponse"]["properties"]["parameter_form"])
            .as_deref(),
        Some("PluginFormSchemaResponse")
    );
    assert!(schemas["ProviderModelDescriptorResponse"]
        .get("properties")
        .and_then(|properties| properties.get("parameter_form"))
        .is_none());
    let override_schema =
        &schemas["ConfiguredModelResponse"]["properties"]["context_window_override_tokens"];
    assert!(
        override_schema["type"].as_str() == Some("integer")
            || override_schema["type"]
                .as_array()
                .is_some_and(|items| items.iter().any(|item| item.as_str() == Some("integer")))
            || override_schema
                .get("anyOf")
                .and_then(Value::as_array)
                .is_some_and(|items| items
                    .iter()
                    .any(|item| item["type"].as_str() == Some("integer")))
    );
    let multimodal_schema =
        &schemas["ConfiguredModelResponse"]["properties"]["supports_multimodal"];
    assert!(
        multimodal_schema["type"].as_str() == Some("boolean")
            || multimodal_schema["type"]
                .as_array()
                .is_some_and(|items| items.iter().any(|item| item.as_str() == Some("boolean")))
            || multimodal_schema
                .get("anyOf")
                .and_then(Value::as_array)
                .is_some_and(|items| items
                    .iter()
                    .any(|item| item["type"].as_str() == Some("boolean")))
    );
    assert!(schemas["ModelProviderOptionResponse"]
        .get("properties")
        .and_then(|properties| properties.get("effective_instance_id"))
        .is_none());

    let get_main_instance = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/console/settings/model-providers/providers/fixture_provider/main-instance")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(get_main_instance.status(), StatusCode::OK);
    let get_main_instance_payload: Value = serde_json::from_slice(
        &to_bytes(get_main_instance.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        get_main_instance_payload["data"]["provider_code"].as_str(),
        Some("fixture_provider")
    );
    assert_eq!(
        get_main_instance_payload["data"]["auto_include_new_instances"].as_bool(),
        Some(true)
    );

    let update_main_instance = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/console/settings/model-providers/providers/fixture_provider/main-instance")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "auto_include_new_instances": false,
                        "expected_revision": 0
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(update_main_instance.status(), StatusCode::OK);
    let update_main_instance_payload: Value = serde_json::from_slice(
        &to_bytes(update_main_instance.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        update_main_instance_payload["data"]["provider_code"].as_str(),
        Some("fixture_provider")
    );
    assert_eq!(
        update_main_instance_payload["data"]["auto_include_new_instances"].as_bool(),
        Some(false)
    );
    assert_eq!(
        update_main_instance_payload["data"]["model_routing_policies"]
            .as_array()
            .unwrap()
            .len(),
        0
    );

    let excluded = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/settings/model-providers/instances")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "installation_id": installation_id,
                        "display_name": "Excluded By Default",
                        "enabled_model_ids": ["fixture_chat"],
                        "config": {
                            "base_url": "https://excluded.example.com/v1",
                            "api_key": "super-secret"
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(excluded.status(), StatusCode::CREATED);
    let excluded_payload: Value =
        serde_json::from_slice(&to_bytes(excluded.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(
        excluded_payload["data"]["included_in_main"].as_bool(),
        Some(false)
    );
    assert!(excluded_payload["data"].get("is_primary").is_none());

    let alpha = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/settings/model-providers/instances")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "installation_id": installation_id,
                        "display_name": "Alpha",
                        "configured_models": [
                            {
                                "model_id": "fixture_chat",
                                "enabled": true,
                                "context_window_override_tokens": 256000
                            }
                        ],
                        "enabled_model_ids": ["fixture_chat"],
                        "included_in_main": true,
                        "config": {
                            "base_url": "https://alpha.example.com/v1",
                            "api_key": "super-secret"
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(alpha.status(), StatusCode::CREATED);
    let alpha_payload: Value =
        serde_json::from_slice(&to_bytes(alpha.into_body(), usize::MAX).await.unwrap()).unwrap();
    let alpha_id = alpha_payload["data"]["id"].as_str().unwrap().to_string();
    assert_eq!(
        alpha_payload["data"]["included_in_main"].as_bool(),
        Some(true)
    );

    let beta = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/settings/model-providers/instances")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "installation_id": installation_id,
                        "display_name": "Beta",
                        "enabled_model_ids": ["custom-beta"],
                        "included_in_main": true,
                        "config": {
                            "base_url": "https://beta.example.com/v1",
                            "api_key": "super-secret"
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(beta.status(), StatusCode::CREATED);
    let beta_payload: Value =
        serde_json::from_slice(&to_bytes(beta.into_body(), usize::MAX).await.unwrap()).unwrap();
    let beta_id = beta_payload["data"]["id"].as_str().unwrap().to_string();
    assert_eq!(
        beta_payload["data"]["included_in_main"].as_bool(),
        Some(true)
    );

    let update_distribution_rule = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/console/settings/model-providers/providers/fixture_provider/main-instance")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "auto_include_new_instances": false,
                        "expected_revision": 1,
                        "model_routing_policies": [
                            {
                                "model_id": "fixture_chat",
                                "distribution_rule": "retry_round_robin",
                                "provider_instance_ids": [alpha_id.clone()],
                                "excluded_provider_instance_ids": [alpha_id.clone()]
                            }
                        ]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(update_distribution_rule.status(), StatusCode::OK);
    let update_distribution_rule_payload: Value = serde_json::from_slice(
        &to_bytes(update_distribution_rule.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        update_distribution_rule_payload["data"]["model_routing_policies"][0]["model_id"].as_str(),
        Some("fixture_chat")
    );
    assert_eq!(
        update_distribution_rule_payload["data"]["model_routing_policies"][0]["distribution_rule"]
            .as_str(),
        Some("retry_round_robin")
    );
    assert_eq!(
        update_distribution_rule_payload["data"]["model_routing_policies"][0]
            ["provider_instance_ids"][0]
            .as_str(),
        Some(alpha_id.as_str())
    );
    assert_eq!(
        update_distribution_rule_payload["data"]["model_routing_policies"][0]
            ["excluded_provider_instance_ids"][0]
            .as_str(),
        Some(alpha_id.as_str())
    );

    let stale_update = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/console/settings/model-providers/providers/fixture_provider/main-instance")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "auto_include_new_instances": true,
                        "expected_revision": 1,
                        "model_routing_policies": []
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(stale_update.status(), StatusCode::CONFLICT);

    let options = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/console/model-providers/options?locale=zh_Hans")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(options.status(), StatusCode::OK);
    let options_payload: Value =
        serde_json::from_slice(&to_bytes(options.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(
        options_payload["data"]["providers"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert!(options_payload["data"]["providers"][0]
        .get("effective_instance_id")
        .is_none());
    assert_eq!(
        options_payload["data"]["providers"][0]["icon"].as_str(),
        Some("/api/console/model-providers/providers/fixture_provider/icon")
    );
    assert_eq!(
        options_payload["data"]["providers"][0]["main_instance"]["provider_code"].as_str(),
        Some("fixture_provider")
    );
    assert_eq!(
        options_payload["data"]["providers"][0]["main_instance"]["auto_include_new_instances"]
            .as_bool(),
        Some(false)
    );
    assert_eq!(
        options_payload["data"]["providers"][0]["main_instance"]["group_count"].as_u64(),
        Some(2)
    );
    assert_eq!(
        options_payload["data"]["providers"][0]["main_instance"]["model_count"].as_u64(),
        Some(2)
    );
    assert_eq!(
        options_payload["data"]["providers"][0]["parameter_form"]["fields"][0]["key"].as_str(),
        Some("temperature")
    );
    let groups = options_payload["data"]["providers"][0]["model_groups"]
        .as_array()
        .unwrap();
    assert_eq!(groups.len(), 2);
    let alpha_group = groups
        .iter()
        .find(|group| group["model_id"].as_str() == Some("fixture_chat"))
        .expect("fixture_chat group");
    assert_eq!(
        alpha_group["distribution_rule"].as_str(),
        Some("retry_round_robin")
    );
    let alpha_target = alpha_group["targets"]
        .as_array()
        .unwrap()
        .iter()
        .find(|target| target["source_instance_id"].as_str() == Some(alpha_id.as_str()))
        .expect("alpha target");
    assert_eq!(
        alpha_target["source_instance_display_name"].as_str(),
        Some("Alpha")
    );
    assert_eq!(alpha_target["routing_enabled"].as_bool(), Some(false));
    assert_eq!(
        alpha_group["model"]["model_id"].as_str(),
        Some("fixture_chat")
    );
    assert_eq!(
        alpha_group["model"]["context_window"].as_u64(),
        Some(256000)
    );
    assert!(alpha_group["model"].get("parameter_form").is_none());
    let beta_group = groups
        .iter()
        .find(|group| group["model_id"].as_str() == Some("custom-beta"))
        .expect("custom-beta group");
    assert_eq!(beta_group["distribution_rule"].as_str(), Some("none"));
    let beta_target = beta_group["targets"]
        .as_array()
        .unwrap()
        .iter()
        .find(|target| target["source_instance_id"].as_str() == Some(beta_id.as_str()))
        .expect("beta target");
    assert_eq!(beta_target["routing_enabled"].as_bool(), Some(true));
    assert_eq!(
        beta_target["source_instance_display_name"].as_str(),
        Some("Beta")
    );
    assert_eq!(
        beta_group["model"]["model_id"].as_str(),
        Some("custom-beta")
    );
    assert!(beta_group["model"].get("parameter_form").is_none());
}

#[tokio::test]
async fn model_provider_request_logs_require_authentication() {
    let app = test_app().await;
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/console/settings/model-providers/request-logs")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn model_provider_request_logs_return_attempt_page_shape() {
    let app = test_app().await;
    let (cookie, _) = login_and_capture_cookie(&app, "root", "change-me").await;
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/console/settings/model-providers/request-logs?page=1&page_size=20&zero_output_only=true")
                .header("cookie", cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let payload: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(payload["data"]["page"], json!(1));
    assert_eq!(payload["data"]["page_size"], json!(20));
    assert!(payload["data"]["items"].is_array());
    assert!(payload["data"]["total_count"].is_number());
}
