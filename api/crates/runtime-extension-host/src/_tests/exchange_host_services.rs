use super::*;
use extension_contracts::*;
use runtime_core::runtime_backend::*;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Mutex,
};
struct Logs(Mutex<Vec<ApplicationLogsBinding>>);
impl ApplicationLogsPort for Logs {
    fn execute<'a>(
        &'a self,
        binding: &'a ApplicationLogsBinding,
        _: &'a ApplicationLogsRequest,
    ) -> HostServiceFuture<'a, ApplicationLogsResponse> {
        Box::pin(async move {
            self.0.lock().unwrap().push(binding.clone());
            Err(HostServiceError {
                code: "agent_logs_partial_import".into(),
                status: Some(409),
                details: Some(serde_json::json!({"accepted_events":3})),
            })
        })
    }
}
fn binding() -> ApplicationLogsBinding {
    ApplicationLogsBinding {
        subject:ManagedContributionSubject::new(ManagedInstallationId::new("installation").unwrap(),ManagedWorkspaceId::new("system").unwrap(),ContributionId::new("logs").unwrap()),
        publisher_namespace:"trusted".into(),plugin_code:"fixture".into(),plugin_version:"1.0.0".into(),
        caller:RuntimeCallerContext {actor:serde_json::from_value(serde_json::json!({"user_id":"00000000-0000-0000-0000-000000000001","tenant_id":"00000000-0000-0000-0000-000000000000","current_workspace_id":"00000000-0000-0000-0000-000000000002","effective_display_role":"root","is_root":true,"permissions":[]})).unwrap(),session_version:7,session_identity:None,interface_id:"fixture.logs".into(),selected_mcp_instance_ids:None},
        deadline_unix_ms:now_unix_ms()+5000,
    }
}
fn context() -> ProviderHostCallContext {
    super::credential_tests::context(Arc::new(super::credential_tests::Credentials(Mutex::new(
        vec![],
    ))))
}
#[tokio::test]
async fn exchange_host_services_require_live_caller_and_preserve_partial_error() {
    let port = Arc::new(Logs(Mutex::new(vec![])));
    let request = serde_json::json!({"operation":"list_applications","application_types":[]});
    assert_eq!(
        execute_callback(
            MultiplexHostService::ApplicationLogsV1,
            request.clone(),
            None
        )
        .await["error"]["code"],
        "host_service_not_granted"
    );
    let mut expired = context();
    let mut b = binding();
    b.deadline_unix_ms = now_unix_ms() - 1;
    expired.application_logs = Some((b, port.clone()));
    assert_eq!(
        execute_callback(
            MultiplexHostService::ApplicationLogsV1,
            request.clone(),
            Some(expired)
        )
        .await["error"]["code"],
        "host_service_deadline_exceeded"
    );
    let mut granted = context();
    granted.application_logs = Some((binding(), port.clone()));
    let mut forged = request.clone();
    forged["actor"] = serde_json::json!("root");
    assert_eq!(
        execute_callback(
            MultiplexHostService::ApplicationLogsV1,
            forged,
            Some(granted.clone())
        )
        .await["error"]["code"],
        "host_service_request_invalid"
    );
    let response = execute_callback(
        MultiplexHostService::ApplicationLogsV1,
        request,
        Some(granted),
    )
    .await;
    assert_eq!(response["error"]["status"], 409);
    assert_eq!(response["error"]["details"]["accepted_events"], 3);
    let calls = port.0.lock().unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(
        calls[0].caller.actor.current_workspace_id.to_string(),
        "00000000-0000-0000-0000-000000000002"
    );
}
struct PendingHttp {
    entered: Arc<tokio::sync::Notify>,
    dropped: Arc<AtomicBool>,
}
struct DropFlag(Arc<AtomicBool>);
impl Drop for DropFlag {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}
impl HttpEgressPort for PendingHttp {
    fn execute<'a>(
        &'a self,
        _: &'a HttpEgressBinding,
        _: &'a HttpEgressRequest,
    ) -> HostServiceFuture<'a, HttpEgressResponse> {
        Box::pin(async move {
            let _guard = DropFlag(self.dropped.clone());
            self.entered.notify_one();
            std::future::pending().await
        })
    }
}
#[tokio::test]
async fn exchange_host_services_cancel_and_deadline_drop_active_http_work() {
    for cancel in [false, true] {
        let dropped = Arc::new(AtomicBool::new(false));
        let entered = Arc::new(tokio::sync::Notify::new());
        let port = Arc::new(PendingHttp {
            entered: entered.clone(),
            dropped: dropped.clone(),
        });
        let mut c = context();
        let mut b = binding();
        b.deadline_unix_ms = now_unix_ms() + if cancel { 5000 } else { 100 };
        c.http_egress = Some((b, port));
        let task = tokio::spawn(execute_callback(
            MultiplexHostService::HttpEgressV1,
            serde_json::json!({"method":"GET","url":"https://example.com/","headers":{},"body_base64":null}),
            Some(c),
        ));
        entered.notified().await;
        if cancel {
            task.abort();
            assert!(task.await.unwrap_err().is_cancelled());
        } else {
            assert_eq!(
                task.await.unwrap()["error"]["code"],
                "host_service_deadline_exceeded"
            );
        }
        assert!(dropped.load(Ordering::SeqCst));
    }
}

#[test]
fn exchange_host_services_preserve_large_results_without_truncation() {
    let result = exchange_service_response(Ok(ApplicationLogsResponse::Exported {
        otlp: serde_json::json!({"complete_fact":"a".repeat(32 * 1024 * 1024 + 1)}),
        record_ids: vec!["r".into()],
        trace_ids: vec!["t".into()],
        exported_spans: 1,
    }));
    assert!(result.get("error").is_none());
    assert_eq!(
        result["result"]["otlp"]["complete_fact"]
            .as_str()
            .unwrap()
            .len(),
        32 * 1024 * 1024 + 1
    );
}
