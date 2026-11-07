use super::*;
fn operation(id: &str) -> ApiEventOperationContext {
    ApiEventOperationContext {
        interface_id: id.into(),
        method: "POST".into(),
        path: "/api/console/settings/model-providers/instances".into(),
    }
}
#[test]
fn provider_raw_credentials_and_preview_proof_are_not_before_fields() {
    let op = operation("model_providers.instances.create");
    let configured_models = (0..80)
        .map(|i| ConfiguredModelBody {
            model_id: format!("model-{i}"),
            enabled: true,
            context_window_override_tokens: Some(128000),
            supports_multimodal: Some(false),
            pricing_provider_code: "pricing".into(),
            pricing_model_id: "pricing-model".into(),
        })
        .collect();
    let input = ProviderInstanceLifecycleInput::Create(CreateModelProviderBody {
        installation_id: "installation-a".into(),
        display_name: "Provider".into(),
        configured_models,
        enabled_model_ids: vec!["model-0".into()],
        included_in_main: Some(true),
        preview_token: Some("PREVIEW-PROOF-SENTINEL".into()),
        config: json!({"api_key":"PROVIDER-KEY-SENTINEL"}),
    });
    let value = input.project_document_input(&op).unwrap();
    assert_eq!(
        value["body"]["configured_models"].as_array().unwrap().len(),
        80
    );
    assert_eq!(
        value["body"]["configured_models"][0]["context_window_override_tokens"],
        128000
    );
    assert!(!value.to_string().contains("SENTINEL"));
    assert!(value["body"].get("config").is_none());
    assert!(value["body"].get("preview_token").is_none());
    let schema = ProviderInstanceLifecycleInput::document_input_schema(&op)
        .unwrap()
        .to_string();
    assert!(!schema.contains("\"config\""));
    assert!(!schema.contains("preview_token"));
}
#[test]
fn instance_public_config_is_preserved_in_completion_without_secret_name_heuristics() {
    let op = operation("model_providers.instances.view");
    let instance = ModelProviderInstanceResponse {
        id: "provider-a".into(),
        installation_id: "installation-a".into(),
        provider_code: "provider".into(),
        protocol: "openai".into(),
        display_name: "Provider".repeat(200),
        status: "ready".into(),
        included_in_main: true,
        config_json: json!({"public":"endpoint","business_metadata":{"secret_label":"Public instruction","values":(0..100).collect::<Vec<_>>()}}),
        configured_models: vec![],
        enabled_model_ids: vec![],
        catalog_refresh_status: None,
        catalog_last_error_message: None,
        catalog_refreshed_at: None,
        model_count: 0,
    };
    let expected = serde_json::to_value(&instance).unwrap();
    assert_eq!(
        ProviderInstanceLifecycleOutput::Instance(instance)
            .project_document_output(&op)
            .unwrap(),
        expected
    );
}
