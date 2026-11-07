use super::*;
#[test]
fn icon_has_real_path_input_and_explicit_binary_output_phase() {
    let op = ApiEventOperationContext {
        interface_id: "model_providers.icons.view".into(),
        method: "GET".into(),
        path: "/api/console/model-providers/providers/:provider_code/icon".into(),
    };
    assert_eq!(
        ProviderIconInput("provider-a".into())
            .project_document_input(&op)
            .unwrap(),
        json!({"path":{"provider_code":"provider-a"}})
    );
    assert_eq!(ProviderIconInput::document_input_unavailable(&op), None);
    assert_eq!(
        ProviderIconOutput::document_output_unavailable(&op),
        Some(domain::event_center::ApiEventUnavailableReason::BinaryResponse)
    );
    assert!(ProviderIconOutput {
        content_type: "image/png",
        content: b"BINARY-ICON-SENTINEL".to_vec()
    }
    .project_document_output(&op)
    .is_err());
}
