//! Bounded, current-thread worker runner for the versioned multiplex stdio transport.
//! Providers may retain borrowed, non-Send state; each call runs as a local Tokio task.
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    future::Future,
    io::{self, Read, Write},
    rc::Rc,
    sync::Arc,
};

use extension_contracts::{
    MultiplexEnvelope, MultiplexHostMessage, MultiplexHostService, MultiplexWorkerMessage,
    MULTIPLEX_MAX_FRAME_BYTES, MULTIPLEX_OUTPUT_BUDGET_BYTES,
};
use serde_json::Value;
use thiserror::Error;
use tokio::{
    io::{AsyncBufRead, AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader},
    sync::{mpsc, oneshot, watch, OwnedSemaphorePermit, Semaphore},
    task::{JoinHandle, LocalSet},
};

const COMPLETION_CHANNEL_CAPACITY: usize = 16;

type PendingCallbacks =
    Rc<RefCell<HashMap<(String, String), oneshot::Sender<(Value, OwnedSemaphorePermit)>>>>;

pub(crate) struct QueuedFrame {
    bytes: Vec<u8>,
    // Held until the writer finishes the write and flush, not merely until enqueue.
    _permit: OwnedSemaphorePermit,
}

struct BoundedJson {
    bytes: Vec<u8>,
    exceeded: bool,
}

impl Write for BoundedJson {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.bytes.len().saturating_add(bytes.len()) > MULTIPLEX_MAX_FRAME_BYTES {
            self.exceeded = true;
            return Err(io::Error::other("multiplex frame too large"));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum MultiplexError {
    #[error("worker I/O failed: {0}")]
    Io(#[from] io::Error),
    #[error("invalid multiplex JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("multiplex frame exceeds the byte limit")]
    FrameTooLarge,
    #[error("unsupported multiplex protocol or control message")]
    Protocol,
    #[error("invalid or duplicate call/callback id")]
    Correlation,
    #[error("worker output channel is full or closed")]
    Backpressure,
}

#[derive(Clone)]
pub struct MultiplexEmitter {
    call_id: String,
    output: mpsc::UnboundedSender<QueuedFrame>,
    byte_budget: Arc<Semaphore>,
    callbacks: PendingCallbacks,
    next_callback_id: Rc<Cell<u64>>,
    live: Rc<Cell<bool>>,
}

impl MultiplexEmitter {
    /// Synchronous event production never waits on stdout or blocks the executor.
    pub fn try_event(&self, event: Value) -> Result<(), MultiplexError> {
        if !self.live.get() {
            return Err(MultiplexError::Correlation);
        }
        let frame = encode(
            MultiplexWorkerMessage::Event {
                call_id: self.call_id.clone(),
                event,
            },
            &self.byte_budget,
        )?;
        self.output
            .send(frame)
            .map_err(|_| MultiplexError::Backpressure)
    }

    /// The Host resolves the service using this call's trusted binding. No identity can be
    /// supplied by the worker. A callback result is correlated to both IDs.
    pub async fn callback(
        &self,
        service: MultiplexHostService,
        request: Value,
    ) -> Result<Value, MultiplexError> {
        if !self.live.get() {
            return Err(MultiplexError::Correlation);
        }
        let next = self
            .next_callback_id
            .get()
            .checked_add(1)
            .ok_or(MultiplexError::Correlation)?;
        self.next_callback_id.set(next);
        let callback_id = next.to_string();
        let key = (self.call_id.clone(), callback_id.clone());
        let (tx, rx) = oneshot::channel();
        self.callbacks.borrow_mut().insert(key.clone(), tx);
        let frame = encode(
            MultiplexWorkerMessage::Callback {
                call_id: self.call_id.clone(),
                callback_id,
                service,
                request,
            },
            &self.byte_budget,
        );
        let frame = match frame {
            Ok(frame) => frame,
            Err(error) => {
                self.callbacks.borrow_mut().remove(&key);
                return Err(error);
            }
        };
        if self.output.send(frame).is_err() {
            self.callbacks.borrow_mut().remove(&key);
            return Err(MultiplexError::Backpressure);
        }
        let result = rx
            .await
            .map(|(value, _permit)| value)
            .map_err(|_| MultiplexError::Correlation);
        self.callbacks.borrow_mut().remove(&key);
        result
    }
}

fn encode_bytes(message: MultiplexWorkerMessage) -> Result<Vec<u8>, MultiplexError> {
    let mut buffer = BoundedJson {
        bytes: Vec::new(),
        exceeded: false,
    };
    if let Err(error) = serde_json::to_writer(&mut buffer, &MultiplexEnvelope::new(message)) {
        return Err(if buffer.exceeded {
            MultiplexError::FrameTooLarge
        } else {
            MultiplexError::Json(error)
        });
    }
    buffer.bytes.push(b'\n');
    Ok(buffer.bytes)
}

pub(crate) fn encode(
    message: MultiplexWorkerMessage,
    budget: &Arc<Semaphore>,
) -> Result<QueuedFrame, MultiplexError> {
    let bytes = encode_bytes(message)?;
    let permit = budget
        .clone()
        .try_acquire_many_owned(bytes.len() as u32)
        .map_err(|_| MultiplexError::Backpressure)?;
    Ok(QueuedFrame {
        bytes,
        _permit: permit,
    })
}

async fn encode_wait(
    message: MultiplexWorkerMessage,
    budget: &Arc<Semaphore>,
) -> Result<QueuedFrame, MultiplexError> {
    // Each outstanding handler owns at most one bounded terminal frame while
    // waiting; the Host retains that call's memory reservation until terminal.
    let bytes = encode_bytes(message)?;
    let permit = budget
        .clone()
        .acquire_many_owned(bytes.len() as u32)
        .await
        .map_err(|_| MultiplexError::Backpressure)?;
    Ok(QueuedFrame {
        bytes,
        _permit: permit,
    })
}

async fn read_frame<R: AsyncBufRead + Unpin>(
    reader: &mut R,
    bytes: &mut Vec<u8>,
) -> Result<Option<MultiplexHostMessage>, MultiplexError> {
    loop {
        let available = reader.fill_buf().await?;
        if available.is_empty() {
            if bytes.is_empty() {
                return Ok(None);
            }
            return Err(MultiplexError::Protocol);
        }
        let count = available
            .iter()
            .position(|byte| *byte == b'\n')
            .map_or(available.len(), |index| index + 1);
        // Only a terminating newline gets the extra byte. An unterminated
        // MAX+1 frame must fail now, without waiting for another byte or EOF.
        let terminated = available.get(count - 1) == Some(&b'\n');
        if bytes.len() + count > MULTIPLEX_MAX_FRAME_BYTES + usize::from(terminated) {
            return Err(MultiplexError::FrameTooLarge);
        }
        bytes.extend_from_slice(&available[..count]);
        reader.consume(count);
        if bytes.last() == Some(&b'\n') {
            bytes.pop();
            if bytes.len() > MULTIPLEX_MAX_FRAME_BYTES {
                return Err(MultiplexError::FrameTooLarge);
            }
            let frame: MultiplexEnvelope<MultiplexHostMessage> = serde_json::from_slice(&bytes)?;
            if !frame.is_supported() {
                return Err(MultiplexError::Protocol);
            }
            bytes.clear();
            return Ok(Some(frame.message));
        }
    }
}

fn remove_callbacks(callbacks: &PendingCallbacks, call_id: &str) {
    callbacks.borrow_mut().retain(|(id, _), _| id != call_id);
}

fn queue_output(
    output: &mpsc::UnboundedSender<QueuedFrame>,
    frame: QueuedFrame,
) -> Result<(), MultiplexError> {
    // The byte permit bounds this queue; a count of tiny messages is not capacity.
    output.send(frame).map_err(|_| MultiplexError::Backpressure)
}

/// Serve a worker's standard input/output using local tasks. Call from a Tokio runtime.
/// The handler owns its business concurrency and may serialize calls by session.
/// A handler returns the legacy typed response serialized to JSON Value.
pub async fn serve<H, Fut>(handler: H) -> Result<(), MultiplexError>
where
    H: Fn(Value, MultiplexEmitter) -> Fut + 'static,
    Fut: Future<Output = Value> + 'static,
{
    // Tokio stdin uses a blocking-pool read that cannot be cancelled while the Host keeps
    // the pipe open. A detached OS reader and bounded bridge let runner shutdown return.
    let (input_tx, mut input_rx) = mpsc::channel::<Vec<u8>>(4);
    std::thread::spawn(move || {
        let mut stdin = io::stdin();
        loop {
            let mut chunk = vec![0; 8192];
            match stdin.read(&mut chunk) {
                Ok(0) | Err(_) => break,
                Ok(size) => {
                    chunk.truncate(size);
                    if input_tx.blocking_send(chunk).is_err() {
                        break;
                    }
                }
            }
        }
    });
    let (bridge_read, mut bridge_write) = tokio::io::duplex(8192);
    LocalSet::new()
        .run_until(async move {
            let bridge = tokio::task::spawn_local(async move {
                while let Some(chunk) = input_rx.recv().await {
                    if bridge_write.write_all(&chunk).await.is_err() {
                        break;
                    }
                }
            });
            let result = serve_io(bridge_read, tokio::io::stdout(), handler).await;
            bridge.abort();
            result
        })
        .await
}

/// Injectable I/O entry point for worker fixtures and transport tests.
pub async fn serve_io<R, W, H, Fut>(input: R, output: W, handler: H) -> Result<(), MultiplexError>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin + 'static,
    H: Fn(Value, MultiplexEmitter) -> Fut + 'static,
    Fut: Future<Output = Value> + 'static,
{
    let mut reader = BufReader::new(input);
    let byte_budget = Arc::new(Semaphore::new(MULTIPLEX_OUTPUT_BUDGET_BYTES));
    let (fatal_tx, mut fatal_rx) = watch::channel(false);
    let (output_tx, mut output_rx) = mpsc::unbounded_channel::<QueuedFrame>();
    let writer = tokio::task::spawn_local(async move {
        let mut output = output;
        while let Some(frame) = output_rx.recv().await {
            output.write_all(&frame.bytes).await?;
            output.flush().await?;
            drop(frame);
        }
        Ok::<_, io::Error>(())
    });
    let (complete_tx, mut complete_rx) =
        mpsc::channel::<(String, Result<QueuedFrame, MultiplexError>)>(COMPLETION_CHANNEL_CAPACITY);
    let callbacks: PendingCallbacks = Rc::new(RefCell::new(HashMap::new()));
    let handler = Rc::new(handler);
    let mut active: HashMap<String, (JoinHandle<()>, Rc<Cell<bool>>)> = HashMap::new();
    let mut max_seen_call_id = 0_u64;
    let mut input_closed = false;
    // Retain partial bytes when another select branch wins; a local read buffer
    // inside the future would lose half a frame on unrelated call completion.
    let mut input_bytes = Vec::new();
    let result = loop {
        if input_closed && active.is_empty() {
            break Ok(());
        }
        tokio::select! {
            _ = fatal_rx.changed() => { break Err(MultiplexError::Backpressure); }
            completed = complete_rx.recv(), if !active.is_empty() => {
                let Some((call_id, response)) = completed else { break Err(MultiplexError::Protocol); };
                if let Some((_, live)) = active.remove(&call_id) {
                    live.set(false);
                    remove_callbacks(&callbacks, &call_id);
                    let frame = match response { Ok(frame) => frame, Err(error) => break Err(error) };
                    if let Err(error) = queue_output(&output_tx, frame) { break Err(error); }
                }
            }
            frame = read_frame(&mut reader, &mut input_bytes), if !input_closed => {
                let frame = match frame { Ok(Some(frame)) => frame, Ok(None) => { input_closed = true; continue; }, Err(error) => break Err(error) };
                match frame {
                    MultiplexHostMessage::Call { call_id, request } => {
                        let Some(id) = parse_call_id(&call_id) else { break Err(MultiplexError::Correlation); };
                        if id <= max_seen_call_id { break Err(MultiplexError::Correlation); }
                        max_seen_call_id = id;
                        let live = Rc::new(Cell::new(true));
                        let emitter = MultiplexEmitter {
                            call_id: call_id.clone(), output: output_tx.clone(), byte_budget: byte_budget.clone(), callbacks: callbacks.clone(),
                            next_callback_id: Rc::new(Cell::new(0)), live: live.clone(),
                        };
                        let handler = handler.clone();
                        let complete_tx = complete_tx.clone();
                        let task_call_id = call_id.clone();
                        let task_budget = byte_budget.clone();
                        let task_fatal = fatal_tx.clone();
                        let task = tokio::task::spawn_local(async move {
                            let response = handler(request, emitter).await;
                            let frame = encode_wait(MultiplexWorkerMessage::Response { call_id: task_call_id.clone(), response }, &task_budget).await;
                            if complete_tx.send((task_call_id, frame)).await.is_err() { task_fatal.send_replace(true); }
                        });
                        active.insert(call_id, (task, live));
                    }
                    MultiplexHostMessage::Cancel { call_id } => {
                        let Some((task, live)) = active.remove(&call_id) else {
                            // Any previously dispatched call has already reached a terminal.
                            if parse_call_id(&call_id).is_some_and(|id| id <= max_seen_call_id) { continue; }
                            break Err(MultiplexError::Correlation);
                        };
                        live.set(false);
                        task.abort();
                        let _ = task.await; // cancellation acknowledgment follows task termination
                        remove_callbacks(&callbacks, &call_id);
                        let output = output_tx.clone();
                        let budget = byte_budget.clone();
                        let fatal = fatal_tx.clone();
                        tokio::task::spawn_local(async move {
                            match encode_wait(MultiplexWorkerMessage::Cancelled { call_id }, &budget).await {
                                Ok(frame) => { if output.send(frame).is_err() { fatal.send_replace(true); } },
                                Err(_) => { fatal.send_replace(true); },
                            }
                        });
                    }
                    MultiplexHostMessage::CallbackResult { call_id, callback_id, response } => {
                        if !active.contains_key(&call_id) { break Err(MultiplexError::Correlation); }
                        let Some(tx) = callbacks.borrow_mut().remove(&(call_id, callback_id)) else { break Err(MultiplexError::Correlation); };
                        let response_bytes = match serde_json::to_vec(&response) { Ok(bytes) => bytes.len(), Err(error) => break Err(MultiplexError::Json(error)) };
                        let permit = match byte_budget.clone().try_acquire_many_owned(response_bytes as u32) {
                            Ok(permit) => permit, Err(_) => break Err(MultiplexError::Backpressure),
                        };
                        let _ = tx.send((response, permit));
                    }
                }
            }
        }
    };
    for (_, (task, live)) in active.drain() {
        live.set(false);
        task.abort();
        let _ = task.await;
    }
    drop(output_tx);
    if result.is_err() {
        writer.abort();
        return result;
    }
    let writer_result = writer.await.map_err(|_| MultiplexError::Protocol)?;
    writer_result?;
    result
}

fn parse_call_id(value: &str) -> Option<u64> {
    let id = value.parse::<u64>().ok()?;
    (id > 0 && id.to_string() == value).then_some(id)
}
