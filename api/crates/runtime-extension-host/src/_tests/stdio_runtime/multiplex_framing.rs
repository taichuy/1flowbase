use super::*;
use serde_json::json;
use std::{
    future::Future,
    task::{Context, Poll, Waker},
};
use tokio::io::{AsyncWriteExt, BufReader};

fn encode(message: MultiplexWorkerMessage) -> Vec<u8> {
    let mut frame = serde_json::to_vec(&MultiplexEnvelope::new(message)).unwrap();
    frame.push(b'\n');
    frame
}

fn response() -> MultiplexWorkerMessage {
    MultiplexWorkerMessage::Response {
        call_id: "1".into(),
        response: json!({
            "text": "完整🙂\u{0000}".repeat(128 * 1024),
            "items": [
                {"type":"function_call", "call_id":"工具-1", "arguments":"{\"路径\":\"🙂\"}"},
                {"type":"function_call_output", "call_id":"工具-1", "output":"结果"},
                {"type":"reasoning", "encrypted_content":"opaque+/=密文", "summary":[]},
                {"type":"input_image", "image_url":"https://example.invalid/image"},
                {"type":"input_audio", "file_id":"audio-reference"},
                {"type":"input_file", "file_id":"file-reference"}
            ],
            "future_field": {"keep": [null, true, 1, "unknown"]}
        }),
    }
}

#[tokio::test]
async fn completed_frames_release_scratch_and_preserve_all_payload_fields() {
    let expected = response();
    let large = encode(expected.clone());
    let small_message = MultiplexWorkerMessage::Cancelled {
        call_id: "2".into(),
    };
    let small = encode(small_message.clone());
    let mut stream = large.clone();
    stream.extend_from_slice(&small);
    let mut reader = BufReader::new(std::io::Cursor::new(stream));
    let mut scratch = Vec::new();

    let decoded = read_frame(&mut reader, &mut scratch)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(decoded, (expected, large.len()));
    assert_eq!(
        scratch.capacity(),
        0,
        "completed history must not remain owned by scratch"
    );
    assert_eq!(
        read_frame(&mut reader, &mut scratch).await.unwrap(),
        Some((small_message, small.len()))
    );
    assert_eq!(scratch.capacity(), 0);
    assert_eq!(read_frame(&mut reader, &mut scratch).await.unwrap(), None);
}

#[tokio::test]
async fn every_partial_utf8_boundary_survives_a_cancelled_read_future() {
    let expected = MultiplexWorkerMessage::Response {
        call_id: "3".into(),
        response: json!({"text":"中文🙂\u{0000}"}),
    };
    let frame = encode(expected.clone());
    for split in 0..frame.len() {
        let (mut writer, stream) = tokio::io::duplex(frame.len() + 1);
        writer.write_all(&frame[..split]).await.unwrap();
        let mut reader = BufReader::new(stream);
        let mut scratch = Vec::new();
        let mut interrupted = Box::pin(read_frame(&mut reader, &mut scratch));
        assert!(matches!(
            interrupted
                .as_mut()
                .poll(&mut Context::from_waker(Waker::noop())),
            Poll::Pending
        ));
        drop(interrupted);
        assert_eq!(scratch, &frame[..split], "partial frame at byte {split}");
        writer.write_all(&frame[split..]).await.unwrap();
        let decoded = read_frame(&mut reader, &mut scratch)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(decoded, (expected.clone(), frame.len()), "split {split}");
        assert_eq!(scratch.capacity(), 0);
    }
}

#[tokio::test]
async fn all_worker_frame_variants_keep_order_and_exact_byte_accounting() {
    let messages = vec![
        MultiplexWorkerMessage::ResourceRetention {
            sequence: 1,
            retained_resources: 2,
        },
        MultiplexWorkerMessage::Event {
            call_id: "1".into(),
            event: json!({"type":"text_delta","delta":"🙂"}),
            await_ack: true,
            event_id: Some("1".into()),
        },
        MultiplexWorkerMessage::Callback {
            call_id: "1".into(),
            callback_id: "1".into(),
            service: extension_contracts::MultiplexHostService::FileReadV1,
            request: json!({"file_id":"opaque"}),
        },
        MultiplexWorkerMessage::Cancelled {
            call_id: "1".into(),
        },
        MultiplexWorkerMessage::Response {
            call_id: "2".into(),
            response: json!({"ok":false,"error":{"code":"upstream_error","details":{"retry_after":2,"unknown":"preserve"}}}),
        },
    ];
    let stream = messages
        .iter()
        .cloned()
        .flat_map(encode)
        .collect::<Vec<_>>();
    let mut reader = BufReader::with_capacity(3, std::io::Cursor::new(stream));
    let mut scratch = Vec::new();
    for message in messages {
        let expected_bytes = encode(message.clone()).len();
        assert_eq!(
            read_frame(&mut reader, &mut scratch).await.unwrap(),
            Some((message, expected_bytes))
        );
        assert_eq!(scratch.capacity(), 0);
    }
    assert_eq!(read_frame(&mut reader, &mut scratch).await.unwrap(), None);
}

#[tokio::test]
async fn partial_eof_malformed_and_unsupported_frames_remain_errors() {
    let mut scratch = Vec::new();
    let mut reader = BufReader::new(std::io::Cursor::new(b"{\"kind\":"));
    assert_eq!(
        read_frame(&mut reader, &mut scratch).await.unwrap_err(),
        "partial multiplex frame"
    );
    scratch.clear();
    let mut reader = BufReader::new(std::io::Cursor::new(b"invalid json\n"));
    assert!(read_frame(&mut reader, &mut scratch).await.is_err());
    assert_eq!(scratch.capacity(), 0);
    let mut reader = BufReader::new(std::io::Cursor::new(
        b"{\"protocol\":\"unsupported\",\"kind\":\"cancelled\",\"call_id\":\"1\"}\n",
    ));
    assert_eq!(
        read_frame(&mut reader, &mut scratch).await.unwrap_err(),
        "unsupported multiplex protocol"
    );
    assert_eq!(scratch.capacity(), 0);
}
