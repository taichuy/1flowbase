use super::super::actor::run_observed_connection_loop;
use crate::routes::application_public_api::client_observer::_tests::{raw_bytes, seeded_flow_run};
use axum::{extract::ws::WebSocketUpgrade, routing::get, Router};
use futures_util::{SinkExt, StreamExt};
use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};
use storage_durable_postgres::PgControlPlaneStore;
use tokio::sync::{mpsc, Notify};
use tokio_tungstenite::tungstenite::Message;

#[tokio::test]
async fn client_websocket_capture_keeps_original_envelopes_and_unique_queued_turns() {
    let (pool, flow) = seeded_flow_run().await;
    let store = PgControlPlaneStore::new(pool);
    tokio::time::timeout(Duration::from_secs(15), async {
        let (observed, mut captures) = mpsc::unbounded_channel();
        let count = Arc::new(AtomicUsize::new(0));
        let release = Arc::new(Notify::new());
        let bridge_release = release.clone();
        let repository = Arc::new(store.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let router = Router::new().route("/ws", get(move |upgrade: WebSocketUpgrade| {
            let observed = observed.clone();
            let count = count.clone();
            let release = bridge_release.clone();
            let repository = repository.clone();
            async move {
                upgrade.on_upgrade(move |socket| run_observed_connection_loop(socket, move |mut request, frames, recorder| {
                    let observed = observed.clone();
                    let count = count.clone();
                    let release = release.clone();
                    async move {
                        let recorder = recorder.expect("production observer enabled");
                        recorder.bind_run(flow, None);
                        assert!(request.get("type").is_none(), "decoder removes only the internal envelope");
                        // Internal translation is allowed to force streaming; evidence is immutable.
                        request["stream"] = serde_json::json!(true);
                        let index = count.fetch_add(1, Ordering::SeqCst);
                        observed.send(recorder).unwrap();
                        frames.send(format!("{{\"type\":\"response.completed\",\"response\":{{\"id\":\"resp_{index}\",\"output\":[]}}}}" )).await.unwrap();
                        if index == 0 { release.notified().await; }
                        Ok(())
                    }
                }, None, Some(repository)))
            }
        }));
        let server = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
        let (mut socket, _) = tokio_tungstenite::connect_async(format!("ws://{address}/ws")).await.unwrap();
        let first = " { \"type\":\"response.create\", \"input\":\"你好  🌏\" } \n";
        let second = "{\"type\":\"response.create\",\"input\":[],\"stream\":false}";
        socket.send(Message::Text(first.into())).await.unwrap();
        let capture1 = captures.recv().await.unwrap();
        let output1 = socket.next().await.unwrap().unwrap().into_text().unwrap();
        socket.send(Message::Text(second.into())).await.unwrap();
        // The next raw request may queue but dispatch waits for the prior completion owner.
        assert!(tokio::time::timeout(Duration::from_millis(50), captures.recv()).await.is_err());
        release.notify_one();
        let capture2 = captures.recv().await.unwrap();
        let output2 = socket.next().await.unwrap().unwrap().into_text().unwrap();
        capture1.wait_finished().await;
        capture2.wait_finished().await;
        assert_ne!(capture1.capture_id(), capture2.capture_id());
        assert_eq!(raw_bytes(&store, flow, capture1.capture_id(), "submitted").await, first.as_bytes());
        assert_eq!(raw_bytes(&store, flow, capture2.capture_id(), "submitted").await, second.as_bytes());
        assert_eq!(raw_bytes(&store, flow, capture1.capture_id(), "emitted").await, output1.as_bytes());
        assert_eq!(raw_bytes(&store, flow, capture2.capture_id(), "emitted").await, output2.as_bytes());
        let captures_count: i64 = sqlx::query_scalar("select count(*) from client_trajectory_captures where flow_run_id=$1").bind(flow).fetch_one(store.pool()).await.unwrap();
        assert_eq!(captures_count, 2);
        socket.close(None).await.unwrap();
        server.abort();
    }).await.unwrap();
}
