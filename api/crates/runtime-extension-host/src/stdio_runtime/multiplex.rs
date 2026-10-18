use super::*;
use extension_contracts::{
    MultiplexEnvelope, MultiplexHostMessage, MultiplexHostService, MultiplexWorkerMessage,
    PluginDataRequest, MULTIPLEX_OUTPUT_BUDGET_BYTES, RUNTIME_WORKER_RETENTION_V1,
};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use tokio::sync::{mpsc, oneshot, OwnedSemaphorePermit, Semaphore};

const QUEUE_CAPACITY: usize = 8;
const WRITER_BYTES: usize = MULTIPLEX_OUTPUT_BUDGET_BYTES;
const CANCEL_GRACE: Duration = Duration::from_secs(2);

struct Delivered {
    message: Result<MultiplexWorkerMessage, String>,
}

struct WriteFrame {
    bytes: Vec<u8>,
    _permit: OwnedSemaphorePermit,
    call_id: Option<String>,
    live: Option<Arc<AtomicBool>>,
}

struct ControlFrame {
    bytes: Vec<u8>,
    _permit: Option<OwnedSemaphorePermit>,
    live: Option<Arc<AtomicBool>>,
}

enum CommandMessage {
    Call {
        request: Value,
        events: mpsc::UnboundedSender<Delivered>,
        host_calls: Option<Box<ProviderHostCallContext>>,
        registered: oneshot::Sender<String>,
        permit: OwnedSemaphorePermit,
        lease: Option<Box<dyn Send + Sync>>,
    },
    Cancel {
        id: String,
    },
    Input {
        id: String,
        input: Value,
        permit: OwnedSemaphorePermit,
    },
    EventAck {
        id: String,
        event_id: String,
    },
    CallbackDone {
        id: String,
        callback_id: String,
        response: Value,
    },
    WriterFailed(String),
    Written(String),
    GracefulStop(oneshot::Sender<()>),
}

struct ActiveCall {
    events: mpsc::UnboundedSender<Delivered>,
    host_calls: Option<ProviderHostCallContext>,
    callbacks: HashMap<String, tokio::task::JoinHandle<()>>,
    max_callback_id: u64,
    cancelling_since: Option<Instant>,
    written: bool,
    pending_acks: std::collections::HashSet<String>,
    max_event_id: u64,
    live: Arc<AtomicBool>,
    _lease: Option<Arc<Box<dyn Send + Sync>>>,
}

struct ReaderContext {
    retention_enabled: bool,
    retained_resources: Arc<AtomicU64>,
    command_sender: mpsc::Sender<CommandMessage>,
    writes: mpsc::UnboundedSender<WriteFrame>,
    controls: mpsc::Sender<ControlFrame>,
    writer_budget: Arc<Semaphore>,
    control: ProviderWorkerProcessControl,
    failed: Arc<AtomicBool>,
}

impl Drop for ActiveCall {
    fn drop(&mut self) {
        for (_, task) in self.callbacks.drain() {
            task.abort();
        }
    }
}

struct CancelOnDrop {
    id: String,
    commands: mpsc::Sender<CommandMessage>,
    control: ProviderWorkerProcessControl,
    armed: bool,
}

impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        if self.armed {
            let commands = self.commands.clone();
            let id = self.id.clone();
            let control = self.control.clone();
            // A full command lane is backpressure, not evidence that neighbours
            // must be killed. The actor enforces the acknowledgment deadline.
            tokio::spawn(async move {
                if commands.send(CommandMessage::Cancel { id }).await.is_err() {
                    control.terminate().await;
                }
            });
        }
    }
}

#[derive(Debug)]
pub(crate) struct MultiplexProviderWorker {
    retained_resources: Arc<AtomicU64>,
    control: ProviderWorkerProcessControl,
    commands: mpsc::Sender<CommandMessage>,
    writer_budget: Arc<Semaphore>,
    failed: Arc<AtomicBool>,
    limits: PluginRuntimeLimits,
}

impl MultiplexProviderWorker {
    pub(crate) fn activate(
        path: PathBuf,
        limits: PluginRuntimeLimits,
        _generation: u64,
    ) -> FrameworkResult<Self> {
        Self::activate_with_retention(path, limits, _generation, false)
    }

    pub(crate) fn activate_with_retention(
        path: PathBuf,
        limits: PluginRuntimeLimits,
        _generation: u64,
        retention_enabled: bool,
    ) -> FrameworkResult<Self> {
        let process = spawn_worker_process(&path, &limits)?;
        let control = process.control.clone();
        let stdin = process.stdin;
        let stdout = process.stdout.into_inner().into_inner();
        let (commands, receiver) = mpsc::channel(QUEUE_CAPACITY);
        let (writes, write_receiver) = mpsc::unbounded_channel();
        let (controls, control_receiver) = mpsc::channel(QUEUE_CAPACITY);
        if retention_enabled {
            let bytes = encode(MultiplexHostMessage::WorkerCapabilities {
                capabilities: vec![RUNTIME_WORKER_RETENTION_V1.to_string()],
            })
            .map_err(|_| {
                PluginFrameworkError::invalid_provider_package("worker capability encoding failed")
            })?;
            controls
                .try_send(ControlFrame {
                    bytes,
                    _permit: None,
                    live: None,
                })
                .map_err(|_| {
                    PluginFrameworkError::invalid_provider_package(
                        "worker capability admission failed",
                    )
                })?;
        }
        let retained_resources = Arc::new(AtomicU64::new(0));
        let reader_retention = retained_resources.clone();
        let writer_budget = Arc::new(Semaphore::new(WRITER_BYTES));
        let failed = Arc::new(AtomicBool::new(false));
        let actor_control = control.clone();
        let actor_failed = failed.clone();
        let actor_commands = commands.clone();
        let reader_commands = commands.clone();
        let actor_writer_budget = writer_budget.clone();
        tokio::spawn(async move {
            writer(stdin, write_receiver, control_receiver, actor_commands).await;
        });
        tokio::spawn(async move {
            run_reader(
                stdout,
                receiver,
                ReaderContext {
                    retention_enabled,
                    retained_resources: reader_retention,
                    command_sender: reader_commands,
                    writes,
                    controls,
                    writer_budget: actor_writer_budget,
                    control: actor_control,
                    failed: actor_failed,
                },
            )
            .await;
        });
        Ok(Self {
            retained_resources,
            control,
            commands,
            writer_budget,
            failed,
            limits,
        })
    }

    pub(crate) fn process_control(&self) -> ProviderWorkerProcessControl {
        self.control.clone()
    }
    pub(crate) fn is_failed(&self) -> bool {
        self.failed.load(Ordering::Acquire)
    }
    pub(crate) fn retained_resources(&self) -> u64 {
        self.retained_resources.load(Ordering::Acquire)
    }
    pub(crate) fn limits(&self) -> &PluginRuntimeLimits {
        &self.limits
    }

    pub(crate) async fn call(
        &self,
        request: &ProviderStdioRequest,
        limits: &PluginRuntimeLimits,
        context: Option<StreamingCallContext>,
        lease: Option<Box<dyn Send + Sync>>,
    ) -> FrameworkResult<(Value, Vec<ProviderStreamEvent>)> {
        if self.is_failed() {
            return Err(worker_exited_error());
        }
        let capture = context
            .as_ref()
            .is_some_and(|context| context.protocol_observation.is_some());
        let request_bytes = serialize_provider_stdio_request(request, capture)
            .map_err(|error| PluginFrameworkError::serialization(None, error.to_string()))?;
        let request = serde_json::from_slice(&request_bytes)
            .map_err(|error| PluginFrameworkError::serialization(None, error.to_string()))?;
        self.call_json(request, limits, context, lease).await
    }

    pub(crate) async fn call_json(
        &self,
        request: Value,
        limits: &PluginRuntimeLimits,
        context: Option<StreamingCallContext>,
        lease: Option<Box<dyn Send + Sync>>,
    ) -> FrameworkResult<(Value, Vec<ProviderStreamEvent>)> {
        self.call_json_duplex(request, limits, context, lease, None, None)
            .await
    }

    pub(crate) async fn call_json_duplex(
        &self,
        request: Value,
        limits: &PluginRuntimeLimits,
        context: Option<StreamingCallContext>,
        lease: Option<Box<dyn Send + Sync>>,
        duplex: Option<runtime_core::runtime_backend::RuntimeManagedDuplex>,
        mut cancel: Option<tokio::sync::watch::Receiver<bool>>,
    ) -> FrameworkResult<(Value, Vec<ProviderStreamEvent>)> {
        if self.is_failed() {
            return Err(worker_exited_error());
        }
        let request_bytes = serde_json::to_vec(&request)
            .map_err(|error| PluginFrameworkError::serialization(None, error.to_string()))?;
        let permit = self
            .writer_budget
            .clone()
            .acquire_many_owned(request_bytes.len().saturating_add(256).min(WRITER_BYTES) as u32)
            .await
            .map_err(|_| transport_error("multiplex writer budget closed"))?;
        let request = serde_json::from_slice(&request_bytes)
            .map_err(|error| PluginFrameworkError::serialization(None, error.to_string()))?;
        let (events, mut receiver) = mpsc::unbounded_channel();
        let (registered, registration) = oneshot::channel();
        self.commands
            .send(CommandMessage::Call {
                request,
                events,
                host_calls: context
                    .as_ref()
                    .and_then(|context| context.host_calls.clone())
                    .map(Box::new),
                registered,
                permit,
                lease,
            })
            .await
            .map_err(|_| transport_error("multiplex command queue is full or closed"))?;
        let id = registration
            .await
            .map_err(|_| transport_error("multiplex call registration failed"))?;
        let mut guard = CancelOnDrop {
            id,
            commands: self.commands.clone(),
            control: self.control.clone(),
            armed: true,
        };
        let (input_closed_tx, input_closed_rx) = tokio::sync::watch::channel(false);
        let mut input_closed = duplex.as_ref().map(|_| input_closed_rx);
        let input_task = duplex.as_ref().map(|duplex| {
            let input = duplex.input.clone();
            let commands = self.commands.clone();
            let budget = self.writer_budget.clone();
            let call_id = guard.id.clone();
            tokio::spawn(async move {
                let mut input = input.lock().await;
                while let Some(value) = input.recv().await {
                    let Ok(bytes) = serde_json::to_vec(&value) else {
                        break;
                    };
                    let size = bytes.len().saturating_add(256).min(WRITER_BYTES);
                    let Ok(permit) = budget.clone().acquire_many_owned(size as u32).await else {
                        break;
                    };
                    if commands
                        .send(CommandMessage::Input {
                            id: call_id.clone(),
                            input: value,
                            permit,
                        })
                        .await
                        .is_err()
                    {
                        return;
                    }
                }
                input_closed_tx.send_replace(true);
                let _ = commands.send(CommandMessage::Cancel { id: call_id }).await;
            })
        });
        struct AbortInput(Option<tokio::task::JoinHandle<()>>);
        impl Drop for AbortInput {
            fn drop(&mut self) {
                if let Some(task) = &self.0 {
                    task.abort();
                }
            }
        }
        let _input_task = AbortInput(input_task);
        let mut timeout = ProviderStreamTimeoutState::new();
        let mut output_events = Vec::new();
        let mut first_error: Option<ProviderRuntimeError> = None;
        let mut pending_acks = tokio::task::JoinSet::new();
        loop {
            let receive = async {
                loop {
                    tokio::select! {
                        message = receiver.recv() => return message,
                    _ = async { if let Some(closed) = &mut input_closed { if !*closed.borrow() { let _ = closed.changed().await; } } else { std::future::pending::<()>().await; } } => {
                        input_closed = None;
                        let _ = self.commands.send(CommandMessage::Cancel { id: guard.id.clone() }).await;
                        continue;
                    }
                        result = pending_acks.join_next(), if !pending_acks.is_empty() => {
                            if !matches!(result, Some(Ok(true))) { let _ = self.commands.send(CommandMessage::Cancel { id: guard.id.clone() }).await; }
                            continue;
                        }
                        _ = async { if let Some(cancel) = &mut cancel { if !*cancel.borrow() { let _ = cancel.changed().await; } } else { std::future::pending::<()>().await; } } => {
                            cancel = None;
                            let _ = self.commands.send(CommandMessage::Cancel { id: guard.id.clone() }).await;
                            continue;
                        }
                    }
                }
            };
            let received = if duplex.is_some() {
                // Raw streams have no provider token or wall-clock timeout. The managed
                // admission owner enforces any explicit finite principal deadline.
                receive.await
            } else {
                let (duration, kind, timeout_ms) = timeout.next_read_timeout(limits);
                tokio::time::timeout(duration, receive).await.map_err(|_| {
                    prefer_first_error(&first_error, provider_timeout_error(kind, timeout_ms))
                })?
            };
            let message = received.ok_or_else(|| {
                prefer_first_error(
                    &first_error,
                    transport_error("multiplex call channel closed"),
                )
            })?;
            let frame = message
                .message
                .map_err(|error| prefer_first_error(&first_error, transport_error(&error)))?;
            match frame {
                MultiplexWorkerMessage::Event {
                    event,
                    await_ack,
                    event_id,
                    ..
                } => {
                    if let Some(duplex) = &duplex {
                        let (consumed, consumed_rx) = oneshot::channel();
                        let forwarded = runtime_core::runtime_backend::RuntimeManagedDuplexEvent {
                            event,
                            consumed,
                        };
                        let event_id = if await_ack { event_id } else { None };
                        let forwarded_result = tokio::select! {
                            result = duplex.events.send(forwarded) => result.is_ok(),
                            _ = async { if let Some(closed) = &mut input_closed { if !*closed.borrow() { let _ = closed.changed().await; } } else { std::future::pending::<()>().await; } } => { input_closed = None; false },
                            _ = async { if let Some(cancel) = &mut cancel { if !*cancel.borrow() { let _ = cancel.changed().await; } } else { std::future::pending::<()>().await; } } => { cancel = None; false }
                        };
                        if !forwarded_result {
                            let _ = self
                                .commands
                                .send(CommandMessage::Cancel {
                                    id: guard.id.clone(),
                                })
                                .await;
                            continue;
                        }
                        let commands = self.commands.clone();
                        let id = guard.id.clone();
                        pending_acks.spawn(async move {
                            if consumed_rx.await.is_err() {
                                let _ = commands.send(CommandMessage::Cancel { id }).await;
                                return false;
                            }
                            if let Some(event_id) = event_id {
                                return commands
                                    .send(CommandMessage::EventAck { id, event_id })
                                    .await
                                    .is_ok();
                            }
                            true
                        });
                        continue;
                    }
                    if await_ack {
                        return Err(transport_error("duplex event on unary call"));
                    }
                    let event: ProviderStreamEvent =
                        serde_json::from_value(event).map_err(|error| {
                            prefer_first_error(
                                &first_error,
                                transport_error(&format!(
                                    "invalid multiplex provider event: {error}"
                                )),
                            )
                        })?;
                    if matches!(event, ProviderStreamEvent::ProtocolObservation { .. }) {
                        forward_provider_live_event(
                            None,
                            None,
                            context
                                .as_ref()
                                .and_then(|c| c.protocol_observation.as_deref()),
                            event,
                        )
                        .await
                        .map_err(|error| prefer_first_error(&first_error, error))?;
                        continue;
                    }
                    if let ProviderStreamEvent::Error { error } = &event {
                        if first_error.is_none() {
                            first_error = Some(error.clone());
                        }
                    }
                    timeout.record_stream_event(&event);
                    if let Some(context) = &context {
                        if let Some(observer) = &context.event_observer {
                            let _ = observer.send(());
                        }
                        let (remaining, kind, timeout_ms) = timeout.next_read_timeout(limits);
                        tokio::time::timeout(
                            remaining,
                            forward_provider_live_event(
                                context.required_live_events.as_ref(),
                                context.diagnostic_live_events.as_ref(),
                                context.protocol_observation.as_deref(),
                                event.clone(),
                            ),
                        )
                        .await
                        .map_err(|_| {
                            prefer_first_error(
                                &first_error,
                                provider_timeout_error(kind, timeout_ms),
                            )
                        })?
                        .map_err(|error| prefer_first_error(&first_error, error))?;
                    }
                    output_events.push(event);
                }
                MultiplexWorkerMessage::ResourceRetention { .. } => {
                    return Err(transport_error("worker control on invocation lane"))
                }
                MultiplexWorkerMessage::Response { response, .. } => {
                    guard.armed = false;
                    if let Some(error) = first_error {
                        return Err(PluginFrameworkError::runtime(error));
                    }
                    return Ok((response, output_events));
                }
                MultiplexWorkerMessage::Cancelled { .. } => {
                    guard.armed = false;
                    return Err(prefer_first_error(
                        &first_error,
                        transport_error("multiplex call cancelled"),
                    ));
                }
                MultiplexWorkerMessage::Callback { .. } => {
                    unreachable!("callback is handled in carrier")
                }
            }
        }
    }

    pub(crate) async fn stop(
        &self,
        generation: u64,
        reason: ProviderWorkerCleanupReason,
        final_state: ProviderWorkerLifecycleState,
        evidence: Option<ProviderWorkerTerminationEvidence>,
    ) -> ProviderWorkerCleanupReceipt {
        let evidence = match evidence {
            Some(evidence) => evidence,
            None => {
                let (sender, receiver) = oneshot::channel();
                let _ = self
                    .commands
                    .send(CommandMessage::GracefulStop(sender))
                    .await;
                let _ = tokio::time::timeout(Duration::from_secs(2), receiver).await;
                self.control.wait_for_graceful_exit().await
            }
        };
        ProviderWorkerCleanupReceipt {
            generation,
            prior_pid: evidence.prior_pid,
            kill_sent: evidence.kill_sent,
            exited: evidence.exited,
            final_state,
            reason,
            cleanup_error: evidence.cleanup_error,
        }
    }
}

fn transport_error(message: &str) -> PluginFrameworkError {
    PluginFrameworkError::runtime(ProviderRuntimeError::normalize(
        "provider_runtime",
        message,
        None,
    ))
}

fn prefer_first_error(
    first: &Option<ProviderRuntimeError>,
    fallback: PluginFrameworkError,
) -> PluginFrameworkError {
    first
        .as_ref()
        .map(|error| PluginFrameworkError::runtime(error.clone()))
        .unwrap_or(fallback)
}

fn encode(message: MultiplexHostMessage) -> Result<Vec<u8>, String> {
    let mut bytes =
        serde_json::to_vec(&MultiplexEnvelope::new(message)).map_err(|error| error.to_string())?;
    bytes.push(b'\n');
    Ok(bytes)
}

async fn read_frame(
    reader: &mut BufReader<ChildStdout>,
    bytes: &mut Vec<u8>,
) -> Result<Option<(MultiplexWorkerMessage, usize)>, String> {
    loop {
        let available = reader.fill_buf().await.map_err(|error| error.to_string())?;
        if available.is_empty() {
            return if bytes.is_empty() {
                Ok(None)
            } else {
                Err("partial multiplex frame".into())
            };
        }
        let count = available
            .iter()
            .position(|byte| *byte == b'\n')
            .map_or(available.len(), |i| i + 1);
        bytes.extend_from_slice(&available[..count]);
        reader.consume(count);
        if bytes.last() == Some(&b'\n') {
            bytes.pop();
            let envelope: MultiplexEnvelope<MultiplexWorkerMessage> =
                serde_json::from_slice(bytes.as_slice()).map_err(|error| error.to_string())?;
            if !envelope.is_supported() {
                return Err("unsupported multiplex protocol".into());
            }
            let frame_bytes = bytes.len() + 1;
            bytes.clear();
            return Ok(Some((envelope.message, frame_bytes)));
        }
    }
}

async fn writer(
    mut stdin: ChildStdin,
    mut writes: mpsc::UnboundedReceiver<WriteFrame>,
    mut controls: mpsc::Receiver<ControlFrame>,
    commands: mpsc::Sender<CommandMessage>,
) {
    loop {
        let frame = tokio::select! {
            biased;
            control = controls.recv() => control.map(|frame| (frame.bytes, frame._permit, None, frame.live)),
            data = writes.recv() => data.map(|frame| (frame.bytes, Some(frame._permit), frame.call_id, frame.live)),
        };
        let Some((bytes, permit, call_id, live)) = frame else {
            return;
        };
        if live.is_some_and(|live| !live.load(Ordering::Acquire)) {
            continue;
        }
        if let Err(error) = stdin.write_all(&bytes).await {
            let _ = commands
                .send(CommandMessage::WriterFailed(error.to_string()))
                .await;
            return;
        }
        if let Err(error) = stdin.flush().await {
            let _ = commands
                .send(CommandMessage::WriterFailed(error.to_string()))
                .await;
            return;
        }
        drop(permit);
        if let Some(id) = call_id {
            if commands.send(CommandMessage::Written(id)).await.is_err() {
                return;
            }
        }
    }
}

async fn run_reader(
    stdout: ChildStdout,
    mut commands: mpsc::Receiver<CommandMessage>,
    context: ReaderContext,
) {
    let ReaderContext {
        retention_enabled,
        retained_resources,
        command_sender,
        writes,
        controls,
        writer_budget,
        control,
        failed,
    } = context;
    let mut reader = BufReader::new(stdout);
    // Kept outside select: a command may interrupt a partially read frame.
    let mut pending_frame = Vec::new();
    let mut active: HashMap<String, ActiveCall> = HashMap::new();
    let mut next_id = 0_u64;
    let mut retention_sequence = 0_u64;
    let mut ticker = tokio::time::interval(Duration::from_millis(100));
    let reason = loop {
        tokio::select! {
            frame = read_frame(&mut reader, &mut pending_frame) => {
                let (frame, _frame_bytes) = match frame { Ok(Some(frame)) => frame, Ok(None) => break "multiplex worker stdout closed".to_string(), Err(error) => break error };
                if let MultiplexWorkerMessage::ResourceRetention { sequence, retained_resources: count } = &frame {
                    if !retention_enabled || *sequence == 0 { break "unsupported worker retention control".to_string(); }
                    if *sequence > retention_sequence {
                        retention_sequence = *sequence;
                        retained_resources.store(*count, Ordering::Release);
                    }
                    continue;
                }
                let id = match &frame { MultiplexWorkerMessage::Event { call_id, .. } | MultiplexWorkerMessage::Response { call_id, .. } | MultiplexWorkerMessage::Cancelled { call_id } | MultiplexWorkerMessage::Callback { call_id, .. } => call_id.clone(), MultiplexWorkerMessage::ResourceRetention { .. } => unreachable!("retention is process-scoped") };
                let Some(call) = active.get_mut(&id) else { break format!("unknown or duplicate multiplex call id: {id}"); };
                match frame {
                    MultiplexWorkerMessage::ResourceRetention { .. } => unreachable!("retention is process-scoped"),
                    MultiplexWorkerMessage::Callback { callback_id, service, request, .. } => {
                        let callback_sequence = match callback_id.parse::<u64>() { Ok(value) if value > call.max_callback_id && callback_id == value.to_string() => value, _ => break "invalid or duplicate multiplex callback id".to_string() };
                        if call.cancelling_since.is_some() { break "callback on cancelling multiplex call".to_string(); }
                        call.max_callback_id = callback_sequence;
                        let binding = call.host_calls.clone();
                        let sender = command_sender.clone();
                        let call_id = id.clone();
                        let callback = callback_id.clone();
                        let retained_lease = call._lease.clone();
                        let task = tokio::spawn(async move {
                            let _retained_lease = retained_lease;
                            let response = execute_callback(service, request, binding).await;
                            let _ = sender.send(CommandMessage::CallbackDone { id: call_id, callback_id: callback, response }).await;
                        });
                        call.callbacks.insert(callback_id, task);
                    }
                    MultiplexWorkerMessage::Event { await_ack, ref event_id, .. } => {
                        if await_ack {
                            let Some((event_id, sequence)) = event_id.as_ref().and_then(|id| id.parse::<u64>().ok().filter(|n| *n > call.max_event_id && n.to_string() == *id).map(|n| (id, n))) else { break "invalid event sequence".to_string(); };
                            call.max_event_id = sequence;
                            call.pending_acks.insert(event_id.clone());
                        } else if event_id.is_some() { break "event id requires acknowledgment".to_string(); }
                        if call.cancelling_since.is_some() { continue; }
                        // Retained unary output is part of the result, not queued transport
                        // credit. Duplex producers release their credit only after consumer ACK.
                        if call.events.send(Delivered { message: Ok(frame) }).is_err() {
                            request_cancel(&id, call, &controls);
                        }
                    }
                    MultiplexWorkerMessage::Response { .. } | MultiplexWorkerMessage::Cancelled { .. } => {
                        if matches!(frame, MultiplexWorkerMessage::Cancelled { .. }) && call.cancelling_since.is_none() { break "unsolicited multiplex cancellation".to_string(); }
                        if let Some(call) = active.remove(&id) {
                            call.live.store(false, Ordering::Release);
                            let _ = call.events.send(Delivered { message: Ok(frame) });
                        }
                    }
                }
            }
            command = commands.recv() => {
                match command {
                    Some(CommandMessage::Call { request, events, host_calls, registered, permit, lease }) => {
                        next_id = match next_id.checked_add(1) { Some(value) => value, None => break "multiplex call id exhausted".to_string() };
                        let id = next_id.to_string();
                        let bytes = match encode(MultiplexHostMessage::Call { call_id: id.clone(), request }) { Ok(bytes) => bytes, Err(error) => { let _ = events.send(Delivered { message: Err(error) }); continue; } };
                        active.insert(id.clone(), ActiveCall { events, host_calls: host_calls.map(|context| *context), callbacks: HashMap::new(), max_callback_id: 0, cancelling_since: None, written: false, pending_acks: std::collections::HashSet::new(), max_event_id: 0, live: Arc::new(AtomicBool::new(true)), _lease: lease.map(Arc::new) });
                        if writes.send(WriteFrame { bytes, _permit: permit, call_id: Some(id), live: None }).is_err() { break "multiplex writer queue is full or closed".to_string(); }
                        let id = next_id.to_string();
                        if registered.send(id.clone()).is_err() { if let Some(call) = active.get_mut(&id) { request_cancel(&id, call, &controls); } }
                    }
                    Some(CommandMessage::Cancel { id }) => {
                        if let Some(call) = active.get_mut(&id) { request_cancel(&id, call, &controls); }
                    }
                    Some(CommandMessage::Input { id, input, permit }) => {
                        if let Some(call) = active.get_mut(&id) {
                            if call.cancelling_since.is_none() {
                                let bytes = match encode(MultiplexHostMessage::Input { call_id: id.clone(), input }) { Ok(bytes) => bytes, Err(_) => { request_cancel(&id, call, &controls); continue; } };
                                if writes.send(WriteFrame { bytes, _permit: permit, call_id: None, live: Some(call.live.clone()) }).is_err() { break "multiplex writer closed".to_string(); }
                            }
                        }
                    }
                    Some(CommandMessage::EventAck { id, event_id }) => {
                        if let Some(call) = active.get_mut(&id) {
                            if call.cancelling_since.is_none() && call.pending_acks.remove(&event_id) {
                                let bytes = match encode(MultiplexHostMessage::EventAck { call_id: id.clone(), event_id }) { Ok(bytes) => bytes, Err(error) => break error };
                                let controls = controls.clone();
                                let live = call.live.clone();
                                tokio::spawn(async move { let _ = controls.send(ControlFrame { bytes, _permit: None, live: Some(live) }).await; });
                            }
                        }
                    }
                    Some(CommandMessage::Written(id)) => {
                        if let Some(call) = active.get_mut(&id) {
                            call.written = true;
                            if call.cancelling_since.is_some() { send_cancel(&id, &controls, call); }
                        }
                    }
                    Some(CommandMessage::CallbackDone { id, callback_id, response }) => {
                        if let Some(call) = active.get_mut(&id) {
                            if call.cancelling_since.is_none() && call.callbacks.remove(&callback_id).is_some() {
                                let bytes = match encode(MultiplexHostMessage::CallbackResult { call_id: id.clone(), callback_id: callback_id.clone(), response }) { Ok(bytes) => bytes, Err(_) => { request_cancel(&id, call, &controls); continue; } };
                                let budget = writer_budget.clone();
                                let controls = controls.clone();
                                let live = call.live.clone();
                                // Waiting inside the shared reader would prevent cancellation and
                                // ACK progress. The call owns and aborts this pending delivery.
                                call.callbacks.insert(callback_id, tokio::spawn(async move {
                                    let Ok(permit) = budget.acquire_many_owned(bytes.len().min(WRITER_BYTES) as u32).await else { return; };
                                    let _ = controls.send(ControlFrame { bytes, _permit: Some(permit), live: Some(live) }).await;
                                }));
                            }
                        }
                    }
                    Some(CommandMessage::GracefulStop(done)) => {
                        if active.is_empty() {
                            drop(writes);
                            drop(controls);
                            let _ = done.send(());
                            return;
                        }
                        let _ = done.send(());
                    }
                    Some(CommandMessage::WriterFailed(error)) => break format!("multiplex writer failed: {error}"),
                    None => break "multiplex command channel closed".to_string(),
                }
            }
            _ = ticker.tick() => {
                if active.values().any(|call| call.cancelling_since.is_some_and(|since| since.elapsed() >= CANCEL_GRACE)) { break "multiplex cancellation acknowledgment deadline exceeded".to_string(); }
            }
        }
    };
    failed.store(true, Ordering::Release);
    for call in active.values() {
        let _ = call.events.send(Delivered {
            message: Err(reason.clone()),
        });
    }
    let _ = control.terminate().await;
    drop(active);
}

fn request_cancel(id: &str, call: &mut ActiveCall, controls: &mpsc::Sender<ControlFrame>) {
    if call.cancelling_since.is_some() {
        return;
    }
    call.cancelling_since = Some(Instant::now());
    call.live.store(false, Ordering::Release);
    call.host_calls = None;
    for (_, task) in call.callbacks.drain() {
        task.abort();
    }
    if call.written {
        send_cancel(id, controls, call);
    }
}

fn send_cancel(id: &str, controls: &mpsc::Sender<ControlFrame>, call: &mut ActiveCall) {
    if let Ok(bytes) = encode(MultiplexHostMessage::Cancel {
        call_id: id.to_owned(),
    }) {
        match controls.try_send(ControlFrame {
            bytes,
            _permit: None,
            live: None,
        }) {
            Ok(()) => return,
            Err(mpsc::error::TrySendError::Full(frame)) => {
                let controls = controls.clone();
                tokio::spawn(async move {
                    let _ = controls.send(frame).await;
                });
                return;
            }
            Err(mpsc::error::TrySendError::Closed(_)) => {}
        }
    }
    // A closed control lane cannot acknowledge cancellation. Retire the child.
    call.cancelling_since = Some(Instant::now() - CANCEL_GRACE);
}

async fn execute_callback(
    service: MultiplexHostService,
    request: Value,
    context: Option<ProviderHostCallContext>,
) -> Value {
    match service {
        MultiplexHostService::ApplicationLogsV1 => {
            let result = async {
                let request: extension_contracts::ApplicationLogsRequest =
                    serde_json::from_value(request).map_err(|_| {
                        extension_contracts::HostServiceError::new("host_service_request_invalid")
                    })?;
                request.validate()?;
                let (binding, port) = context
                    .and_then(|context| context.application_logs)
                    .ok_or_else(|| {
                        extension_contracts::HostServiceError::new("host_service_not_granted")
                    })?;
                let remaining = binding.deadline_unix_ms.saturating_sub(now_unix_ms());
                if remaining <= 0 {
                    return Err(extension_contracts::HostServiceError::new(
                        "host_service_deadline_exceeded",
                    ));
                }
                if binding.deadline_unix_ms == i64::MAX {
                    return port.execute(&binding, &request).await;
                }
                tokio::time::timeout(
                    Duration::from_millis(remaining as u64),
                    port.execute(&binding, &request),
                )
                .await
                .unwrap_or_else(|_| {
                    Err(extension_contracts::HostServiceError::new(
                        "host_service_deadline_exceeded",
                    ))
                })
            }
            .await;
            exchange_service_response(result)
        }
        MultiplexHostService::HttpEgressV1 => {
            let result = async {
                let request: extension_contracts::HttpEgressRequest =
                    serde_json::from_value(request).map_err(|_| {
                        extension_contracts::HostServiceError::new("host_service_request_invalid")
                    })?;
                request.validate()?;
                let (binding, port) =
                    context
                        .and_then(|context| context.http_egress)
                        .ok_or_else(|| {
                            extension_contracts::HostServiceError::new("host_service_not_granted")
                        })?;
                let remaining = binding.deadline_unix_ms.saturating_sub(now_unix_ms());
                if remaining <= 0 {
                    return Err(extension_contracts::HostServiceError::new(
                        "host_service_deadline_exceeded",
                    ));
                }
                if binding.deadline_unix_ms == i64::MAX {
                    return port.execute(&binding, &request).await;
                }
                tokio::time::timeout(
                    Duration::from_millis(remaining as u64),
                    port.execute(&binding, &request),
                )
                .await
                .unwrap_or_else(|_| {
                    Err(extension_contracts::HostServiceError::new(
                        "host_service_deadline_exceeded",
                    ))
                })
            }
            .await;
            exchange_service_response(result)
        }
        MultiplexHostService::PluginDataV1 => {
            let result = async {
                let request: PluginDataRequest = serde_json::from_value(request)
                    .map_err(|_| PluginDataError::invalid("plugin_data_request_invalid"))?;
                request.validate()?;
                let context = context.ok_or_else(|| {
                    plugin_data_error(
                        PluginDataErrorKind::PermissionDenied,
                        "runtime_host_call_not_granted",
                        false,
                    )
                })?;
                let remaining = context
                    .binding
                    .deadline_unix_ms
                    .saturating_sub(now_unix_ms());
                if remaining <= 0 {
                    return Err(plugin_data_error(
                        PluginDataErrorKind::DeadlineExceeded,
                        "plugin_data_deadline",
                        false,
                    ));
                }
                if context.binding.deadline_unix_ms == i64::MAX {
                    return context
                        .plugin_data
                        .execute(&context.binding, &request)
                        .await;
                }
                tokio::time::timeout(
                    Duration::from_millis(remaining as u64),
                    context.plugin_data.execute(&context.binding, &request),
                )
                .await
                .unwrap_or_else(|_| {
                    Err(plugin_data_error(
                        PluginDataErrorKind::DeadlineExceeded,
                        "plugin_data_deadline",
                        false,
                    ))
                })
            }
            .await;
            match result {
                Ok(value) => serde_json::json!({"result":value}),
                Err(error) => serde_json::json!({"error":error}),
            }
        }
        MultiplexHostService::FileReadV1 => {
            let result = match context.and_then(|c| c.file_read) {
                Some(call) => call.execute(request).await,
                None => Err(extension_contracts::FileReadError::new(
                    "file_read_not_granted",
                )),
            };
            match result {
                Ok(value) => serde_json::json!({"result":value}),
                Err(error) => serde_json::json!({"error":error}),
            }
        }
        MultiplexHostService::FileImportV1 => {
            use extension_contracts::{FileImportError, FileImportRequest};
            let result = async {
                let request: FileImportRequest = serde_json::from_value(request)
                    .map_err(|_| FileImportError::new("file_import_request_invalid"))?;
                request.validate()?;
                let (binding, port) = context
                    .and_then(|context| context.file_import)
                    .ok_or_else(|| FileImportError::new("file_import_not_granted"))?;
                let remaining = binding.deadline_unix_ms.saturating_sub(now_unix_ms());
                if remaining <= 0 {
                    return Err(FileImportError::new("file_import_deadline_exceeded"));
                }
                if binding.deadline_unix_ms == i64::MAX {
                    return port.execute(&binding, &request).await;
                }
                tokio::time::timeout(
                    Duration::from_millis(remaining as u64),
                    port.execute(&binding, &request),
                )
                .await
                .unwrap_or_else(|_| Err(FileImportError::new("file_import_deadline_exceeded")))
            }
            .await;
            match result {
                Ok(value) => serde_json::json!({"result":value}),
                Err(error) => serde_json::json!({"error":error}),
            }
        }
        MultiplexHostService::PluginCredentialV1 => {
            use extension_contracts::{PluginCredentialError, PluginCredentialRequest};
            let result = async {
                let request: PluginCredentialRequest = serde_json::from_value(request)
                    .map_err(|_| PluginCredentialError::new("plugin_credential_request_invalid"))?;
                request.validate()?;
                let (binding, port) = context
                    .and_then(|context| context.plugin_credentials)
                    .ok_or_else(|| PluginCredentialError::new("plugin_credential_not_granted"))?;
                let remaining = binding.deadline_unix_ms.saturating_sub(now_unix_ms());
                if remaining <= 0 {
                    return Err(PluginCredentialError::new("plugin_credential_deadline"));
                }
                if binding.deadline_unix_ms == i64::MAX {
                    return port.execute(&binding, &request).await;
                }
                tokio::time::timeout(
                    Duration::from_millis(remaining as u64),
                    port.execute(&binding, &request),
                )
                .await
                .unwrap_or_else(|_| Err(PluginCredentialError::new("plugin_credential_deadline")))
            }
            .await;
            match result {
                Ok(value) => serde_json::json!({"result":value}),
                Err(error) => serde_json::json!({"error":error}),
            }
        }
    }
}

#[cfg(test)]
#[path = "../_tests/plugin_credential_callbacks.rs"]
mod credential_tests;

#[cfg(test)]
#[path = "../_tests/file_import_callbacks.rs"]
mod file_import_tests;

#[cfg(test)]
#[path = "../_tests/exchange_host_services.rs"]
mod exchange_host_services;

fn exchange_service_response<T: serde::Serialize>(
    result: Result<T, extension_contracts::HostServiceError>,
) -> Value {
    let value = match result {
        Ok(value) => serde_json::json!({"result":value}),
        Err(error) => serde_json::json!({"error":error}),
    };
    value
}

#[cfg(all(test, unix))]
#[path = "../_tests/multiplex_payloads.rs"]
mod multiplex_payloads;
