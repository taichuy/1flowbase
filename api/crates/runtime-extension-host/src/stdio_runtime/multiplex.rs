use super::*;
use extension_contracts::{
    MultiplexEnvelope, MultiplexHostMessage, MultiplexHostService, MultiplexWorkerMessage,
    PluginDataRequest, MULTIPLEX_CALL_EVENT_BUDGET_BYTES, MULTIPLEX_MAX_FRAME_BYTES,
    MULTIPLEX_OUTPUT_BUDGET_BYTES,
};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::{mpsc, oneshot, OwnedSemaphorePermit, Semaphore};

const QUEUE_CAPACITY: usize = 8;
const WRITER_BYTES: usize = MULTIPLEX_OUTPUT_BUDGET_BYTES;
const CALL_EVENT_BYTES: usize = MULTIPLEX_CALL_EVENT_BUDGET_BYTES + 1;
const CANCEL_GRACE: Duration = Duration::from_secs(2);

struct Delivered {
    message: Result<MultiplexWorkerMessage, String>,
    _permit: Option<OwnedSemaphorePermit>,
}

struct WriteFrame {
    bytes: Vec<u8>,
    _permit: OwnedSemaphorePermit,
    call_id: Option<String>,
}

struct ControlFrame {
    bytes: Vec<u8>,
    _permit: Option<OwnedSemaphorePermit>,
}

enum CommandMessage {
    Call {
        request: Value,
        events: mpsc::UnboundedSender<Delivered>,
        host_calls: Option<ProviderHostCallContext>,
        registered: oneshot::Sender<String>,
        permit: OwnedSemaphorePermit,
        lease: Option<Box<dyn Send + Sync>>,
    },
    Cancel {
        id: String,
    },
    CallbackDone {
        id: String,
        callback_id: String,
        response: Value,
    },
    WriterFailed(String),
    Written(String),
}

struct ActiveCall {
    events: mpsc::UnboundedSender<Delivered>,
    event_budget: Arc<Semaphore>,
    host_calls: Option<ProviderHostCallContext>,
    callbacks: HashMap<String, tokio::task::JoinHandle<()>>,
    max_callback_id: u64,
    cancelling_since: Option<Instant>,
    written: bool,
    _lease: Option<Box<dyn Send + Sync>>,
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
        let process = spawn_worker_process(&path, &limits)?;
        let control = process.control.clone();
        let stdin = process.stdin;
        let stdout = process.stdout.into_inner().into_inner();
        let (commands, receiver) = mpsc::channel(QUEUE_CAPACITY);
        let (writes, write_receiver) = mpsc::unbounded_channel();
        let (controls, control_receiver) = mpsc::channel(QUEUE_CAPACITY);
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
                reader_commands,
                writes,
                controls,
                actor_writer_budget,
                actor_control,
                actor_failed,
            )
            .await;
        });
        Ok(Self {
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
        if request_bytes.len() + 256 > MULTIPLEX_MAX_FRAME_BYTES {
            return Err(transport_error("multiplex request frame too large"));
        }
        let permit = self
            .writer_budget
            .clone()
            .acquire_many_owned((request_bytes.len() + 256) as u32)
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
                    .and_then(|context| context.host_calls.clone()),
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
        let mut timeout = ProviderStreamTimeoutState::new();
        let mut output_events = Vec::new();
        let mut retained_event_permits = Vec::new();
        let mut first_error: Option<ProviderRuntimeError> = None;
        loop {
            let (duration, kind, timeout_ms) = timeout.next_read_timeout(limits);
            let mut message = match tokio::time::timeout(duration, receiver.recv()).await {
                Ok(Some(message)) => message,
                Ok(None) => {
                    return Err(prefer_first_error(
                        &first_error,
                        transport_error("multiplex call channel closed"),
                    ))
                }
                Err(_) => {
                    return Err(prefer_first_error(
                        &first_error,
                        provider_timeout_error(kind, timeout_ms),
                    ))
                }
            };
            let frame = message
                .message
                .map_err(|error| prefer_first_error(&first_error, transport_error(&error)))?;
            match frame {
                MultiplexWorkerMessage::Event { event, .. } => {
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
                    // Retained output is part of this call's same byte budget;
                    // dequeuing must not turn bounded buffering into an unbounded Vec.
                    if let Some(permit) = message._permit.take() {
                        retained_event_permits.push(permit);
                    }
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
            None => self.control.terminate().await,
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
    if bytes.len() > MULTIPLEX_MAX_FRAME_BYTES {
        return Err("multiplex frame too large".into());
    }
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
        // The extra byte is reserved for a newline, not undelimited payload.
        let terminated = available.get(count - 1) == Some(&b'\n');
        if bytes.len() + count > MULTIPLEX_MAX_FRAME_BYTES + usize::from(terminated) {
            return Err("multiplex frame too large".into());
        }
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
            control = controls.recv() => control.map(|frame| (frame.bytes, frame._permit, None)),
            data = writes.recv() => data.map(|frame| (frame.bytes, Some(frame._permit), frame.call_id)),
        };
        let Some((bytes, permit, call_id)) = frame else {
            return;
        };
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
    command_sender: mpsc::Sender<CommandMessage>,
    writes: mpsc::UnboundedSender<WriteFrame>,
    controls: mpsc::Sender<ControlFrame>,
    writer_budget: Arc<Semaphore>,
    control: ProviderWorkerProcessControl,
    failed: Arc<AtomicBool>,
) {
    let mut reader = BufReader::new(stdout);
    // Kept outside select: a command may interrupt a partially read frame.
    let mut pending_frame = Vec::new();
    let mut active: HashMap<String, ActiveCall> = HashMap::new();
    let mut next_id = 0_u64;
    let mut ticker = tokio::time::interval(Duration::from_millis(100));
    let reason = loop {
        tokio::select! {
            frame = read_frame(&mut reader, &mut pending_frame) => {
                let (frame, frame_bytes) = match frame { Ok(Some(frame)) => frame, Ok(None) => break "multiplex worker stdout closed".to_string(), Err(error) => break error };
                let id = match &frame { MultiplexWorkerMessage::Event { call_id, .. } | MultiplexWorkerMessage::Response { call_id, .. } | MultiplexWorkerMessage::Cancelled { call_id } | MultiplexWorkerMessage::Callback { call_id, .. } => call_id.clone() };
                let Some(call) = active.get_mut(&id) else { break format!("unknown or duplicate multiplex call id: {id}"); };
                match frame {
                    MultiplexWorkerMessage::Callback { callback_id, service, request, .. } => {
                        let callback_sequence = match callback_id.parse::<u64>() { Ok(value) if value > call.max_callback_id && callback_id == value.to_string() => value, _ => break "invalid or duplicate multiplex callback id".to_string() };
                        if call.cancelling_since.is_some() { break "callback on cancelling multiplex call".to_string(); }
                        call.max_callback_id = callback_sequence;
                        if service != MultiplexHostService::PluginDataV1 { break "unsupported multiplex host service".to_string(); }
                        let parsed: PluginDataRequest = match serde_json::from_value(request) { Ok(request) => request, Err(error) => break format!("invalid multiplex host request: {error}") };
                        if let Err(error) = parsed.validate() { break format!("invalid multiplex host request: {}", error.code); }
                        let binding = call.host_calls.clone();
                        let sender = command_sender.clone();
                        let call_id = id.clone();
                        let callback = callback_id.clone();
                        let task = tokio::spawn(async move {
                            let result = if let Some(context) = binding {
                                let remaining = context.binding.deadline_unix_ms.saturating_sub(now_unix_ms());
                                if remaining <= 0 { Err(plugin_data_error(PluginDataErrorKind::DeadlineExceeded, "plugin_data_deadline", false)) }
                                else { match tokio::time::timeout(Duration::from_millis(remaining as u64), context.plugin_data.execute(&context.binding, &parsed)).await {
                                    Ok(result) => result,
                                    Err(_) => Err(plugin_data_error(PluginDataErrorKind::DeadlineExceeded, "plugin_data_deadline", false)),
                                }}
                            } else { Err(plugin_data_error(PluginDataErrorKind::PermissionDenied, "runtime_host_call_not_granted", false)) };
                            let response = match result { Ok(value) => serde_json::json!({"result":value}), Err(error) => serde_json::json!({"error":error}) };
                            let _ = sender.send(CommandMessage::CallbackDone { id: call_id, callback_id: callback, response }).await;
                        });
                        call.callbacks.insert(callback_id, task);
                    }
                    MultiplexWorkerMessage::Event { .. } => {
                        if call.cancelling_since.is_some() { continue; }
                        let permit = call.event_budget.clone().try_acquire_many_owned(frame_bytes as u32);
                        if let Ok(permit) = permit {
                            if call.events.send(Delivered { message: Ok(frame), _permit: Some(permit) }).is_ok() { continue; }
                        }
                        request_cancel(&id, call, &controls);
                    }
                    MultiplexWorkerMessage::Response { .. } | MultiplexWorkerMessage::Cancelled { .. } => {
                        if matches!(frame, MultiplexWorkerMessage::Cancelled { .. }) && call.cancelling_since.is_none() { break "unsolicited multiplex cancellation".to_string(); }
                        if let Some(call) = active.remove(&id) {
                            if let Ok(permit) = call.event_budget.clone().try_acquire_many_owned(frame_bytes as u32) {
                                let _ = call.events.send(Delivered { message: Ok(frame), _permit: Some(permit) });
                            }
                        }
                    }
                }
            }
            command = commands.recv() => {
                match command {
                    Some(CommandMessage::Call { request, events, host_calls, registered, permit, lease }) => {
                        next_id = match next_id.checked_add(1) { Some(value) => value, None => break "multiplex call id exhausted".to_string() };
                        let id = next_id.to_string();
                        let bytes = match encode(MultiplexHostMessage::Call { call_id: id.clone(), request }) { Ok(bytes) => bytes, Err(error) => { let _ = events.send(Delivered { message: Err(error), _permit: None }); continue; } };
                        active.insert(id.clone(), ActiveCall { events, event_budget: Arc::new(Semaphore::new(CALL_EVENT_BYTES)), host_calls, callbacks: HashMap::new(), max_callback_id: 0, cancelling_since: None, written: false, _lease: lease });
                        if writes.send(WriteFrame { bytes, _permit: permit, call_id: Some(id) }).is_err() { break "multiplex writer queue is full or closed".to_string(); }
                        let id = next_id.to_string();
                        if registered.send(id.clone()).is_err() { if let Some(call) = active.get_mut(&id) { request_cancel(&id, call, &controls); } }
                    }
                    Some(CommandMessage::Cancel { id }) => {
                        if let Some(call) = active.get_mut(&id) { request_cancel(&id, call, &controls); }
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
                                let bytes = match encode(MultiplexHostMessage::CallbackResult { call_id: id.clone(), callback_id, response }) { Ok(bytes) => bytes, Err(_) => { request_cancel(&id, call, &controls); continue; } };
                                let permit = match writer_budget.clone().try_acquire_many_owned(bytes.len() as u32) { Ok(permit) => permit, Err(_) => { request_cancel(&id, call, &controls); continue; } };
                                if controls.try_send(ControlFrame { bytes, _permit: Some(permit) }).is_err() { request_cancel(&id, call, &controls); }
                            }
                        }
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
            _permit: None,
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
