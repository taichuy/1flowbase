use super::*;

#[tokio::test]
async fn validate_and_preview_redact_runtime_echoed_secret_values() {
    let repository = InMemoryDataSourceRepository::default();
    let runtime = StubDataSourceRuntime::echoing_secret();
    let service = data_source_service(repository.clone(), runtime, "test-master-key");
    let plaintext = "secret-runtime-echo";

    let created = service
        .create_instance(CreateDataSourceInstanceCommand {
            actor_user_id: user_id(),
            workspace_id: workspace_id(),
            installation_id: installation_id(),
            source_code: "acme_hubspot_source".into(),
            display_name: "HubSpot".into(),
            config_json: json!({ "client_id": "abc" }),
            secret_json: json!({ "client_secret": plaintext }),
        })
        .await
        .unwrap();

    let validated = service
        .validate_instance(ValidateDataSourceInstanceCommand {
            actor_user_id: user_id(),
            workspace_id: workspace_id(),
            instance_id: created.instance.id,
        })
        .await
        .unwrap();
    let preview = service
        .preview_read(PreviewDataSourceReadCommand {
            actor_user_id: user_id(),
            workspace_id: workspace_id(),
            instance_id: created.instance.id,
            resource_key: "contacts".into(),
            limit: Some(20),
            cursor: None,
            options_json: json!({}),
        })
        .await
        .unwrap();

    let validate_text = validated.output.to_string();
    let preview_text = serde_json::to_string(&preview.output.rows).unwrap();
    let preview_session = repository
        .preview_sessions
        .read()
        .await
        .values()
        .next()
        .unwrap()
        .clone();
    assert!(!validate_text.contains(plaintext));
    assert!(!preview_text.contains(plaintext));
    assert!(!preview_session.config_fingerprint.contains(plaintext));
    assert!(validate_text.contains("***"));
    assert!(preview_text.contains("***"));
    assert!(!serde_json::to_string(&preview_session.preview_json)
        .unwrap()
        .contains(plaintext));
}

#[tokio::test]
async fn validate_preview_and_catalog_redact_embedded_secret_substrings() {
    let repository = InMemoryDataSourceRepository::default();
    let runtime = StubDataSourceRuntime::echoing_secret();
    let service = data_source_service(repository.clone(), runtime, "test-master-key");
    let plaintext = "embedded-secret-value";

    let created = service
        .create_instance(CreateDataSourceInstanceCommand {
            actor_user_id: user_id(),
            workspace_id: workspace_id(),
            installation_id: installation_id(),
            source_code: "acme_hubspot_source".into(),
            display_name: "HubSpot".into(),
            config_json: json!({ "client_id": "abc" }),
            secret_json: json!({ "client_secret": plaintext }),
        })
        .await
        .unwrap();

    let validated = service
        .validate_instance(ValidateDataSourceInstanceCommand {
            actor_user_id: user_id(),
            workspace_id: workspace_id(),
            instance_id: created.instance.id,
        })
        .await
        .unwrap();
    let discovered = service
        .discover_resources(DiscoverDataSourceResourcesCommand {
            actor_user_id: user_id(),
            workspace_id: workspace_id(),
            instance_id: created.instance.id,
        })
        .await
        .unwrap();
    let preview = service
        .preview_read(PreviewDataSourceReadCommand {
            actor_user_id: user_id(),
            workspace_id: workspace_id(),
            instance_id: created.instance.id,
            resource_key: "contacts".into(),
            limit: Some(20),
            cursor: None,
            options_json: json!({}),
        })
        .await
        .unwrap();

    let validate_text = validated.output.to_string();
    let catalog_text = serde_json::to_string(&discovered.entries).unwrap();
    let stored_catalog = repository
        .caches
        .read()
        .await
        .get(&created.instance.id)
        .expect("catalog cache should be persisted")
        .catalog_json
        .clone();
    let stored_catalog_text = serde_json::to_string(&stored_catalog).unwrap();
    let preview_text = serde_json::to_string(&preview.output.rows).unwrap();

    assert!(!validate_text.contains(plaintext));
    assert!(!catalog_text.contains(plaintext));
    assert!(!stored_catalog_text.contains(plaintext));
    assert!(!preview_text.contains(plaintext));
    assert!(validate_text.contains("Bearer ***"));
    assert!(catalog_text.contains("Bearer ***"));
    assert!(stored_catalog_text.contains("Bearer ***"));
    assert!(preview_text.contains("Bearer ***"));
}
