use super::*;
#[test]
fn model_discovery_preserves_complete_descriptors_and_provider_metadata() {
    let op = ApiEventOperationContext {
        interface_id: "model_providers.instances.models.view".into(),
        method: "GET".into(),
        path: "/api/console/settings/model-providers/instances/:id/models".into(),
    };
    let models = (0..80).map(|i|ProviderModelDescriptorResponse { model_id:format!("model-{i}"), display_name:"Model".repeat(200), namespace:None, label_key:None, description_key:None, display_name_fallback:None, source:"remote".into(), supports_streaming:true, supports_tool_call:true, supports_multimodal:false, context_window:Some(128000), max_output_tokens:Some(8192), provider_metadata:json!({"secret_label":"Public label","values":(0..90).collect::<Vec<_>>()}) }).collect();
    let response = ModelProviderModelCatalogResponse {
        provider_instance_id: "provider-a".into(),
        refresh_status: "ready".into(),
        source: "remote".into(),
        last_error_message: None,
        refreshed_at: None,
        models,
    };
    let expected = serde_json::to_value(&response).unwrap();
    assert_eq!(
        ProviderDiscoveryOutput::Models(response)
            .project_document_output(&op)
            .unwrap(),
        expected
    );
    assert_eq!(
        ProviderDiscoveryInput::Models {
            id: "provider-a".into()
        }
        .project_document_input(&op)
        .unwrap(),
        json!({"path":{"id":"provider-a"}})
    );
}
