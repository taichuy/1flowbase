use super::*;

#[test]
fn upstream_failure_keeps_exact_body_through_runtime_context() {
    let body = " \n{\"future_error\":{\"shape\":\"unknown\"}}\n ";
    let runtime = plugin_framework::ProviderRuntimeError::new(
        plugin_framework::ProviderRuntimeErrorKind::ProviderUpstreamError,
        body,
    )
    .with_provider_summary(body);
    let error = anyhow::Error::new(plugin_framework::PluginFrameworkError::runtime(runtime))
        .context("provider invocation failed");

    assert_eq!(serde_error_payload(&error), json!({ "message": body }));
}
