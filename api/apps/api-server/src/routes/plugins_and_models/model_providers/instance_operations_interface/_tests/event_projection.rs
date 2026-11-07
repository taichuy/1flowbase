use super::*;
fn operation(id: &str) -> ApiEventOperationContext {
    ApiEventOperationContext {
        interface_id: id.into(),
        method: "POST".into(),
        path: "/api/console/settings/model-providers/instances/:id/authenticate".into(),
    }
}
fn instance() -> ModelProviderInstanceResponse {
    ModelProviderInstanceResponse {
        id: "provider-a".into(),
        installation_id: "installation-a".into(),
        provider_code: "provider".into(),
        protocol: "openai".into(),
        display_name: "Provider".into(),
        status: "ready".into(),
        included_in_main: true,
        config_json: json!({"endpoint":"https://provider.test","metadata":{"secret_label":"Public label"}}),
        configured_models: vec![],
        enabled_model_ids: vec![],
        catalog_refresh_status: None,
        catalog_last_error_message: None,
        catalog_refreshed_at: None,
        model_count: 0,
    }
}
#[test]
fn authentication_submit_and_device_challenge_never_enter_event_schema_or_value() {
    let op = operation("model_providers.instances.authenticate");
    let input = ProviderInstanceOperationsInput::Authenticate {
        id: "provider-a".into(),
        body: AuthenticateModelProviderInstanceBody {
            operation: json!({"type":"submit","value":"AUTH-CALLBACK-SENTINEL"}),
        },
    };
    assert_eq!(
        input.project_document_input(&op).unwrap(),
        json!({"path":{"id":"provider-a"},"body":{"operation":{"type":"submit"}}})
    );
    assert!(!ProviderInstanceOperationsInput::document_input_schema(&op)
        .unwrap()
        .to_string()
        .contains("\"value\""));
    let response = AuthenticateModelProviderInstanceResponse {
        instance: instance(),
        status: "pending".into(),
        message: Some("Continue in your browser".into()),
        user_action: Some(ModelProviderAuthUserActionResponse {
            kind: "device_code".into(),
            open_url: Some("https://provider.test?state=AUTH-URL-SENTINEL".into()),
            user_code: Some("AUTH-CODE-SENTINEL".into()),
            expires_at: Some("2026-11-07".into()),
            poll_interval_seconds: Some(5),
            prompt: Some("Complete authentication".into()),
        }),
    };
    assert!(serde_json::to_value(&response)
        .unwrap()
        .to_string()
        .contains("AUTH-CODE-SENTINEL"));
    let value = ProviderInstanceOperationsOutput::Authentication(response)
        .project_document_output(&op)
        .unwrap();
    assert!(!value.to_string().contains("SENTINEL"));
    assert_eq!(value["user_action"]["kind"], "device_code");
    assert_eq!(value["user_action"]["poll_interval_seconds"], 5);
    assert_eq!(
        value["instance"]["config_json"]["metadata"]["secret_label"],
        "Public label"
    );
    let schema = ProviderInstanceOperationsOutput::document_output_schema(&op)
        .unwrap()
        .to_string();
    assert!(!schema.contains("open_url"));
    assert!(!schema.contains("user_code"));
}
#[test]
fn revealed_secret_value_and_model_preview_proof_are_separate_from_event_business_data() {
    let op = operation("model_providers.instances.secrets.reveal");
    let response = RevealModelProviderSecretResponse {
        key: "api_key".into(),
        value: "REVEALED-KEY-SENTINEL".into(),
    };
    assert_eq!(
        serde_json::to_value(&response).unwrap()["value"],
        "REVEALED-KEY-SENTINEL"
    );
    assert_eq!(
        ProviderInstanceOperationsOutput::Secret(response)
            .project_document_output(&op)
            .unwrap(),
        json!({"key":"api_key"})
    );
    assert!(
        !ProviderInstanceOperationsOutput::document_output_schema(&op)
            .unwrap()
            .to_string()
            .contains("\"value\"")
    );
    let op = operation("model_providers.preview.view");
    let preview = ProviderInstanceOperationsOutput::Preview(PreviewModelProviderModelsResponse {
        models: vec![],
        preview_token: "MODEL-PREVIEW-PROOF-SENTINEL".into(),
        expires_at: "2026-11-07".into(),
    });
    assert_eq!(
        preview.project_document_output(&op).unwrap(),
        json!({"models":[],"expires_at":"2026-11-07"})
    );
    assert!(
        !ProviderInstanceOperationsOutput::document_output_schema(&op)
            .unwrap()
            .to_string()
            .contains("preview_token")
    );
}
#[test]
fn balance_metadata_remains_complete_public_business_json() {
    let op = operation("model_providers.balance.view");
    let metadata =
        json!({"secret_label":"Public billing label","entries":(0..100).collect::<Vec<_>>()});
    let response = ModelProviderBalanceResponse {
        is_available: true,
        balance_infos: vec![ModelProviderBalanceInfoResponse {
            currency: "USD".into(),
            total_balance: "12.34".into(),
            granted_balance: None,
            topped_up_balance: None,
        }],
        provider_metadata: metadata.clone(),
    };
    assert_eq!(
        ProviderInstanceOperationsOutput::Balance(response)
            .project_document_output(&op)
            .unwrap()["provider_metadata"],
        metadata
    );
}
