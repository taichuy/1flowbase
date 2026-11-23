use super::*;
use crate::serve_io;
use serde_json::json;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

#[tokio::test]
async fn workspace_governance_client_correlates_typed_callbacks() {
    tokio::task::LocalSet::new().run_until(exchange()).await;
}

async fn send(write: &mut (impl AsyncWriteExt + Unpin), message: MultiplexHostMessage) {
    let mut bytes = serde_json::to_vec(&MultiplexEnvelope::new(message)).unwrap();
    bytes.push(b'\n');
    write.write_all(&bytes).await.unwrap();
}

async fn exchange() {
    let (host, worker) = tokio::io::duplex(8192);
    let (read, write) = tokio::io::split(worker);
    let runner = tokio::task::spawn_local(serve_io(read, write, |_, emitter| async move {
        let client = WorkspaceGovernanceClient::new(emitter);
        let result = client
            .inspect_quota_counts("tenant".into(), "workspace".into())
            .await
            .unwrap();
        assert_eq!(result.active_member_count, 2);
        let caller = client.inspect_caller_context().await.unwrap().unwrap();
        assert_eq!(caller.effective_display_role, "admin");
        assert!(!caller.is_root);
        assert!(client.inspect_caller_context().await.unwrap().is_none());
        assert!(matches!(
            client.list_tenants_and_workspaces().await,
            Err(WorkspaceGovernanceClientError::InvalidResponse)
        ));
        json!({"ok":true})
    }));
    let (read, mut write) = tokio::io::split(host);
    let mut lines = BufReader::new(read).lines();
    send(
        &mut write,
        MultiplexHostMessage::Call {
            call_id: "1".into(),
            request: json!({}),
        },
    )
    .await;
    let counts = json!({"operation":"quota_counts","active_member_count":2,"tenant_workspace_count":3,"workspace_application_count":4});
    let bindings = [
        ("inspect_quota_counts", counts.clone()),
        (
            "inspect_caller_context",
            json!({"operation":"caller_context", "caller": {"user_id":"u","tenant_id":"t","current_workspace_id":"w","effective_display_role":"admin","is_root":false}}),
        ),
        (
            "inspect_caller_context",
            json!({"operation":"caller_context", "caller": null}),
        ),
        ("list_tenants_and_workspaces", counts),
    ];
    for (index, (operation, result)) in bindings.into_iter().enumerate() {
        let envelope: MultiplexEnvelope<MultiplexWorkerMessage> =
            serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
        let MultiplexWorkerMessage::Callback {
            call_id,
            callback_id,
            service,
            request,
        } = envelope.message
        else {
            panic!("expected host callback")
        };
        assert_eq!(call_id, "1");
        assert_eq!(callback_id, (index + 1).to_string());
        assert_eq!(service, MultiplexHostService::WorkspaceGovernanceV1);
        assert_eq!(request["operation"], operation);
        if operation == "inspect_caller_context" {
            assert_eq!(request, json!({"operation":operation}));
        }
        send(
            &mut write,
            MultiplexHostMessage::CallbackResult {
                call_id,
                callback_id,
                response: json!({"result":result}),
            },
        )
        .await;
    }
    let response: MultiplexEnvelope<MultiplexWorkerMessage> =
        serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
    assert!(matches!(
        response.message,
        MultiplexWorkerMessage::Response { .. }
    ));
    write.shutdown().await.unwrap();
    runner.await.unwrap().unwrap();
}
