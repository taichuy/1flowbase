use super::*;

#[test]
fn wrapped_upstream_error_preserves_exact_body_and_kind() {
    let body = " \n{\"message\":\"keep complete body\"}\n ";
    let original = ProviderRuntimeError::new(ProviderRuntimeErrorKind::ProviderUpstreamError, body)
        .with_provider_details(json!({ "status_code": 500 }));
    let wrapped = anyhow::Error::new(ExtensionContractError::runtime(original))
        .context("provider invocation failed");

    let recovered = provider_runtime_error_from_anyhow(&wrapped);
    assert_eq!(
        recovered.kind,
        ProviderRuntimeErrorKind::ProviderUpstreamError
    );
    assert_eq!(recovered.message, body);
    assert_eq!(
        recovered.provider_details,
        Some(json!({ "status_code": 500 }))
    );
}
