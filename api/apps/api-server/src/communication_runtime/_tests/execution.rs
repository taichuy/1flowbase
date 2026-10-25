use super::*;

#[test]
fn interrupted_send_is_unknown_and_preflight_never_claims_delivery() {
    assert!(matches!(
        interrupted(&AtomicU8::new(0), "host_stopped"),
        Some(CommunicationProgress::RunFailed { .. })
    ));
    assert!(
        matches!(interrupted(&AtomicU8::new(2),"channel_disabled"), Some(CommunicationProgress::SendFailed { failure_code }) if failure_code=="channel_disabled")
    );
    assert!(
        matches!(interrupted(&AtomicU8::new(3),"host_stopped"), Some(CommunicationProgress::SendUnknown { failure_code }) if failure_code=="provider_result_unknown")
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

#[tokio::test]
async fn linked_run_recovery_never_creates_a_second_run() {
    let run_id = Uuid::now_v7();
    let starts = AtomicU8::new(0);
    for _ in 0..3 {
        let observed = linked_or_start(Some(run_id), || async {
            starts.fetch_add(1, Ordering::SeqCst);
            anyhow::bail!("a linked run cannot be created twice")
        })
        .await
        .unwrap();
        assert_eq!(observed, run_id);
    }
    assert_eq!(starts.load(Ordering::SeqCst), 0);
    let created = linked_or_start(None, || async {
        starts.fetch_add(1, Ordering::SeqCst);
        Ok(run_id)
    })
    .await
    .unwrap();
    assert_eq!(created, run_id);
    assert_eq!(starts.load(Ordering::SeqCst), 1);
}

#[test]
fn waiting_and_observer_disconnect_do_not_fail_a_linked_application_run() {
    use domain::FlowRunStatus as Status;
    for status in [
        Status::Queued,
        Status::Running,
        Status::WaitingCallback,
        Status::WaitingHuman,
        Status::Paused,
    ] {
        assert_eq!(run_disposition(status), RunDisposition::Observe);
    }
    for reason in [
        "host_stopped",
        "claim_lost",
        "communication_execution_failed",
    ] {
        assert!(interrupted(&AtomicU8::new(1), reason).is_none());
    }
    assert_eq!(
        run_disposition(Status::Succeeded),
        RunDisposition::FinalAnswer
    );
    assert_eq!(
        run_disposition(Status::Failed),
        RunDisposition::Failed("run_failed")
    );
    assert_eq!(
        run_disposition(Status::Cancelled),
        RunDisposition::Failed("run_cancelled")
    );
}
