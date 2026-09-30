use crate::{serve_io, MultiplexError};
use extension_contracts::{MultiplexEnvelope, MultiplexHostMessage, MultiplexWorkerMessage};
use serde_json::{json, Value};
use std::{
    cell::RefCell,
    io,
    pin::Pin,
    rc::Rc,
    task::{Context, Poll},
    time::Duration,
};
use tokio::{
    io::{AsyncWrite, AsyncWriteExt},
    sync::watch,
};

#[derive(Default)]
struct OutputState {
    bytes: Vec<u8>,
    visible: usize,
    writes: usize,
    flushes: usize,
}
struct Output {
    state: Rc<RefCell<OutputState>>,
    changed: watch::Sender<usize>,
    partial: usize,
    fail_flush: bool,
}
impl AsyncWrite for Output {
    fn poll_write(
        self: Pin<&mut Self>,
        _: &mut Context<'_>,
        bytes: &[u8],
    ) -> Poll<io::Result<usize>> {
        let mut state = self.state.borrow_mut();
        let count = bytes.len().min(self.partial);
        state.bytes.extend_from_slice(&bytes[..count]);
        state.writes += 1;
        Poll::Ready(Ok(count))
    }
    fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
        if self.fail_flush {
            return Poll::Ready(Err(io::Error::other("fixture flush failed")));
        }
        let mut state = self.state.borrow_mut();
        state.flushes += 1;
        state.visible = state.bytes.len();
        self.changed.send_replace(state.visible);
        Poll::Ready(Ok(()))
    }
    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.poll_flush(cx)
    }
}
fn line(message: MultiplexHostMessage) -> Vec<u8> {
    let mut bytes = serde_json::to_vec(&MultiplexEnvelope::new(message)).unwrap();
    bytes.push(b'\n');
    bytes
}
fn output(
    partial: usize,
    fail_flush: bool,
) -> (Output, Rc<RefCell<OutputState>>, watch::Receiver<usize>) {
    let state = Rc::new(RefCell::new(OutputState::default()));
    let (changed, receiver) = watch::channel(0);
    (
        Output {
            state: state.clone(),
            changed,
            partial,
            fail_flush,
        },
        state,
        receiver,
    )
}
fn frames(state: &Rc<RefCell<OutputState>>) -> Vec<MultiplexWorkerMessage> {
    let state = state.borrow();
    assert_eq!(
        state.visible,
        state.bytes.len(),
        "EOF must flush every byte"
    );
    assert_eq!(state.bytes.last(), Some(&b'\n'));
    state.bytes[..state.visible]
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
        .map(|line| {
            serde_json::from_slice::<MultiplexEnvelope<MultiplexWorkerMessage>>(line)
                .unwrap()
                .message
        })
        .collect()
}
async fn burst(
    partial: usize,
    fail_flush: bool,
) -> (Result<(), MultiplexError>, Rc<RefCell<OutputState>>) {
    let (writer, state, _) = output(partial, fail_flush);
    let input = line(MultiplexHostMessage::Call {
        call_id: "1".into(),
        request: Value::Null,
    });
    let result = serve_io(
        std::io::Cursor::new(input),
        writer,
        |_, emitter| async move {
            for index in 0..1024 {
                emitter
                    .try_event(json!({"index":index,"text":"完整🙂é"}))
                    .unwrap();
            }
            json!({"complete":true})
        },
    )
    .await;
    (result, state)
}
fn exact_burst(state: &Rc<RefCell<OutputState>>) {
    let messages = frames(state);
    assert_eq!(messages.len(), 1025);
    for (index, message) in messages[..1024].iter().enumerate() {
        assert_eq!(
            message,
            &MultiplexWorkerMessage::Event {
                call_id: "1".into(),
                event: json!({"index":index,"text":"完整🙂é"})
            }
        );
    }
    assert_eq!(
        messages[1024],
        MultiplexWorkerMessage::Response {
            call_id: "1".into(),
            response: json!({"complete":true})
        }
    );
}
#[tokio::test(flavor = "current_thread")]
async fn ready_event_burst_is_exact_and_amortizes_physical_writes() {
    tokio::task::LocalSet::new()
        .run_until(async {
            let (result, state) = burst(usize::MAX, false).await;
            result.unwrap();
            exact_burst(&state);
            let state = state.borrow();
            assert!(
                state.writes < 1024 / 16,
                "ready tiny frames must not each force a physical write: {}",
                state.writes
            );
            assert!(
                state.flushes < 1024 / 16,
                "ready tiny frames must not each force a physical flush: {}",
                state.flushes
            );
        })
        .await;
}
#[tokio::test(flavor = "current_thread")]
async fn batched_output_preserves_partial_write_utf8_and_terminal() {
    tokio::task::LocalSet::new()
        .run_until(async {
            let (result, state) = burst(7, false).await;
            result.unwrap();
            exact_burst(&state);
        })
        .await;
}
#[tokio::test(flavor = "current_thread")]
async fn one_idle_event_flushes_before_waiting_for_cancel() {
    tokio::task::LocalSet::new()
        .run_until(async {
            let (writer, state, mut visible) = output(usize::MAX, false);
            let (mut host, worker) = tokio::io::duplex(4096);
            let runner =
                tokio::task::spawn_local(serve_io(worker, writer, |_, emitter| async move {
                    emitter.try_event(json!({"text":"idle-first"})).unwrap();
                    std::future::pending::<Value>().await
                }));
            host.write_all(&line(MultiplexHostMessage::Call {
                call_id: "1".into(),
                request: Value::Null,
            }))
            .await
            .unwrap();
            tokio::time::timeout(Duration::from_secs(1), visible.changed())
                .await
                .unwrap()
                .unwrap();
            let bytes = {
                let state = state.borrow();
                state.bytes[..state.visible].to_vec()
            };
            let event: MultiplexEnvelope<MultiplexWorkerMessage> =
                serde_json::from_slice(&bytes).unwrap();
            assert_eq!(
                event.message,
                MultiplexWorkerMessage::Event {
                    call_id: "1".into(),
                    event: json!({"text":"idle-first"})
                }
            );
            host.write_all(&line(MultiplexHostMessage::Cancel {
                call_id: "1".into(),
            }))
            .await
            .unwrap();
            host.shutdown().await.unwrap();
            tokio::time::timeout(Duration::from_secs(1), runner)
                .await
                .unwrap()
                .unwrap()
                .unwrap();
            assert_eq!(
                frames(&state).last(),
                Some(&MultiplexWorkerMessage::Cancelled {
                    call_id: "1".into()
                })
            );
        })
        .await;
}
#[tokio::test(flavor = "current_thread")]
async fn failed_output_flush_is_not_reported_as_successful_eof() {
    tokio::task::LocalSet::new()
        .run_until(async {
            let (result, _) = burst(usize::MAX, true).await;
            assert!(
                result.is_err(),
                "failed physical flush cannot become successful EOF: {result:?}"
            );
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn buffered_callback_flushes_and_keeps_call_identity() {
    tokio::task::LocalSet::new()
        .run_until(async {
            let (writer, state, mut visible) = output(usize::MAX, false);
            let (mut host, worker) = tokio::io::duplex(4096);
            let runner =
                tokio::task::spawn_local(serve_io(worker, writer, |_, emitter| async move {
                    for index in 0..32 {
                        emitter.try_event(json!({"index":index})).unwrap();
                    }
                    let result = emitter
                        .callback(
                            extension_contracts::MultiplexHostService::PluginDataV1,
                            json!({"fixture":"callback"}),
                        )
                        .await
                        .unwrap();
                    json!({"callback_result":result})
                }));
            host.write_all(&line(MultiplexHostMessage::Call {
                call_id: "1".into(),
                request: Value::Null,
            }))
            .await
            .unwrap();
            tokio::time::timeout(Duration::from_secs(1), visible.changed())
                .await
                .unwrap()
                .unwrap();
            let messages = frames(&state);
            assert_eq!(messages.len(), 33);
            let MultiplexWorkerMessage::Callback {
                call_id,
                callback_id,
                service,
                request,
            } = messages.last().unwrap()
            else {
                panic!("callback missing");
            };
            assert_eq!(call_id, "1");
            assert_eq!(
                service,
                &extension_contracts::MultiplexHostService::PluginDataV1
            );
            assert_eq!(request, &json!({"fixture":"callback"}));
            host.write_all(&line(MultiplexHostMessage::CallbackResult {
                call_id: call_id.clone(),
                callback_id: callback_id.clone(),
                response: json!({"value":"完整🙂"}),
            }))
            .await
            .unwrap();
            host.shutdown().await.unwrap();
            tokio::time::timeout(Duration::from_secs(1), runner)
                .await
                .unwrap()
                .unwrap()
                .unwrap();
            assert_eq!(
                frames(&state).last(),
                Some(&MultiplexWorkerMessage::Response {
                    call_id: "1".into(),
                    response: json!({"callback_result":{"value":"完整🙂"}})
                })
            );
        })
        .await;
}
