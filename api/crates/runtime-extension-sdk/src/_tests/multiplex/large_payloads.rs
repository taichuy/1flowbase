use crate::{serve_duplex_io, serve_io};
use extension_contracts::{
    MultiplexEnvelope, MultiplexHostMessage, MultiplexHostService, MultiplexWorkerMessage,
};
use serde_json::{json, Value};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

fn line(message: MultiplexHostMessage) -> Vec<u8> {
    let mut bytes = serde_json::to_vec(&MultiplexEnvelope::new(message)).unwrap();
    bytes.push(b'\n');
    bytes
}
async fn receive<R: tokio::io::AsyncBufRead + Unpin>(reader: &mut R) -> MultiplexWorkerMessage {
    let mut text = String::new();
    reader.read_line(&mut text).await.unwrap();
    serde_json::from_str::<MultiplexEnvelope<MultiplexWorkerMessage>>(&text)
        .unwrap()
        .message
}

#[tokio::test(flavor = "current_thread")]
async fn multiplex_large_request_and_terminal_exceed_both_old_frame_and_write_window() {
    tokio::task::LocalSet::new().run_until(async {
        tokio::time::timeout(Duration::from_secs(60), async {
            let (host, worker) = tokio::io::duplex(8192);
            let (r, w) = tokio::io::split(worker);
            let (r_host, mut w_host) = tokio::io::split(host);
            let runner = tokio::task::spawn_local(serve_io(r, w, |request, _| async move { request }));
            let original = format!("{}完整🙂", "x".repeat(64 * 1024 * 1024 + 1));
            w_host.write_all(&line(MultiplexHostMessage::Call { call_id:"1".into(), request:json!(original) })).await.unwrap();
            let frame = receive(&mut BufReader::new(r_host)).await;
            assert!(matches!(frame, MultiplexWorkerMessage::Response { response, .. } if response.as_str() == Some(original.as_str())));
            w_host.shutdown().await.unwrap();
            runner.await.unwrap().unwrap();
        }).await.unwrap();
    }).await;
}

#[tokio::test(flavor = "current_thread")]
async fn multiplex_synchronous_events_above_old_accumulated_budget_are_all_delivered() {
    tokio::task::LocalSet::new().run_until(async {
        tokio::time::timeout(Duration::from_secs(60), async {
            let (host, worker) = tokio::io::duplex(8192);
            let (r, w) = tokio::io::split(worker);
            let (r_host, mut w_host) = tokio::io::split(host);
            let runner = tokio::task::spawn_local(serve_io(r, w, |_, emitter| async move {
                for index in 0..40 { emitter.try_event(json!({"index":index,"text":"x".repeat(2 * 1024 * 1024)})).unwrap(); }
                json!({"done":40})
            }));
            w_host.write_all(&line(MultiplexHostMessage::Call { call_id:"1".into(), request:Value::Null })).await.unwrap();
            let mut reader = BufReader::new(r_host);
            for index in 0..40 {
                let MultiplexWorkerMessage::Event { event, .. } = receive(&mut reader).await else { panic!("event missing"); };
                assert_eq!(event["index"], index);
                assert_eq!(event["text"].as_str().unwrap().len(), 2 * 1024 * 1024);
            }
            assert!(matches!(receive(&mut reader).await, MultiplexWorkerMessage::Response { response, .. } if response == json!({"done":40})));
            w_host.shutdown().await.unwrap(); runner.await.unwrap().unwrap();
        }).await.unwrap();
    }).await;
}

#[tokio::test(flavor = "current_thread")]
async fn multiplex_large_acknowledged_event_and_callback_keep_progress() {
    tokio::task::LocalSet::new().run_until(async {
        tokio::time::timeout(Duration::from_secs(90), async {
            let (host, worker) = tokio::io::duplex(8192);
            let (r, w) = tokio::io::split(worker);
            let (r_host, mut w_host) = tokio::io::split(host);
            let runner = tokio::task::spawn_local(serve_duplex_io(r, w, |_, emitter, _| async move {
                emitter.event(json!("e".repeat(32 * 1024 * 1024 + 1))).await.unwrap();
                emitter.callback(MultiplexHostService::HttpEgressV1, json!("c".repeat(32 * 1024 * 1024 + 1))).await.unwrap()
            }));
            w_host.write_all(&line(MultiplexHostMessage::Call { call_id:"1".into(), request:Value::Null })).await.unwrap();
            let mut reader = BufReader::new(r_host);
            let MultiplexWorkerMessage::Event { event, event_id:Some(event_id), await_ack:true, .. } = receive(&mut reader).await else { panic!("event missing"); };
            assert_eq!(event.as_str().unwrap().len(), 32 * 1024 * 1024 + 1);
            drop(event);
            w_host.write_all(&line(MultiplexHostMessage::EventAck { call_id:"1".into(), event_id })).await.unwrap();
            let MultiplexWorkerMessage::Callback { callback_id, request, .. } = receive(&mut reader).await else { panic!("callback missing"); };
            assert_eq!(request.as_str().unwrap().len(), 32 * 1024 * 1024 + 1);
            drop(request);
            let result = "r".repeat(64 * 1024 * 1024 + 1);
            w_host.write_all(&line(MultiplexHostMessage::CallbackResult { call_id:"1".into(), callback_id, response:json!(result) })).await.unwrap();
            assert!(matches!(receive(&mut reader).await, MultiplexWorkerMessage::Response { response, .. } if response.as_str() == Some(result.as_str())));
            w_host.shutdown().await.unwrap(); runner.await.unwrap().unwrap();
        }).await.unwrap();
    }).await;
}
