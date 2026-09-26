use std::{cell::RefCell, future::pending, rc::Rc};

use extension_contracts::{
    MultiplexEnvelope, MultiplexHostMessage, MultiplexWorkerMessage, MULTIPLEX_MAX_FRAME_BYTES,
};
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use crate::{serve_io, MultiplexError};

fn line(message: MultiplexHostMessage) -> String {
    format!(
        "{}\n",
        serde_json::to_string(&MultiplexEnvelope::new(message)).unwrap()
    )
}

#[tokio::test(flavor = "current_thread")]
async fn multiplex_runs_calls_concurrently_and_correlates_out_of_order_responses() {
    tokio::task::LocalSet::new()
        .run_until(async {
            let (host, worker) = tokio::io::duplex(4096);
            let (worker_read, worker_write) = tokio::io::split(worker);
            let (host_read, mut host_write) = tokio::io::split(host);
            let runner = tokio::task::spawn_local(serve_io(
                worker_read,
                worker_write,
                |request, _| async move {
                    if request == json!("slow") {
                        tokio::task::yield_now().await;
                    }
                    request
                },
            ));
            host_write
                .write_all(
                    line(MultiplexHostMessage::Call {
                        call_id: "1".into(),
                        request: json!("slow"),
                    })
                    .as_bytes(),
                )
                .await
                .unwrap();
            host_write
                .write_all(
                    line(MultiplexHostMessage::Call {
                        call_id: "2".into(),
                        request: json!("fast"),
                    })
                    .as_bytes(),
                )
                .await
                .unwrap();
            let mut reader = BufReader::new(host_read);
            let mut first = String::new();
            let mut second = String::new();
            reader.read_line(&mut first).await.unwrap();
            reader.read_line(&mut second).await.unwrap();
            let a: MultiplexEnvelope<MultiplexWorkerMessage> =
                serde_json::from_str(&first).unwrap();
            let b: MultiplexEnvelope<MultiplexWorkerMessage> =
                serde_json::from_str(&second).unwrap();
            assert_eq!(
                a.message,
                MultiplexWorkerMessage::Response {
                    call_id: "2".into(),
                    response: json!("fast")
                }
            );
            assert_eq!(
                b.message,
                MultiplexWorkerMessage::Response {
                    call_id: "1".into(),
                    response: json!("slow")
                }
            );
            host_write.shutdown().await.unwrap();
            assert!(runner.await.unwrap().is_ok());
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn multiplex_cancel_waits_for_task_exit_and_emits_only_cancelled() {
    tokio::task::LocalSet::new()
        .run_until(async {
            let (host, worker) = tokio::io::duplex(4096);
            let (worker_read, worker_write) = tokio::io::split(worker);
            let (host_read, mut host_write) = tokio::io::split(host);
            let dropped = Rc::new(RefCell::new(false));
            let observed = dropped.clone();
            let runner =
                tokio::task::spawn_local(serve_io(worker_read, worker_write, move |_, _| {
                    let dropped = dropped.clone();
                    async move {
                        struct Exit(Rc<RefCell<bool>>);
                        impl Drop for Exit {
                            fn drop(&mut self) {
                                *self.0.borrow_mut() = true;
                            }
                        }
                        let _exit = Exit(dropped);
                        pending::<()>().await;
                        Value::Null
                    }
                }));
            host_write
                .write_all(
                    line(MultiplexHostMessage::Call {
                        call_id: "3".into(),
                        request: Value::Null,
                    })
                    .as_bytes(),
                )
                .await
                .unwrap();
            tokio::task::yield_now().await;
            host_write
                .write_all(
                    line(MultiplexHostMessage::Cancel {
                        call_id: "3".into(),
                    })
                    .as_bytes(),
                )
                .await
                .unwrap();
            let mut response = String::new();
            BufReader::new(host_read)
                .read_line(&mut response)
                .await
                .unwrap();
            let frame: MultiplexEnvelope<MultiplexWorkerMessage> =
                serde_json::from_str(&response).unwrap();
            assert_eq!(
                frame.message,
                MultiplexWorkerMessage::Cancelled {
                    call_id: "3".into()
                }
            );
            assert!(*observed.borrow());
            host_write.shutdown().await.unwrap();
            assert!(runner.await.unwrap().is_ok());
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn multiplex_rejects_duplicate_id_and_oversized_frame() {
    tokio::task::LocalSet::new()
        .run_until(async {
            let (host, worker) = tokio::io::duplex(4096);
            let (worker_read, worker_write) = tokio::io::split(worker);
            let (_, mut host_write) = tokio::io::split(host);
            let runner =
                tokio::task::spawn_local(serve_io(worker_read, worker_write, |_, _| async {
                    pending::<Value>().await
                }));
            let call = line(MultiplexHostMessage::Call {
                call_id: "1".into(),
                request: Value::Null,
            });
            host_write.write_all(call.as_bytes()).await.unwrap();
            host_write.write_all(call.as_bytes()).await.unwrap();
            assert!(matches!(
                runner.await.unwrap(),
                Err(MultiplexError::Correlation)
            ));

            let (host, worker) = tokio::io::duplex(4096);
            let (worker_read, worker_write) = tokio::io::split(worker);
            let (_, mut host_write) = tokio::io::split(host);
            let runner =
                tokio::task::spawn_local(serve_io(worker_read, worker_write, |_, _| async {
                    Value::Null
                }));
            let writer = tokio::task::spawn_local(async move {
                let _ = host_write
                    .write_all(&vec![b'x'; MULTIPLEX_MAX_FRAME_BYTES + 1])
                    .await;
            });
            assert!(matches!(
                runner.await.unwrap(),
                Err(MultiplexError::FrameTooLarge)
            ));
            writer.await.unwrap();
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn synchronous_events_report_bounded_backpressure() {
    tokio::task::LocalSet::new()
        .run_until(async {
            let (host, worker) = tokio::io::duplex(64);
            let (worker_read, worker_write) = tokio::io::split(worker);
            let (_, mut host_write) = tokio::io::split(host);
            let (signal_tx, signal_rx) = tokio::sync::oneshot::channel();
            let signal_tx = Rc::new(RefCell::new(Some(signal_tx)));
            let runner =
                tokio::task::spawn_local(serve_io(worker_read, worker_write, move |_, emitter| {
                    let signal_tx = signal_tx.clone();
                    async move {
                        let mut full = false;
                        for _ in 0..40 {
                            if matches!(
                                emitter.try_event(json!("e".repeat(2 * 1024 * 1024))),
                                Err(MultiplexError::Backpressure)
                            ) {
                                full = true;
                                break;
                            }
                        }
                        signal_tx.borrow_mut().take().unwrap().send(full).unwrap();
                        Value::Null
                    }
                }));
            host_write
                .write_all(
                    line(MultiplexHostMessage::Call {
                        call_id: "1".into(),
                        request: Value::Null,
                    })
                    .as_bytes(),
                )
                .await
                .unwrap();
            assert!(signal_rx.await.unwrap());
            runner.abort();
            let _ = runner.await;
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn late_cancel_after_response_keeps_neighbor_alive() {
    tokio::task::LocalSet::new()
        .run_until(async {
            let (host, worker) = tokio::io::duplex(4096);
            let (worker_read, worker_write) = tokio::io::split(worker);
            let (host_read, mut host_write) = tokio::io::split(host);
            let runner = tokio::task::spawn_local(serve_io(
                worker_read,
                worker_write,
                |request, _| async move { request },
            ));
            host_write
                .write_all(
                    line(MultiplexHostMessage::Call {
                        call_id: "1".into(),
                        request: json!(1),
                    })
                    .as_bytes(),
                )
                .await
                .unwrap();
            let mut reader = BufReader::new(host_read);
            let mut first = String::new();
            reader.read_line(&mut first).await.unwrap();
            let frame: MultiplexEnvelope<MultiplexWorkerMessage> =
                serde_json::from_str(&first).unwrap();
            assert_eq!(
                frame.message,
                MultiplexWorkerMessage::Response {
                    call_id: "1".into(),
                    response: json!(1)
                }
            );
            host_write
                .write_all(
                    line(MultiplexHostMessage::Cancel {
                        call_id: "1".into(),
                    })
                    .as_bytes(),
                )
                .await
                .unwrap();
            host_write
                .write_all(
                    line(MultiplexHostMessage::Call {
                        call_id: "2".into(),
                        request: json!(2),
                    })
                    .as_bytes(),
                )
                .await
                .unwrap();
            let mut second = String::new();
            reader.read_line(&mut second).await.unwrap();
            let frame: MultiplexEnvelope<MultiplexWorkerMessage> =
                serde_json::from_str(&second).unwrap();
            assert_eq!(
                frame.message,
                MultiplexWorkerMessage::Response {
                    call_id: "2".into(),
                    response: json!(2)
                }
            );
            host_write.shutdown().await.unwrap();
            assert!(runner.await.unwrap().is_ok());
        })
        .await;
}

#[test]
fn multiplex_wire_rejects_unknown_control_kind() {
    let unknown = r#"{"protocol":"stdio_json_multiplex_v1","kind":"shutdown","call_id":"a"}"#;
    assert!(serde_json::from_str::<MultiplexEnvelope<MultiplexHostMessage>>(unknown).is_err());
}

#[tokio::test(flavor = "current_thread")]
async fn future_cancel_is_rejected_without_running_a_handler() {
    tokio::task::LocalSet::new()
        .run_until(async {
            let (host, worker) = tokio::io::duplex(4096);
            let (worker_read, worker_write) = tokio::io::split(worker);
            let (_, mut host_write) = tokio::io::split(host);
            let runner =
                tokio::task::spawn_local(serve_io(worker_read, worker_write, |_, _| async {
                    Value::Null
                }));
            host_write
                .write_all(
                    line(MultiplexHostMessage::Cancel {
                        call_id: "1".into(),
                    })
                    .as_bytes(),
                )
                .await
                .unwrap();
            assert!(matches!(
                runner.await.unwrap(),
                Err(MultiplexError::Correlation)
            ));
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn multiplex_accepts_response_larger_than_one_mib() {
    tokio::task::LocalSet::new().run_until(async {
        let (host, worker) = tokio::io::duplex(64 * 1024);
        let (worker_read, worker_write) = tokio::io::split(worker);
        let (host_read, mut host_write) = tokio::io::split(host);
        let runner = tokio::task::spawn_local(serve_io(worker_read, worker_write, |request, _| async move { request }));
        let payload = "x".repeat(2 * 1024 * 1024);
        host_write.write_all(line(MultiplexHostMessage::Call { call_id: "1".into(), request: json!(payload) }).as_bytes()).await.unwrap();
        let mut response = String::new();
        BufReader::new(host_read).read_line(&mut response).await.unwrap();
        let frame: MultiplexEnvelope<MultiplexWorkerMessage> = serde_json::from_str(&response).unwrap();
        assert!(matches!(frame.message, MultiplexWorkerMessage::Response { call_id, response } if call_id == "1" && response.as_str().is_some_and(|value| value.len() == 2 * 1024 * 1024)));
        host_write.shutdown().await.unwrap();
        assert!(runner.await.unwrap().is_ok());
    }).await;
}

#[test]
fn encoded_frame_holds_shared_byte_budget_until_dropped() {
    use std::sync::Arc;
    use tokio::sync::Semaphore;

    let budget = Arc::new(Semaphore::new(1024));
    let frame = crate::multiplex::encode(
        MultiplexWorkerMessage::Event {
            call_id: "1".into(),
            event: json!("x"),
        },
        &budget,
    )
    .unwrap();
    assert!(budget.available_permits() < 1024);
    drop(frame);
    assert_eq!(budget.available_permits(), 1024);
}

#[tokio::test(flavor = "current_thread")]
async fn partial_frame_at_eof_is_protocol_error() {
    tokio::task::LocalSet::new()
        .run_until(async {
            let (host, worker) = tokio::io::duplex(128);
            let (worker_read, worker_write) = tokio::io::split(worker);
            let (_, mut host_write) = tokio::io::split(host);
            let runner =
                tokio::task::spawn_local(serve_io(worker_read, worker_write, |_, _| async {
                    Value::Null
                }));
            host_write.write_all(b"{\"protocol\":").await.unwrap();
            host_write.shutdown().await.unwrap();
            assert!(matches!(
                runner.await.unwrap(),
                Err(MultiplexError::Protocol)
            ));
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn panicking_handler_fails_worker_and_releases_pending_neighbor() {
    tokio::task::LocalSet::new()
        .run_until(async {
            let (host, worker) = tokio::io::duplex(4096);
            let (worker_read, worker_write) = tokio::io::split(worker);
            let (_, mut host_write) = tokio::io::split(host);
            let retained = Rc::new(vec![0_u8; 1024 * 1024]);
            let retained_weak = Rc::downgrade(&retained);
            let released = Rc::new(RefCell::new(false));
            let observed_release = released.clone();
            let (started_tx, started_rx) = tokio::sync::oneshot::channel();
            let started_tx = Rc::new(RefCell::new(Some(started_tx)));
            let runner =
                tokio::task::spawn_local(serve_io(worker_read, worker_write, move |request, _| {
                    let retained = retained.clone();
                    let released = released.clone();
                    let started_tx = started_tx.clone();
                    async move {
                        if request == json!("panic") {
                            panic!("handler fixture panic");
                        }
                        struct Held {
                            released: Rc<RefCell<bool>>,
                            _payload: Rc<Vec<u8>>,
                        }
                        impl Drop for Held {
                            fn drop(&mut self) {
                                *self.released.borrow_mut() = true;
                            }
                        }
                        let _held = Held {
                            released,
                            _payload: retained,
                        };
                        started_tx.borrow_mut().take().unwrap().send(()).unwrap();
                        pending::<Value>().await
                    }
                }));
            host_write
                .write_all(
                    line(MultiplexHostMessage::Call {
                        call_id: "1".into(),
                        request: json!("neighbor"),
                    })
                    .as_bytes(),
                )
                .await
                .unwrap();
            started_rx.await.unwrap();
            host_write
                .write_all(
                    line(MultiplexHostMessage::Call {
                        call_id: "2".into(),
                        request: json!("panic"),
                    })
                    .as_bytes(),
                )
                .await
                .unwrap();
            let result = tokio::time::timeout(std::time::Duration::from_secs(2), runner)
                .await
                .expect("panic must terminate runner before Host deadline")
                .unwrap();
            assert!(matches!(result, Err(MultiplexError::HandlerPanicked)));
            assert!(*observed_release.borrow());
            assert!(retained_weak.upgrade().is_none());
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn small_completion_burst_exceeds_old_count_capacity_without_backpressure() {
    use std::{cell::Cell, collections::HashSet};

    const CALLS: usize = 64;
    tokio::task::LocalSet::new()
        .run_until(async {
            let (host, worker) = tokio::io::duplex(16 * 1024);
            let (worker_read, worker_write) = tokio::io::split(worker);
            let (host_read, mut host_write) = tokio::io::split(host);
            let started = Rc::new(Cell::new(0));
            let (all_started_tx, all_started_rx) = tokio::sync::oneshot::channel();
            let all_started_tx = Rc::new(RefCell::new(Some(all_started_tx)));
            let (release_tx, release_rx) = tokio::sync::watch::channel(false);
            let runner =
                tokio::task::spawn_local(serve_io(worker_read, worker_write, move |request, _| {
                    let started = started.clone();
                    let all_started_tx = all_started_tx.clone();
                    let mut release_rx = release_rx.clone();
                    async move {
                        let count = started.get() + 1;
                        started.set(count);
                        if count == CALLS {
                            all_started_tx
                                .borrow_mut()
                                .take()
                                .unwrap()
                                .send(())
                                .unwrap();
                        }
                        release_rx.changed().await.unwrap();
                        request
                    }
                }));
            for id in 1..=CALLS {
                host_write
                    .write_all(
                        line(MultiplexHostMessage::Call {
                            call_id: id.to_string(),
                            request: json!(id),
                        })
                        .as_bytes(),
                    )
                    .await
                    .unwrap();
            }
            tokio::time::timeout(std::time::Duration::from_secs(5), all_started_rx)
                .await
                .unwrap()
                .unwrap();
            release_tx.send_replace(true);
            host_write.shutdown().await.unwrap();
            let mut reader = BufReader::new(host_read);
            let mut seen = HashSet::new();
            for _ in 0..CALLS {
                let mut response = String::new();
                tokio::time::timeout(
                    std::time::Duration::from_secs(5),
                    reader.read_line(&mut response),
                )
                .await
                .unwrap()
                .unwrap();
                let frame: MultiplexEnvelope<MultiplexWorkerMessage> =
                    serde_json::from_str(&response).unwrap();
                let MultiplexWorkerMessage::Response { call_id, response } = frame.message else {
                    panic!("expected terminal response");
                };
                assert_eq!(response, json!(call_id.parse::<usize>().unwrap()));
                assert!(seen.insert(call_id));
            }
            assert_eq!(seen.len(), CALLS);
            assert!(runner.await.unwrap().is_ok());
        })
        .await;
}
