use crate::{serve_io, PluginExecutionGateClient, PluginExecutionGateClientError};
use extension_contracts::{
    MultiplexEnvelope, MultiplexHostMessage, MultiplexHostService, MultiplexWorkerMessage,
};
use serde_json::json;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

#[tokio::test]
async fn execution_gate_typed_client_uses_correlated_acquire_release_and_host_errors() {
    tokio::task::LocalSet::new().run_until(async {
        let (host, worker) = tokio::io::duplex(8192);
        let (worker_read, worker_write) = tokio::io::split(worker);
        let runner = tokio::task::spawn_local(serve_io(worker_read, worker_write, |_, emitter| async move {
            let client = PluginExecutionGateClient::new(emitter);
            let permit = client.acquire("entry".into()).await.unwrap();
            permit.release().await.unwrap();
            match client.acquire("denied".into()).await {
                Err(PluginExecutionGateClientError::Host(error)) => assert_eq!(error.code, "execution_gate_authority_denied"),
                _ => panic!("expected typed authority error"),
            }
            assert!(matches!(client.acquire("malformed".into()).await, Err(PluginExecutionGateClientError::InvalidResponse)));
            json!({"ok":true})
        }));
        let (host_read, mut host_write) = tokio::io::split(host);
        let mut reader = BufReader::new(host_read);
        let mut line = serde_json::to_vec(&MultiplexEnvelope::new(MultiplexHostMessage::Call { call_id: "1".into(), request: json!({}) })).unwrap();
        line.push(b'\n'); host_write.write_all(&line).await.unwrap();
        for (index, expected) in [json!({"operation":"acquire","key":"entry"}), json!({"operation":"release","key":"entry","token":"opaque-host-token"}), json!({"operation":"acquire","key":"denied"}), json!({"operation":"acquire","key":"malformed"})].into_iter().enumerate() {
            let mut line = String::new(); reader.read_line(&mut line).await.unwrap();
            let envelope: MultiplexEnvelope<MultiplexWorkerMessage> = serde_json::from_str(&line).unwrap();
            let MultiplexWorkerMessage::Callback { call_id, callback_id, service, request } = envelope.message else { panic!("expected callback") };
            assert_eq!(service, MultiplexHostService::PluginExecutionGateV1);
            assert_eq!(request, expected);
            assert_eq!(call_id, "1"); assert_eq!(callback_id, (index+1).to_string());
            let response = match index {
                0 => json!({"result":{"operation":"acquire","token":"opaque-host-token"}}),
                1 => json!({"result":{"operation":"release"}}),
                2 => json!({"error":{"code":"execution_gate_authority_denied"}}),
                _ => json!({"result":{"operation":"acquire","token":""}, "error":{"code":"ambiguous"}}),
            };
            let mut line = serde_json::to_vec(&MultiplexEnvelope::new(MultiplexHostMessage::CallbackResult { call_id, callback_id, response })).unwrap();
            line.push(b'\n'); host_write.write_all(&line).await.unwrap();
        }
        let mut line = String::new(); reader.read_line(&mut line).await.unwrap();
        let envelope: MultiplexEnvelope<MultiplexWorkerMessage> = serde_json::from_str(&line).unwrap();
        assert!(matches!(envelope.message, MultiplexWorkerMessage::Response { .. }));
        host_write.shutdown().await.unwrap(); runner.await.unwrap().unwrap();
    }).await;
}
