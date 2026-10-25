use super::*;

#[test]
fn interrupted_send_is_unknown_and_preflight_never_claims_delivery() {
    assert!(matches!(
        interrupted(&AtomicU8::new(0), "host_stopped"),
        CommunicationProgress::RunFailed { .. }
    ));
    assert!(
        matches!(interrupted(&AtomicU8::new(2),"channel_disabled"), CommunicationProgress::SendFailed { failure_code } if failure_code=="channel_disabled")
    );
    assert!(
        matches!(interrupted(&AtomicU8::new(3),"host_stopped"), CommunicationProgress::SendUnknown { failure_code } if failure_code=="provider_result_unknown")
    );
}
#[test]
fn ambiguous_provider_or_storage_error_does_not_become_retryable_failure() {
    use control_plane::errors::ControlPlaneError as Error;
    for error in [
        anyhow::anyhow!("transport disconnected"),
        Error::UpstreamUnavailable("notification_provider_failed").into(),
    ] {
        assert!(matches!(
            send_failure(&error),
            CommunicationProgress::SendUnknown { .. }
        ));
    }
    assert!(
        matches!(send_failure(&Error::UpstreamUnavailable("provider_rejected").into()),CommunicationProgress::SendFailed { failure_code } if failure_code=="provider_rejected")
    );
    assert!(
        matches!(send_failure(&Error::Conflict("instance_disabled").into()),CommunicationProgress::SendFailed { failure_code } if failure_code=="instance_disabled")
    );
}
