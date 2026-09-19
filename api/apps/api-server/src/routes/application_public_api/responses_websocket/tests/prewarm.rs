use std::{sync::Arc, time::Duration};

use axum::{extract::ws::WebSocketUpgrade, routing::get, Router};
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::sync::{mpsc, Mutex};
use tokio_tungstenite::tungstenite::Message;

use super::super::{actor::run_connection_loop, turn_bridge::ResponsesTurnBridgeError};

#[tokio::test]
async fn generate_false_executes_bridge_and_standard_continuation_stays_provider_owned() {
    tokio::time::timeout(Duration::from_secs(5), async {
        let (requests, mut observed) = mpsc::unbounded_channel();
        let sequence = Arc::new(Mutex::new(0_u8));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let router = Router::new().route(
            "/ws",
            get(move |upgrade: WebSocketUpgrade| {
                let requests = requests.clone();
                let sequence = sequence.clone();
                async move {
                    upgrade.on_upgrade(move |socket| {
                        run_connection_loop(socket, move |request, frames| {
                            let requests = requests.clone();
                            let sequence = sequence.clone();
                            async move {
                                requests.send(request).unwrap();
                                let mut current = sequence.lock().await;
                                *current += 1;
                                let response_id = format!("resp_provider_{}", *current);
                                frames
                                    .send(
                                        json!({
                                            "type": "response.completed",
                                            "response": {
                                                "id": response_id,
                                                "status": "completed"
                                            }
                                        })
                                        .to_string(),
                                    )
                                    .await
                                    .unwrap();
                                Ok::<_, ResponsesTurnBridgeError>(())
                            }
                        })
                    })
                }
            }),
        );
        let server = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
        let (mut socket, _) = tokio_tungstenite::connect_async(format!("ws://{address}/ws"))
            .await
            .unwrap();

        socket
            .send(Message::Text(
                json!({
                    "type": "response.create",
                    "model": "published-model",
                    "input": [{"role": "developer", "content": "provider-owned prefix"}],
                    "generate": false
                })
                .to_string(),
            ))
            .await
            .unwrap();
        let prewarm = observed
            .recv()
            .await
            .expect("generate=false must invoke execute bridge");
        assert_eq!(prewarm["generate"], false);
        let first_terminal: Value =
            serde_json::from_str(socket.next().await.unwrap().unwrap().to_text().unwrap()).unwrap();
        assert_eq!(first_terminal["response"]["id"], "resp_provider_1");

        socket
            .send(Message::Text(
                json!({
                    "type": "response.create",
                    "model": "published-model",
                    "previous_response_id": "resp_provider_1",
                    "input": "new delta"
                })
                .to_string(),
            ))
            .await
            .unwrap();
        let continuation = observed
            .recv()
            .await
            .expect("second turn must invoke execute bridge");
        assert_eq!(continuation["previous_response_id"], "resp_provider_1");
        assert_eq!(continuation["input"], "new delta");
        assert_eq!(continuation.as_object().unwrap().len(), 3);

        socket.close(None).await.unwrap();
        server.abort();
    })
    .await
    .expect("prewarm and continuation must complete within the bounded test window");
}

#[tokio::test]
async fn delivered_failure_is_not_replaced_by_late_success_or_transport_close() {
    use super::super::actor::run_connection_loop_with_terminations;
    use crate::provider_runtime::TransportTerminationNotice;

    tokio::time::timeout(Duration::from_secs(5), async {
        let (notices, _) = tokio::sync::broadcast::channel(4);
        let route_notices = notices.clone();
        let finish = Arc::new(tokio::sync::Notify::new());
        let route_finish = finish.clone();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let router = Router::new().route("/ws", get(move |upgrade: WebSocketUpgrade| {
            let termination = route_notices.subscribe();
            let finish = route_finish.clone();
            async move {
                upgrade.on_upgrade(move |socket| {
                    run_connection_loop_with_terminations(socket, move |_, frames| {
                        let finish = finish.clone();
                        async move {
                            frames.send(json!({"type":"response.failed","response":{"status":"failed","error":{"code":"original-provider-error"}}}).to_string()).await.unwrap();
                            frames.send(json!({"type":"response.completed","response":{"status":"completed"}}).to_string()).await.unwrap();
                            finish.notified().await;
                            Ok::<_, ResponsesTurnBridgeError>(())
                        }
                    }, Some(("owner-fixture".into(), termination)))
                })
            }
        }));
        let server = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
        let (mut socket, _) = tokio_tungstenite::connect_async(format!("ws://{address}/ws")).await.unwrap();
        socket.send(Message::Text(json!({"type":"response.create","model":"model","input":[]}).to_string())).await.unwrap();
        let first: Value = serde_json::from_str(socket.next().await.unwrap().unwrap().to_text().unwrap()).unwrap();
        assert_eq!(first["type"], "response.failed");
        assert_eq!(first["response"]["error"]["code"], "original-provider-error");
        // Receiving the first terminal is the causal barrier before physical close.
        notices.send(TransportTerminationNotice { owner_id:"owner-fixture".into(), code:"provider_connection_max_age" }).unwrap();
        assert!(matches!(socket.next().await.unwrap().unwrap(), Message::Close(_)), "close must not append another success or failure terminal");
        let _ = socket.close(None).await;
        finish.notify_one();
        server.abort();
        let _ = server.await;
    }).await.expect("terminal-once fixture must finish within its bound");
}
