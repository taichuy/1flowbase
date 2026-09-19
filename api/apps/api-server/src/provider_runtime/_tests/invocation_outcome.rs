use super::*;
use plugin_framework::provider_contract::ProviderInvocationResult;
use serde_json::json;

fn output() -> ProviderRuntimeInvocationOutput {
    ProviderRuntimeInvocationOutput {
        events: Vec::new(),
        result: ProviderInvocationResult {
            provider_metadata: json!({}),
            ..Default::default()
        },
    }
}

fn original_error() -> ProviderRuntimeError {
    ProviderRuntimeError::new(
        ProviderRuntimeErrorKind::RateLimited,
        "original-provider-error",
    )
    .with_provider_summary("fixture-summary")
    .with_provider_details(json!({"status":429,"code":"fixture-quota"}))
}

fn assert_original(error: anyhow::Error) {
    let Some(PluginFrameworkError::RuntimeContract { error }) =
        error.downcast_ref::<PluginFrameworkError>()
    else {
        panic!("original typed error must remain downcastable");
    };
    assert_eq!(error, &original_error());
}

#[test]
fn first_provider_error_wins_over_result_delimiter_fencing_and_cleanup() {
    let mut output = output();
    output.events = vec![
        ProviderStreamEvent::Error {
            error: original_error(),
        },
        ProviderStreamEvent::Error {
            error: ProviderRuntimeError::new(
                ProviderRuntimeErrorKind::ProviderInvalidResponse,
                "secondary",
            ),
        },
    ];
    output.result.finish_reason = Some(ProviderFinishReason::Error);
    let primary = validate_provider_invocation_output(output);
    let fenced = preserve_provider_invocation_outcome(
        primary,
        Err(anyhow::anyhow!("provider_transport_stale_generation")),
    );
    let released = preserve_provider_invocation_outcome(
        fenced,
        Err(anyhow::anyhow!("non-typed release failure")),
    );
    assert_original(released.unwrap_err());
}

#[test]
fn failed_delimiter_without_diagnostic_is_not_success() {
    for event_only in [false, true] {
        let mut output = output();
        if event_only {
            output.events.push(ProviderStreamEvent::Finish {
                reason: ProviderFinishReason::Error,
            });
        } else {
            output.result.finish_reason = Some(ProviderFinishReason::Error);
        }
        let error = validate_provider_invocation_output(output).unwrap_err();
        let Some(PluginFrameworkError::RuntimeContract { error }) =
            error.downcast_ref::<PluginFrameworkError>()
        else {
            panic!("typed contract failure expected")
        };
        assert_eq!(
            error.kind,
            ProviderRuntimeErrorKind::ProviderInvalidResponse
        );
        assert_eq!(
            error.message,
            "provider invocation failed without an error diagnostic"
        );
    }
}

#[test]
fn successful_output_still_requires_successful_receipt_finalization() {
    assert!(validate_provider_invocation_output(output()).is_ok());
    let error = preserve_provider_invocation_outcome(
        Ok(output()),
        Err(anyhow::anyhow!("provider_transport_receipt_missing")),
    )
    .unwrap_err();
    assert_eq!(error.to_string(), "provider_transport_receipt_missing");
    assert_original(
        preserve_provider_invocation_outcome(
            Err(PluginFrameworkError::runtime(original_error()).into()),
            Ok(()),
        )
        .unwrap_err(),
    );
}
