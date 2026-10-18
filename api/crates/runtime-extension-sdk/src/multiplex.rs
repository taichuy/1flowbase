//! Current-thread worker runner for the versioned multiplex stdio transport.
//! Providers may retain borrowed, non-Send state; each call runs as a local Tokio task.
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    future::Future,
    io::{self, Read},
    rc::Rc,
    sync::{Arc, Mutex},
};

use extension_contracts::{
    MultiplexEnvelope, MultiplexHostMessage, MultiplexHostService, MultiplexWorkerMessage,
    MULTIPLEX_CALL_EVENT_BUDGET_BYTES, MULTIPLEX_OUTPUT_BUDGET_BYTES, RUNTIME_WORKER_RETENTION_V1,
};
use serde_json::Value;
use thiserror::Error;
use tokio::{
    io::{
        AsyncBufRead, AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader, BufWriter,
    },
    sync::{mpsc, oneshot, watch, OwnedSemaphorePermit, Semaphore},
    task::{JoinHandle, LocalSet},
};

// Match Tokio BufWriter's standard physical I/O buffer, not a stream/call limit.
const OUTPUT_WRITE_BUFFER_BYTES: usize = 8 * 1024;

// One active invocation owns cancellation, observed termination, and emitter validity.
struct ActiveCall {
    abort: tokio::task::AbortHandle,
    watcher: JoinHandle<()>,
    live: Rc<Cell<bool>>,
    input: Option<mpsc::UnboundedSender<Value>>,
    ack: Rc<RefCell<HashMap<String, OwnedSemaphorePermit>>>,
    cancelled: Rc<Cell<bool>>,
    event_credit: Arc<Semaphore>,
}

type PendingCallbacks = Rc<RefCell<HashMap<(String, String), oneshot::Sender<Value>>>>;

pub(crate) struct QueuedFrame {
    bytes: Vec<u8>,
    // Held until the writer finishes the write and flush, not merely until enqueue.
    _permit: Option<OwnedSemaphorePermit>,
}

#[derive(Debug, Error)]
pub enum MultiplexError {
    #[error("worker I/O failed: {0}")]
    Io(#[from] io::Error),
    #[error("invalid multiplex JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("unsupported multiplex protocol or control message")]
    Protocol,
    #[error("invalid or duplicate call/callback id")]
    Correlation,
    #[error("worker output channel is full or closed")]
    Backpressure,
    #[error("runtime extension handler panicked")]
    HandlerPanicked,
}

/// Per-call input. Cancellation closes this receiver so the handler can reap its resources.
pub struct MultiplexInput {
    receiver: mpsc::UnboundedReceiver<Value>,
    live: Rc<Cell<bool>>,
}
impl MultiplexInput {
    pub async fn recv(&mut self) -> Option<Value> {
        if !self.live.get() {
            return None;
        }
        self.receiver.recv().await
    }
}

/// A process-local resource lease. Drop publishes a bounded, coalesced control update,
/// including from a background task after the creating invocation has completed.
#[derive(Debug)]
pub struct MultiplexResourceLease {
    retention: Arc<ResourceRetention>,
}
#[derive(Debug)]
struct ResourceRetention {
    state: Mutex<(u64, u64)>,
    updates: watch::Sender<(u64, u64)>,
    flushed: watch::Receiver<u64>,
}
impl Drop for MultiplexResourceLease {
    fn drop(&mut self) {
        let mut state = self
            .retention
            .state
            .lock()
            .expect("worker retention lock poisoned");
        state.1 -= 1;
        // Acquisitions reserve one sequence for their eventual release.
        state.0 += 1;
        self.retention.updates.send_replace(*state);
    }
}

#[derive(Clone)]
pub struct MultiplexEmitter {
    retention: Arc<ResourceRetention>,
    retention_supported: bool,
    call_id: String,
    output: mpsc::UnboundedSender<QueuedFrame>,
    byte_budget: Arc<Semaphore>,
    callbacks: PendingCallbacks,
    next_callback_id: Rc<Cell<u64>>,
    live: Rc<Cell<bool>>,
    ack: Rc<RefCell<HashMap<String, OwnedSemaphorePermit>>>,
    event_lock: Rc<tokio::sync::Mutex<()>>,
    event_credit: Arc<Semaphore>,
    next_event_id: Rc<Cell<u64>>,
}

impl MultiplexEmitter {
    /// Keep this worker alive while a business resource exists. Requires the installed
    /// binding's runtime_worker_retention/v1 capability and the Host advertisement.
    /// Returning the guard implies its acquisition was flushed ahead of this call's Response.
    pub async fn retain_resource(&self) -> Result<MultiplexResourceLease, MultiplexError> {
        if !self.live.get() || !self.retention_supported {
            return Err(MultiplexError::Protocol);
        }
        let sequence = {
            let mut state = self
                .retention
                .state
                .lock()
                .map_err(|_| MultiplexError::Protocol)?;
            let count = state.1.checked_add(1).ok_or(MultiplexError::Protocol)?;
            let sequence = state.0.checked_add(1).ok_or(MultiplexError::Protocol)?;
            sequence
                .checked_add(count)
                .ok_or(MultiplexError::Protocol)?;
            *state = (sequence, count);
            self.retention.updates.send_replace(*state);
            sequence
        };
        let guard = MultiplexResourceLease {
            retention: self.retention.clone(),
        };
        let mut flushed = self.retention.flushed.clone();
        while *flushed.borrow_and_update() < sequence {
            flushed
                .changed()
                .await
                .map_err(|_| MultiplexError::Backpressure)?;
        }
        Ok(guard)
    }

    /// Sliding byte credit bounds unconsumed output without one network roundtrip per event.
    pub async fn event(&self, event: Value) -> Result<(), MultiplexError> {
        let _lock = self.event_lock.lock().await;
        if !self.live.get() {
            return Err(MultiplexError::Correlation);
        }
        let next = self
            .next_event_id
            .get()
            .checked_add(1)
            .ok_or(MultiplexError::Correlation)?;
        let event_id = next.to_string();
        let message = MultiplexWorkerMessage::Event {
            call_id: self.call_id.clone(),
            event,
            await_ack: true,
            event_id: Some(event_id.clone()),
        };
        let size = encode_bytes(message.clone())?.len();
        let credit = self
            .event_credit
            .clone()
            .acquire_many_owned(size.min(MULTIPLEX_CALL_EVENT_BUDGET_BYTES) as u32)
            .await
            .map_err(|_| MultiplexError::Correlation)?;
        let frame = encode_wait(message, &self.byte_budget).await?;
        if !self.live.get() {
            return Err(MultiplexError::Correlation);
        }
        self.next_event_id.set(next);
        self.ack.borrow_mut().insert(event_id.clone(), credit);
        if let Err(error) = queue_output(&self.output, frame) {
            self.ack.borrow_mut().remove(&event_id);
            return Err(error);
        }
        Ok(())
    }

    /// Synchronous callbacks cannot wait on the current-thread writer. They retain
    /// complete events until stdout drains; use `event` for acknowledged async flow control.
    pub fn try_event(&self, event: Value) -> Result<(), MultiplexError> {
        if !self.live.get() {
            return Err(MultiplexError::Correlation);
        }
        let frame = encode(
            MultiplexWorkerMessage::Event {
                call_id: self.call_id.clone(),
                event,
                await_ack: false,
                event_id: None,
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
        let result = rx.await.map_err(|_| MultiplexError::Correlation);
        self.callbacks.borrow_mut().remove(&key);
        result
    }
}

fn encode_bytes(message: MultiplexWorkerMessage) -> Result<Vec<u8>, MultiplexError> {
    let mut bytes = serde_json::to_vec(&MultiplexEnvelope::new(message))?;
    bytes.push(b'\n');
    Ok(bytes)
}

pub(crate) fn encode(
    message: MultiplexWorkerMessage,
    budget: &Arc<Semaphore>,
) -> Result<QueuedFrame, MultiplexError> {
    let bytes = encode_bytes(message)?;
    let permit = budget
        .clone()
        .try_acquire_many_owned(bytes.len().min(MULTIPLEX_OUTPUT_BUDGET_BYTES) as u32)
        .ok();
    Ok(QueuedFrame {
        bytes,
        _permit: permit,
    })
}

async fn encode_wait(
    message: MultiplexWorkerMessage,
    budget: &Arc<Semaphore>,
) -> Result<QueuedFrame, MultiplexError> {
    // The window schedules queued writes, not admissible message sizes. A message
    // larger than the window acquires it exclusively and still makes progress.
    let bytes = encode_bytes(message)?;
    let permit = budget
        .clone()
        .acquire_many_owned(bytes.len().min(MULTIPLEX_OUTPUT_BUDGET_BYTES) as u32)
        .await
        .map_err(|_| MultiplexError::Backpressure)?;
    Ok(QueuedFrame {
        bytes,
        _permit: Some(permit),
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
        bytes.extend_from_slice(&available[..count]);
        reader.consume(count);
        if bytes.last() == Some(&b'\n') {
            bytes.pop();
            let frame: MultiplexEnvelope<MultiplexHostMessage> = serde_json::from_slice(bytes)?;
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
    // Sync provider callbacks cannot await their own current-thread writer. Preserve
    // accepted frames; async producers use encode_wait and stdout supplies I/O backpressure.
    output.send(frame).map_err(|_| MultiplexError::Backpressure)
}

pub async fn serve<H, Fut>(handler: H) -> Result<(), MultiplexError>
where
    H: Fn(Value, MultiplexEmitter) -> Fut + 'static,
    Fut: Future<Output = Value> + 'static,
{
    serve_duplex(move |request, emitter, _input| handler(request, emitter)).await
}
pub async fn serve_io<R, W, H, Fut>(input: R, output: W, handler: H) -> Result<(), MultiplexError>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin + 'static,
    H: Fn(Value, MultiplexEmitter) -> Fut + 'static,
    Fut: Future<Output = Value> + 'static,
{
    serve_duplex_io(input, output, move |request, emitter, _input| {
        handler(request, emitter)
    })
    .await
}

/// Serve a worker's standard input/output using local tasks. Call from a Tokio runtime.
/// The handler owns its business concurrency and may serialize calls by session.
/// A handler returns the legacy typed response serialized to JSON Value.
pub async fn serve_duplex<H, Fut>(handler: H) -> Result<(), MultiplexError>
where
    H: Fn(Value, MultiplexEmitter, MultiplexInput) -> Fut + 'static,
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
            let result = serve_duplex_io(bridge_read, tokio::io::stdout(), handler).await;
            bridge.abort();
            result
        })
        .await
}

/// Injectable I/O entry point for worker fixtures and transport tests.
pub async fn serve_duplex_io<R, W, H, Fut>(
    input: R,
    output: W,
    handler: H,
) -> Result<(), MultiplexError>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin + 'static,
    H: Fn(Value, MultiplexEmitter, MultiplexInput) -> Fut + 'static,
    Fut: Future<Output = Value> + 'static,
{
    let mut reader = BufReader::new(input);
    let byte_budget = Arc::new(Semaphore::new(MULTIPLEX_OUTPUT_BUDGET_BYTES));
    let (_fatal_tx, mut fatal_rx) = watch::channel(false);
    let (panic_tx, mut panic_rx) = watch::channel(false);
    let (output_tx, mut output_rx) = mpsc::unbounded_channel::<QueuedFrame>();
    let (retention_updates, mut retention_rx) = watch::channel((0_u64, 0_u64));
    let (retention_flushed, flushed_rx) = watch::channel(0_u64);
    let retention = Arc::new(ResourceRetention {
        state: Mutex::new((0, 0)),
        updates: retention_updates,
        flushed: flushed_rx,
    });
    let writer = tokio::task::spawn_local(async move {
        let mut output = BufWriter::with_capacity(OUTPUT_WRITE_BUFFER_BYTES, output);
        let mut pending: Option<QueuedFrame> = None;
        loop {
            let frame = match pending.take() {
                Some(frame) => frame,
                None => tokio::select! {
                    biased;
                    update = retention_rx.changed() => {
                        if update.is_err() { break; }
                        let (sequence, retained_resources) = *retention_rx.borrow_and_update();
                        let bytes = encode_bytes(MultiplexWorkerMessage::ResourceRetention { sequence, retained_resources })?;
                        output.write_all(&bytes).await?;
                        output.flush().await?;
                        retention_flushed.send_replace(sequence);
                        continue;
                    }
                    frame = output_rx.recv() => match frame {
                        Some(frame) => frame,
                        None => break,
                    },
                },
            };
            // Coalesce ready frames within one physical buffer. Larger individual
            // frames still pass whole. Flush and release prefix credit before a
            // slow tail: draining the entire byte budget withholds that credit.
            // No timer or stream/call count limit; FIFO and terminals stay intact.
            let mut bytes = frame.bytes.len();
            let mut batch = vec![frame];
            while bytes < OUTPUT_WRITE_BUFFER_BYTES {
                let Ok(frame) = output_rx.try_recv() else {
                    break;
                };
                if bytes + frame.bytes.len() > OUTPUT_WRITE_BUFFER_BYTES {
                    pending = Some(frame);
                    break;
                }
                bytes += frame.bytes.len();
                batch.push(frame);
            }
            for frame in &batch {
                output.write_all(&frame.bytes).await?;
            }
            output.flush().await?;
            drop(batch);
        }
        Ok::<_, MultiplexError>(())
    });
    // Completed frames already hold the shared byte-budget permit. A count cap
    // would reject a burst of small independent calls for no memory benefit.
    let (complete_tx, mut complete_rx) =
        mpsc::unbounded_channel::<(String, Result<QueuedFrame, MultiplexError>)>();
    let callbacks: PendingCallbacks = Rc::new(RefCell::new(HashMap::new()));
    let handler = Rc::new(handler);
    // The watcher owns each handler JoinHandle, so construction, polling, and
    // terminal encoding panics cannot leave an active call waiting forever.
    let mut active: HashMap<String, ActiveCall> = HashMap::new();
    let mut max_seen_call_id = 0_u64;
    let mut retention_supported = false;
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
            _ = panic_rx.changed() => { break Err(MultiplexError::HandlerPanicked); }
            completed = complete_rx.recv(), if !active.is_empty() => {
                let Some((call_id, response)) = completed else { break Err(MultiplexError::Protocol); };
                if let Some(ActiveCall { watcher, live, .. }) = active.remove(&call_id) {
                    let _ = watcher.await;
                    live.set(false);
                    remove_callbacks(&callbacks, &call_id);
                    let frame = match response { Ok(frame) => frame, Err(error) => break Err(error) };
                    if let Err(error) = queue_output(&output_tx, frame) { break Err(error); }
                }
            }
            frame = read_frame(&mut reader, &mut input_bytes), if !input_closed => {
                let frame = match frame { Ok(Some(frame)) => frame, Ok(None) => { input_closed = true; continue; }, Err(error) => break Err(error) };
                match frame {
                    MultiplexHostMessage::WorkerCapabilities { capabilities } => {
                        if max_seen_call_id != 0 || retention_supported
                            || capabilities != [RUNTIME_WORKER_RETENTION_V1.to_string()] {
                            break Err(MultiplexError::Protocol);
                        }
                        retention_supported = true;
                    }
                    MultiplexHostMessage::Call { call_id, request } => {
                        let Some(id) = parse_call_id(&call_id) else { break Err(MultiplexError::Correlation); };
                        if id <= max_seen_call_id { break Err(MultiplexError::Correlation); }
                        max_seen_call_id = id;
                        let live = Rc::new(Cell::new(true));
                        let cancelled = Rc::new(Cell::new(false));
                        let ack = Rc::new(RefCell::new(HashMap::new()));
                        let event_credit = Arc::new(Semaphore::new(MULTIPLEX_CALL_EVENT_BUDGET_BYTES));
                        let (input_tx, input_rx) = mpsc::unbounded_channel();
                        let emitter = MultiplexEmitter {
                            retention: retention.clone(), retention_supported,
                            call_id: call_id.clone(), output: output_tx.clone(), byte_budget: byte_budget.clone(), callbacks: callbacks.clone(),
                            next_callback_id: Rc::new(Cell::new(0)), live: live.clone(), ack: ack.clone(), event_lock: Rc::new(tokio::sync::Mutex::new(())), event_credit: event_credit.clone(), next_event_id: Rc::new(Cell::new(0)),
                        };
                        let handler = handler.clone();
                        let complete_tx = complete_tx.clone();
                        let task_call_id = call_id.clone();
                        let task_budget = byte_budget.clone();
                        let input_live = live.clone();
                        let final_credit = event_credit.clone();
                        let task = tokio::task::spawn_local(async move {
                            let response = handler(request, emitter, MultiplexInput { receiver: input_rx, live: input_live }).await;
                            // Response must not overtake output still owned by the downstream consumer.
                            let _drained = final_credit.acquire_many(MULTIPLEX_CALL_EVENT_BUDGET_BYTES as u32).await;
                            encode_wait(MultiplexWorkerMessage::Response { call_id: task_call_id, response }, &task_budget).await
                        });
                        let abort = task.abort_handle();
                        let task_call_id = call_id.clone();
                        let task_panic = panic_tx.clone();
                        let task_cancelled = cancelled.clone();
                        let cancel_budget = byte_budget.clone();
                        let watcher = tokio::task::spawn_local(async move {
                            match task.await {
                                Ok(frame) => {
                                    let frame = if task_cancelled.get() { encode_wait(MultiplexWorkerMessage::Cancelled { call_id: task_call_id.clone() }, &cancel_budget).await } else { frame };
                                    let _ = complete_tx.send((task_call_id, frame));
                                }
                                Err(error) if error.is_panic() => { task_panic.send_replace(true); }
                                Err(_) => {
                                    if task_cancelled.get() {
                                        let frame = encode_wait(MultiplexWorkerMessage::Cancelled { call_id: task_call_id.clone() }, &cancel_budget).await;
                                        let _ = complete_tx.send((task_call_id, frame));
                                    }
                                }
                            }
                        });
                        active.insert(call_id, ActiveCall { abort, watcher, live, input: Some(input_tx), ack, cancelled, event_credit });
                    }
                    MultiplexHostMessage::Input { call_id, input } => {
                        let Some(call) = active.get(&call_id) else { break Err(MultiplexError::Correlation); };
                        if call.cancelled.get() { continue; }
                        // Read-side values must not reserve output credit: their consumer may
                        // need to emit a response before it can consume another input.
                        if call.input.as_ref().is_none_or(|tx| tx.send(input).is_err()) { break Err(MultiplexError::Correlation); }
                    }
                    MultiplexHostMessage::EventAck { call_id, event_id } => {
                        let Some(call) = active.get(&call_id) else { break Err(MultiplexError::Correlation); };
                        if call.cancelled.get() { continue; }
                        if call.ack.borrow_mut().remove(&event_id).is_none() { break Err(MultiplexError::Correlation); }
                    }
                    MultiplexHostMessage::Cancel { call_id } => {
                        let Some(call) = active.get_mut(&call_id) else {
                            if parse_call_id(&call_id).is_some_and(|id| id <= max_seen_call_id) { continue; }
                            break Err(MultiplexError::Correlation);
                        };
                        if call.cancelled.replace(true) { continue; }
                        call.live.set(false);
                        let graceful = call.input.as_ref().is_some_and(|input| !input.is_closed());
                        call.input.take();
                        call.ack.borrow_mut().clear();
                        call.event_credit.close();
                        remove_callbacks(&callbacks, &call_id);
                        let abort = call.abort.clone();
                        // Keep the reader responsive while the handler performs PTY/process cleanup.
                        tokio::task::spawn_local(async move {
                            if graceful { tokio::time::sleep(std::time::Duration::from_secs(1)).await; }
                            abort.abort();
                        });
                    }
                    MultiplexHostMessage::CallbackResult { call_id, callback_id, response } => {
                        if !active.contains_key(&call_id) { break Err(MultiplexError::Correlation); }
                        let Some(tx) = callbacks.borrow_mut().remove(&(call_id, callback_id)) else { break Err(MultiplexError::Correlation); };
                        let _ = tx.send(response);
                    }
                }
            }
        }
    };
    for (
        _,
        ActiveCall {
            abort,
            watcher,
            live,
            ..
        },
    ) in active.drain()
    {
        live.set(false);
        abort.abort();
        let _ = watcher.await;
    }
    drop(output_tx);
    if result.is_err() {
        writer.abort();
        let _ = writer.await;
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
