use super::*;
#[tokio::test]
async fn exchange_host_services_api_error_retains_partial_import_receipt() {
    let error = control_plane::agent_logs::AgentLogsPartialImportError {
        accepted_events: 3,
        duplicate_events: 2,
        committed_records: 1,
    };
    let safe = safe_api_error(crate::error_response::ApiError(anyhow::Error::new(error))).await;
    assert_eq!(safe.code, "agent_logs_partial_import");
    assert_eq!(safe.status, Some(409));
    assert_eq!(safe.details.unwrap()["accepted_events"], 3);
}

#[test]
fn exchange_host_services_cleanup_failure_preserves_business_failure() {
    let partial = HostServiceError {
        code: "agent_logs_partial_import".into(),
        status: Some(409),
        details: Some(serde_json::json!({"accepted_events":3})),
    };
    let result =
        finish_authority::<()>(Err(partial.clone()), Err(anyhow::anyhow!("cleanup failed")));
    assert_eq!(result.unwrap_err(), partial);
    assert_eq!(
        finish_authority(Ok(()), Err(anyhow::anyhow!("cleanup failed")))
            .unwrap_err()
            .code,
        "host_service_authority_unavailable"
    );
}
