use super::*;

#[tokio::test]
async fn update_defaults_persists_valid_data_model_defaults() {
    let repository = InMemoryDataSourceRepository::default();
    let runtime = StubDataSourceRuntime::ready();
    let service = DataSourceService::new(repository, runtime);

    let created = service
        .create_instance(CreateDataSourceInstanceCommand {
            actor_user_id: user_id(),
            workspace_id: workspace_id(),
            installation_id: installation_id(),
            source_code: "acme_hubspot_source".into(),
            display_name: "HubSpot".into(),
            config_json: json!({ "client_id": "abc" }),
            secret_json: json!({ "client_secret": "secret" }),
        })
        .await
        .unwrap();

    let updated = service
        .update_defaults(UpdateDataSourceDefaultsCommand {
            actor_user_id: user_id(),
            workspace_id: workspace_id(),
            instance_id: created.instance.id,
            defaults: DataSourceDefaults {
                data_model_status: domain::DataModelStatus::Draft,
            },
        })
        .await
        .unwrap();

    assert_eq!(
        updated.defaults.data_model_status,
        domain::DataModelStatus::Draft
    );
}

#[tokio::test]
async fn preview_read_uses_stored_secret_and_creates_preview_session() {
    let repository = InMemoryDataSourceRepository::default();
    let runtime = StubDataSourceRuntime::ready();
    let service = DataSourceService::new(repository.clone(), runtime.clone());

    let created = service
        .create_instance(CreateDataSourceInstanceCommand {
            actor_user_id: user_id(),
            workspace_id: workspace_id(),
            installation_id: installation_id(),
            source_code: "acme_hubspot_source".into(),
            display_name: "HubSpot".into(),
            config_json: json!({ "client_id": "abc" }),
            secret_json: json!({ "client_secret": "secret" }),
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
            options_json: json!({ "sample": true }),
        })
        .await
        .unwrap();

    assert_eq!(preview.output.rows.len(), 1);
    assert_eq!(repository.preview_session_count().await, 1);

    let runtime_input = runtime.last_preview_input().await.unwrap();
    assert_eq!(
        runtime_input.connection,
        DataSourceConfigInput {
            config_json: json!({ "client_id": "abc" }),
            secret_json: json!({ "client_secret": "secret" }),
        }
    );
    assert_eq!(runtime_input.resource_key, "contacts");
}

#[tokio::test]
async fn map_resource_to_model_uses_descriptor_fields_capabilities_and_stored_secret() {
    let repository = InMemoryDataSourceRepository::default();
    let runtime = StubDataSourceRuntime::ready();
    let service = DataSourceService::new(repository.clone(), runtime.clone());

    let created = service
        .create_instance(CreateDataSourceInstanceCommand {
            actor_user_id: user_id(),
            workspace_id: workspace_id(),
            installation_id: installation_id(),
            source_code: "acme_hubspot_source".into(),
            display_name: "HubSpot".into(),
            config_json: json!({ "client_id": "abc" }),
            secret_json: json!({ "client_secret": "secret-for-describe" }),
        })
        .await
        .unwrap();

    let mapped = service
        .map_resource_to_model(MapDataSourceResourceToModelCommand {
            actor_user_id: user_id(),
            workspace_id: workspace_id(),
            instance_id: created.instance.id,
            resource_key: "contacts".into(),
        })
        .await
        .unwrap();

    assert_eq!(
        mapped.model.data_source_instance_id,
        Some(created.instance.id)
    );
    assert_eq!(
        mapped.model.source_kind,
        domain::DataModelSourceKind::ExternalSource
    );
    assert_eq!(
        mapped.model.external_resource_key.as_deref(),
        Some("contacts")
    );
    assert_eq!(
        mapped.model.external_capability_snapshot,
        Some(json!({
            "supports_list": true,
            "supports_get": true,
            "supports_create": false,
            "supports_update": false,
            "supports_delete": false,
            "supports_filter": true,
            "supports_sort": false,
            "supports_pagination": false,
            "supports_owner_filter": false,
            "supports_scope_filter": true,
            "supports_write": false,
            "supports_transactions": false
        }))
    );
    assert_eq!(mapped.fields.len(), 2);
    assert_eq!(mapped.fields[0].external_field_key.as_deref(), Some("id"));
    assert_eq!(
        mapped.fields[1].external_field_key.as_deref(),
        Some("properties.email")
    );
    assert_eq!(mapped.fields[1].code, "properties_email");

    let runtime_input = runtime.last_describe_input().await.unwrap();
    assert_eq!(
        runtime_input.connection,
        DataSourceConfigInput {
            config_json: json!({ "client_id": "abc" }),
            secret_json: json!({ "client_secret": "secret-for-describe" }),
        }
    );
    assert_eq!(runtime_input.resource_key, "contacts");

    let models = repository.mapped_models().await;
    assert_eq!(models.len(), 1);
    let audit_events = repository.audit_events().await;
    assert!(audit_events.iter().any(|event| event.event_code
        == "data_source.resource_mapped_to_model"
        && event.target_id == Some(mapped.model.id)
        && event.payload["data_source_instance_id"] == json!(created.instance.id)
        && event.payload["resource_key"] == json!("contacts")));
    let audit_text = serde_json::to_string(&audit_events).unwrap();
    assert!(!audit_text.contains("secret-for-describe"));
}

#[tokio::test]
async fn map_resource_to_model_redacts_descriptor_secret_echoes_before_mapping() {
    let repository = InMemoryDataSourceRepository::default();
    let runtime = StubDataSourceRuntime::echoing_secret();
    let service = DataSourceService::new(repository.clone(), runtime.clone());
    let plaintext = "descriptor-secret-substring";

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

    let mapped = service
        .map_resource_to_model(MapDataSourceResourceToModelCommand {
            actor_user_id: user_id(),
            workspace_id: workspace_id(),
            instance_id: created.instance.id,
            resource_key: "contacts".into(),
        })
        .await
        .unwrap();

    let runtime_input = runtime.last_describe_input().await.unwrap();
    assert_eq!(
        runtime_input.connection,
        DataSourceConfigInput {
            config_json: json!({ "client_id": "abc" }),
            secret_json: json!({ "client_secret": plaintext }),
        }
    );

    assert!(!mapped.model.title.contains(plaintext));
    assert!(!mapped
        .model
        .external_resource_key
        .as_deref()
        .unwrap()
        .contains(plaintext));
    assert!(mapped.fields.iter().all(|field| {
        !field.title.contains(plaintext)
            && !field
                .external_field_key
                .as_deref()
                .unwrap()
                .contains(plaintext)
    }));

    let mapped_text = serde_json::to_string(&json!({
        "model_title": mapped.model.title,
        "external_resource_key": mapped.model.external_resource_key,
        "fields": mapped.fields,
    }))
    .unwrap();
    let model_store_text = serde_json::to_string(&repository.mapped_models().await).unwrap();
    let audit_text = serde_json::to_string(&repository.audit_events().await).unwrap();
    assert!(!mapped_text.contains(plaintext));
    assert!(!model_store_text.contains(plaintext));
    assert!(!audit_text.contains(plaintext));
    assert!(mapped_text.contains("***"));
}
