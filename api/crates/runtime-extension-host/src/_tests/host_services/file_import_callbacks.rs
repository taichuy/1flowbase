use super::*;
use extension_contracts::{
    ContributionId, FileImportRequest, FileImportResponse, ManagedContributionSubject,
    ManagedInstallationId, ManagedWorkspaceId,
};
use runtime_core::runtime_backend::{
    FileImportBinding, FileImportFuture, FileImportPort, RuntimeCallerContext,
};
use std::sync::Mutex;

struct Imports(Mutex<Vec<FileImportBinding>>);
impl FileImportPort for Imports {
    fn execute<'a>(
        &'a self,
        binding: &'a FileImportBinding,
        _: &'a FileImportRequest,
    ) -> FileImportFuture<'a> {
        Box::pin(async move {
            self.0.lock().unwrap().push(binding.clone());
            Ok(FileImportResponse {
                file_table_id: "table".into(),
                storage_id: "storage".into(),
                record: serde_json::json!({"filename":"a.bin"}),
            })
        })
    }
}
fn context(port: Arc<Imports>) -> ProviderHostCallContext {
    let mut context = super::credential_tests::context(Arc::new(
        super::credential_tests::Credentials(Mutex::new(vec![])),
    ));
    context.file_import = Some((FileImportBinding {
        subject: ManagedContributionSubject::new(ManagedInstallationId::new("installation").unwrap(), ManagedWorkspaceId::new("system").unwrap(), ContributionId::new("import").unwrap()),
        publisher_namespace: "trusted".into(), plugin_code: "fixture".into(), plugin_version: "1.0.0".into(),
        caller: RuntimeCallerContext { actor: serde_json::from_value(serde_json::json!({"user_id":"00000000-0000-0000-0000-000000000001","tenant_id":"00000000-0000-0000-0000-000000000000","current_workspace_id":"00000000-0000-0000-0000-000000000002","effective_display_role":"root","is_root":true,"permissions":[]})).unwrap(), session_version: 7, session_identity: None, interface_id: "fixture".into(), selected_mcp_instance_ids: Some(vec!["selected".into()]) },
        deadline_unix_ms: now_unix_ms() + 5000,
    }, port));
    context
}
#[tokio::test]
async fn import_callback_uses_only_trusted_binding_and_denies_missing_expired_or_forged_context() {
    let port = Arc::new(Imports(Mutex::new(vec![])));
    let request = serde_json::json!({"resource_uri":"flowbase-resource://read/a","file_table_id":"00000000-0000-0000-0000-000000000003"});
    let result = execute_callback(
        MultiplexHostService::FileImportV1,
        request.clone(),
        Some(context(port.clone())),
    )
    .await;
    assert_eq!(result["result"]["record"]["filename"], "a.bin");
    {
        let calls = port.0.lock().unwrap();
        assert_eq!(calls[0].caller.session_version, 7);
        assert_eq!(
            calls[0].caller.selected_mcp_instance_ids.as_deref(),
            Some(["selected".to_owned()].as_slice())
        );
    }
    for field in ["actor", "workspace_id", "installation_id"] {
        let mut forged = request.clone();
        forged[field] = serde_json::json!("forged");
        let result = execute_callback(
            MultiplexHostService::FileImportV1,
            forged,
            Some(context(port.clone())),
        )
        .await;
        assert_eq!(result["error"]["code"], "file_import_request_invalid");
    }
    let result = execute_callback(MultiplexHostService::FileImportV1, request.clone(), None).await;
    assert_eq!(result["error"]["code"], "file_import_not_granted");
    let mut expired = context(port.clone());
    expired.file_import.as_mut().unwrap().0.deadline_unix_ms = 0;
    let result = execute_callback(MultiplexHostService::FileImportV1, request, Some(expired)).await;
    assert_eq!(result["error"]["code"], "file_import_deadline_exceeded");
    assert_eq!(port.0.lock().unwrap().len(), 1);
}

struct WaitingImport(Arc<std::sync::atomic::AtomicBool>);
impl FileImportPort for WaitingImport {
    fn execute<'a>(
        &'a self,
        _: &'a FileImportBinding,
        _: &'a FileImportRequest,
    ) -> FileImportFuture<'a> {
        Box::pin(async move {
            use std::sync::atomic::Ordering;
            struct Active(Arc<std::sync::atomic::AtomicBool>);
            impl Drop for Active {
                fn drop(&mut self) {
                    self.0.store(false, Ordering::SeqCst);
                }
            }
            self.0.store(true, Ordering::SeqCst);
            let _active = Active(self.0.clone());
            std::future::pending().await
        })
    }
}
#[tokio::test]
async fn callback_deadline_and_cancellation_drop_the_exact_active_import_future() {
    use std::sync::atomic::{AtomicBool, Ordering};
    let active = Arc::new(AtomicBool::new(false));
    let mut deadline_context = context(Arc::new(Imports(Mutex::new(vec![]))));
    deadline_context.file_import.as_mut().unwrap().1 = Arc::new(WaitingImport(active.clone()));
    deadline_context
        .file_import
        .as_mut()
        .unwrap()
        .0
        .deadline_unix_ms = now_unix_ms() + 20;
    let request = serde_json::json!({"resource_uri":"flowbase-resource://read/a","file_table_id":"00000000-0000-0000-0000-000000000003"});
    let result = execute_callback(
        MultiplexHostService::FileImportV1,
        request.clone(),
        Some(deadline_context),
    )
    .await;
    assert_eq!(result["error"]["code"], "file_import_deadline_exceeded");
    assert!(!active.load(Ordering::SeqCst));
    let mut cancel_context = context(Arc::new(Imports(Mutex::new(vec![]))));
    cancel_context.file_import.as_mut().unwrap().1 = Arc::new(WaitingImport(active.clone()));
    let task = tokio::spawn(execute_callback(
        MultiplexHostService::FileImportV1,
        request,
        Some(cancel_context),
    ));
    while !active.load(Ordering::SeqCst) {
        tokio::task::yield_now().await;
    }
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    assert!(!active.load(Ordering::SeqCst));
}

#[tokio::test]
async fn file_read_callback_without_injected_grant_is_denied() {
    let response = execute_callback(MultiplexHostService::FileReadV1, serde_json::json!({"operation":"open","file_table_id":"00000000-0000-0000-0000-000000000001","record_id":"00000000-0000-0000-0000-000000000002"}), None).await;
    assert_eq!(response["error"]["code"], "file_read_not_granted");
}
