use crate::{serve_io, PluginCredentialClient};
use extension_contracts::{
    MultiplexEnvelope, MultiplexHostMessage, MultiplexHostService, MultiplexWorkerMessage,
};
use serde_json::json;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

#[tokio::test]
async fn credential_client_uses_shared_correlated_callbacks() {
    tokio::task::LocalSet::new()
        .run_until(async {
            let (host, worker) = tokio::io::duplex(8192);
            let (worker_read, worker_write) = tokio::io::split(worker);
            let runner = tokio::task::spawn_local(serve_io(
                worker_read,
                worker_write,
                |_, emitter| async move {
                    let client = PluginCredentialClient::new(emitter);
                    client
                        .put("server/password".into(), json!({"password":"fixture-only"}))
                        .await
                        .unwrap();
                    assert_eq!(
                        client.get("server/password".into()).await.unwrap(),
                        Some(json!({"password":"fixture-only"}))
                    );
                    client.delete("server/password".into()).await.unwrap();
                    assert!(client.get("forbidden".into()).await.is_err());
                    json!({"ok":true})
                },
            ));
            let (host_read, mut host_write) = tokio::io::split(host);
            let mut reader = BufReader::new(host_read);
            let mut line =
                serde_json::to_vec(&MultiplexEnvelope::new(MultiplexHostMessage::Call {
                    call_id: "1".into(),
                    request: json!({}),
                }))
                .unwrap();
            line.push(b'\n');
            host_write.write_all(&line).await.unwrap();
            for (index, expected) in ["put", "get", "delete", "get"].into_iter().enumerate() {
                let mut line = String::new();
                reader.read_line(&mut line).await.unwrap();
                let envelope: MultiplexEnvelope<MultiplexWorkerMessage> =
                    serde_json::from_str(&line).unwrap();
                let MultiplexWorkerMessage::Callback {
                    call_id,
                    callback_id,
                    service,
                    request,
                } = envelope.message
                else {
                    panic!("expected callback")
                };
                assert_eq!(service, MultiplexHostService::PluginCredentialV1);
                assert_eq!(request["operation"], expected);
                assert_eq!(
                    request.as_object().unwrap().len(),
                    if expected == "put" { 3 } else { 2 }
                );
                assert_eq!(callback_id, (index + 1).to_string());
                let response = match index {
                    1 => json!({"result":{"value":{"password":"fixture-only"}}}),
                    3 => json!({"error":{"code":"plugin_credential_not_granted"}}),
                    _ => json!({"result":{"value":null}}),
                };
                let mut line = serde_json::to_vec(&MultiplexEnvelope::new(
                    MultiplexHostMessage::CallbackResult {
                        call_id,
                        callback_id,
                        response,
                    },
                ))
                .unwrap();
                line.push(b'\n');
                host_write.write_all(&line).await.unwrap();
            }
            let mut line = String::new();
            reader.read_line(&mut line).await.unwrap();
            let envelope: MultiplexEnvelope<MultiplexWorkerMessage> =
                serde_json::from_str(&line).unwrap();
            assert!(matches!(
                envelope.message,
                MultiplexWorkerMessage::Response { .. }
            ));
            host_write.shutdown().await.unwrap();
            runner.await.unwrap().unwrap();
        })
        .await;
}
