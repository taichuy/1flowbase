use super::*;

#[tokio::test]
async fn model_provider_routes_mask_secret_until_reveal_and_keep_ready_options() {
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
                        "display_name": "Fixture Prod",
                        "enabled_model_ids": [],
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
    assert_eq!(
        create_payload["data"]["included_in_main"].as_bool(),
        Some(true)
    );
    assert!(create_payload["data"].get("is_primary").is_none());
    assert_eq!(create_payload["data"]["enabled_model_ids"], json!([]));
    assert!(create_payload["data"].get("validation_model_id").is_none());

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
        list_payload["data"][0]["config_json"]["base_url"].as_str(),
        Some("https://api.example.com")
    );
    assert_eq!(
        list_payload["data"][0]["config_json"]["api_key"].as_str(),
        Some("supe****cret")
    );
    assert_eq!(
        list_payload["data"][0]["included_in_main"].as_bool(),
        Some(true)
    );
    assert!(list_payload["data"][0].get("is_primary").is_none());
    assert_eq!(list_payload["data"][0]["enabled_model_ids"], json!([]));
    assert!(list_payload["data"][0].get("validation_model_id").is_none());

    let reveal = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/console/settings/model-providers/instances/{instance_id}/secrets/reveal"
                ))
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "key": "api_key"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(reveal.status(), StatusCode::OK);
    let reveal_payload: Value =
        serde_json::from_slice(&to_bytes(reveal.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(reveal_payload["data"]["key"].as_str(), Some("api_key"));
    assert_eq!(
        reveal_payload["data"]["value"].as_str(),
        Some("super-secret")
    );

    let validate = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/console/settings/model-providers/instances/{instance_id}/validate"
                ))
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(validate.status(), StatusCode::OK);
    let validate_payload: Value =
        serde_json::from_slice(&to_bytes(validate.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(
        validate_payload["data"]["instance"]["status"].as_str(),
        Some("draft")
    );
    assert_eq!(
        validate_payload["data"]["instance"]["enabled_model_ids"],
        json!([])
    );
    assert_eq!(
        validate_payload["data"]["instance"]["included_in_main"].as_bool(),
        Some(true)
    );
    assert!(
        validate_payload["data"]["instance"]
            .get("is_primary")
            .is_none()
    );
    assert!(
        validate_payload["data"]["instance"]
            .get("validation_model_id")
            .is_none()
    );
    assert_eq!(
        validate_payload["data"]["output"]["sanitized"]["api_key"].as_str(),
        Some("***")
    );

    let balance = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!(
                    "/api/console/model-providers/{instance_id}/balance"
                ))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(balance.status(), StatusCode::OK);
    let balance_payload: Value =
        serde_json::from_slice(&to_bytes(balance.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(
        balance_payload["data"]["is_available"].as_bool(),
        Some(true)
    );
    assert_eq!(
        balance_payload["data"]["balance_infos"][0]["currency"].as_str(),
        Some("CNY")
    );
    assert!(!balance_payload.to_string().contains("super-secret"));

    let catalog = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/console/settings/model-providers/catalog?locale=zh_Hans")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(catalog.status(), StatusCode::OK);
    let catalog_payload: Value =
        serde_json::from_slice(&to_bytes(catalog.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(
        catalog_payload["data"]["locale_meta"]["resolved_locale"].as_str(),
        Some("zh_Hans")
    );
    assert!(
        catalog_payload["data"]["i18n_catalog"]["plugin.fixture_provider"]["zh_Hans"].is_object()
    );
    assert_eq!(
        catalog_payload["data"]["entries"][0]["namespace"].as_str(),
        Some("plugin.fixture_provider")
    );
    assert_eq!(
        catalog_payload["data"]["entries"][0]["label_key"].as_str(),
        Some("provider.label")
    );
    assert_eq!(
        catalog_payload["data"]["entries"][0]["catalog_refresh_status"].as_str(),
        Some("ok")
    );
    assert_eq!(
        catalog_payload["data"]["entries"][0]["predefined_models"][0]["label_key"].as_str(),
        Some("models.fixture_chat.label")
    );
    assert_eq!(
        catalog_payload["data"]["entries"][0]["form_schema"][2]["key"].as_str(),
        Some("api_protocol")
    );
    assert_eq!(
        catalog_payload["data"]["entries"][0]["form_schema"][2]["control"].as_str(),
        Some("select")
    );
    assert_eq!(
        catalog_payload["data"]["entries"][0]["form_schema"][2]["default_value"].as_str(),
        Some("openai_chat")
    );
    assert_eq!(
        catalog_payload["data"]["entries"][0]["form_schema"][2]["options"][0]["value"].as_str(),
        Some("openai_chat")
    );
    assert_eq!(
        catalog_payload["data"]["entries"][0]["form_schema"][3]["key"].as_str(),
        Some("organization")
    );
    assert_eq!(
        catalog_payload["data"]["entries"][0]["form_schema"][3]["advanced"].as_bool(),
        Some(true)
    );

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
    assert_eq!(options_payload["data"]["providers"], json!([]));
    assert_eq!(
        options_payload["data"]["locale_meta"]["resolved_locale"].as_str(),
        Some("zh_Hans")
    );
    assert_eq!(options_payload["data"]["i18n_catalog"], json!({}));
}

#[tokio::test]
async fn model_provider_routes_preview_models_from_draft_config_and_existing_secret() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let installation_id = install_enable_assign(&app, &cookie, &csrf).await;

    let preview_create = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/settings/model-providers/preview-models")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "installation_id": installation_id,
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
    assert_eq!(preview_create.status(), StatusCode::OK);
    let preview_create_payload: Value = serde_json::from_slice(
        &to_bytes(preview_create.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        preview_create_payload["data"]["models"][0]["model_id"].as_str(),
        Some("fixture_chat")
    );

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
                        "display_name": "Fixture Prod",
                        "enabled_model_ids": [],
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

    let preview_edit = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/settings/model-providers/preview-models")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "instance_id": instance_id,
                        "config": {
                            "base_url": "https://api.example.com"
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(preview_edit.status(), StatusCode::OK);
    let preview_edit_payload: Value = serde_json::from_slice(
        &to_bytes(preview_edit.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        preview_edit_payload["data"]["models"][0]["model_id"].as_str(),
        Some("fixture_chat")
    );
}

#[tokio::test]
async fn model_provider_routes_create_instance_accepts_configured_models_with_preview_token() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let installation_id = install_enable_assign(&app, &cookie, &csrf).await;

    let preview = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/settings/model-providers/preview-models")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "installation_id": installation_id,
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
    assert_eq!(preview.status(), StatusCode::OK);
    let preview_payload: Value =
        serde_json::from_slice(&to_bytes(preview.into_body(), usize::MAX).await.unwrap()).unwrap();
    let preview_token = preview_payload["data"]["preview_token"]
        .as_str()
        .expect("preview token should exist");

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
                        "display_name": "Fixture Prod",
                        "configured_models": [
                            { "model_id": "fixture_chat", "enabled": true, "context_window_override_tokens": 128000, "supports_multimodal": true },
                            { "model_id": "custom-preview", "enabled": false, "context_window_override_tokens": null, "supports_multimodal": false }
                        ],
                        "preview_token": preview_token,
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
    assert_eq!(create_payload["data"]["status"].as_str(), Some("ready"));
    assert_eq!(
        create_payload["data"]["configured_models"],
        json!([
            { "model_id": "fixture_chat", "enabled": true, "context_window_override_tokens": 128000, "supports_multimodal": true },
            { "model_id": "custom-preview", "enabled": false, "context_window_override_tokens": null, "supports_multimodal": false }
        ])
    );
    assert_eq!(
        create_payload["data"]["enabled_model_ids"],
        json!(["fixture_chat"])
    );
    assert!(create_payload["data"].get("validation_model_id").is_none());
    assert_eq!(create_payload["data"]["model_count"].as_u64(), Some(1));
}

#[tokio::test]
async fn model_provider_routes_update_instance_accepts_configured_models() {
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
                        "display_name": "Fixture Draft",
                        "configured_models": [],
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

    let update = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/api/console/settings/model-providers/instances/{instance_id}"))
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "display_name": "Fixture Ready",
                        "included_in_main": true,
                        "configured_models": [
                            { "model_id": " fixture_chat ", "enabled": true, "context_window_override_tokens": 64000, "supports_multimodal": true },
                            { "model_id": "custom-preview", "enabled": false, "context_window_override_tokens": null, "supports_multimodal": false },
                            { "model_id": "fixture_chat", "enabled": true, "context_window_override_tokens": 128000, "supports_multimodal": false },
                            { "model_id": "", "enabled": true, "context_window_override_tokens": 32000 }
                        ],
                        "config": {}
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(update.status(), StatusCode::OK);
    let update_payload: Value =
        serde_json::from_slice(&to_bytes(update.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(update_payload["data"]["status"].as_str(), Some("ready"));
    assert_eq!(
        update_payload["data"]["configured_models"],
        json!([
            { "model_id": "fixture_chat", "enabled": true, "context_window_override_tokens": 64000, "supports_multimodal": true },
            { "model_id": "custom-preview", "enabled": false, "context_window_override_tokens": null, "supports_multimodal": false }
        ])
    );
    assert_eq!(
        update_payload["data"]["enabled_model_ids"],
        json!(["fixture_chat"])
    );
    assert_eq!(
        update_payload["data"]["included_in_main"].as_bool(),
        Some(true)
    );
    assert!(update_payload["data"].get("is_primary").is_none());
    assert!(update_payload["data"].get("validation_model_id").is_none());

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
            { "model_id": "fixture_chat", "enabled": true, "context_window_override_tokens": 64000, "supports_multimodal": true },
            { "model_id": "custom-preview", "enabled": false, "context_window_override_tokens": null, "supports_multimodal": false }
        ])
    );
    assert_eq!(
        list_payload["data"][0]["enabled_model_ids"],
        json!(["fixture_chat"])
    );
    assert_eq!(
        list_payload["data"][0]["included_in_main"].as_bool(),
        Some(true)
    );
    assert!(list_payload["data"][0].get("is_primary").is_none());
    assert!(list_payload["data"][0].get("validation_model_id").is_none());
}

#[tokio::test]
async fn model_provider_routes_create_instance_allows_preview_token_with_empty_configured_models() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let installation_id = install_enable_assign(&app, &cookie, &csrf).await;

    let preview = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/settings/model-providers/preview-models")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "installation_id": installation_id,
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
    assert_eq!(preview.status(), StatusCode::OK);
    let preview_payload: Value =
        serde_json::from_slice(&to_bytes(preview.into_body(), usize::MAX).await.unwrap()).unwrap();
    let preview_token = preview_payload["data"]["preview_token"]
        .as_str()
        .expect("preview token should exist");

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
                        "display_name": "Fixture Draft",
                        "configured_models": [],
                        "preview_token": preview_token,
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
    assert_eq!(create_payload["data"]["status"].as_str(), Some("draft"));
    assert_eq!(create_payload["data"]["configured_models"], json!([]));
    assert_eq!(create_payload["data"]["enabled_model_ids"], json!([]));
    assert!(create_payload["data"].get("validation_model_id").is_none());
}
