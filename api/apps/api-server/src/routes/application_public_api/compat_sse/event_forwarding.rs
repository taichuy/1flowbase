use super::*;
use crate::routes::application_public_api::delivery_receipt::{
    RuntimeEventDeliveryReceipt, RuntimeEventDeliverySettler,
};
use control_plane::ports::{
    RuntimeEventDeliveryBatch, RuntimeEventDeliveryClosedProof, RuntimeEventDeliveryScope,
};
use std::collections::{HashMap, HashSet};

pub(super) struct SubscribedCompatibleTypedEventStream {
    pub(super) terminal_dependencies: NativeRunTerminalDependencies,
    pub(super) initial_run: NativeRunResult,
    pub(super) delivery_policy: CompatibleDeliveryPolicy,
    pub(super) delivery_scope: RuntimeEventDeliveryScope,
    pub(super) from_sequence: Option<i64>,
    pub(super) durable_round_replay: Vec<RuntimeEventEnvelope>,
    pub(super) ignored_waiting_callback_task_id: Option<uuid::Uuid>,
    pub(super) subscription: control_plane::ports::RuntimeEventSubscription,
    pub(super) sender: mpsc::Sender<CompatibleProjectionInput>,
}

/// Forwards cursor-ordered facts to the protocol projector. Committed tool
/// deliveries are claimed here, but their durable ACK is settled only from the
/// protocol writer's receipt; this task waits for every receipt to settle.
pub(super) async fn send_subscribed_compatible_typed_event_stream(
    stream: SubscribedCompatibleTypedEventStream,
) {
    let settler = RuntimeEventDeliverySettler::spawn(stream.terminal_dependencies.clone());
    forward_subscribed_typed_events(stream, &settler).await;
    settler.settled().await;
}

// IDs select candidate buckets only; complete JSON equality remains authoritative.
#[derive(Default)]
struct EventTypeBucket {
    identified: HashMap<String, Vec<Value>>,
    unidentified: HashSet<Value>,
}
#[derive(Default)]
struct ForwardedCommittedEvents(HashMap<String, EventTypeBucket>);
fn payload_partition(payload: &Value) -> Option<&str> {
    payload
        .get("call_id")
        .and_then(Value::as_str)
        .or_else(|| {
            payload
                .get("item")
                .and_then(|item| item.get("call_id"))
                .and_then(Value::as_str)
        })
        .or_else(|| {
            payload
                .get("item")
                .and_then(|item| item.get("id"))
                .and_then(Value::as_str)
        })
}
impl ForwardedCommittedEvents {
    fn contains(&self, event_type: &str, payload: &Value) -> bool {
        let Some(bucket) = self.0.get(event_type) else {
            return false;
        };
        match payload_partition(payload) {
            Some(id) => bucket
                .identified
                .get(id)
                .is_some_and(|values| values.iter().any(|value| value == payload)),
            None => bucket.unidentified.contains(payload),
        }
    }
    // Production insertion follows a negative contains result; preserves original Vec insertion semantics.
    fn insert(&mut self, event_type: String, payload: Value) {
        let bucket = self.0.entry(event_type).or_default();
        match payload_partition(&payload) {
            Some(id) => match bucket.identified.entry(id.to_owned()) {
                std::collections::hash_map::Entry::Vacant(entry) => {
                    entry.insert(vec![payload]);
                }
                std::collections::hash_map::Entry::Occupied(mut entry) => {
                    entry.get_mut().push(payload);
                }
            },
            None => {
                bucket.unidentified.insert(payload);
            }
        }
    }
}

struct TypedForwarding<'a> {
    terminal_dependencies: &'a NativeRunTerminalDependencies,
    initial_run: &'a Arc<NativeRunResult>,
    sender: &'a mpsc::Sender<CompatibleProjectionInput>,
    settler: &'a RuntimeEventDeliverySettler,
    delivery_policy: CompatibleDeliveryPolicy,
    ignored_waiting_callback_task_id: Option<uuid::Uuid>,
    last_forwarded_sequence: i64,
    emitted_answer_delta: bool,
    forwarded_committed_events: ForwardedCommittedEvents,
    eligible_committed_events: ForwardedCommittedEvents,
    closed_proof: Option<RuntimeEventDeliveryClosedProof>,
}

async fn forward_subscribed_typed_events(
    stream: SubscribedCompatibleTypedEventStream,
    settler: &RuntimeEventDeliverySettler,
) {
    // The only synthesized typed facts are canonical answer deltas recovered
    // from a durable terminal snapshot when the live lane did not deliver them.
    let SubscribedCompatibleTypedEventStream {
        terminal_dependencies,
        initial_run,
        delivery_policy,
        delivery_scope,
        from_sequence,
        durable_round_replay,
        ignored_waiting_callback_task_id,
        mut subscription,
        sender,
    } = stream;
    let initial_run = Arc::new(initial_run);
    let mut forwarding = TypedForwarding {
        terminal_dependencies: &terminal_dependencies,
        initial_run: &initial_run,
        sender: &sender,
        settler,
        delivery_policy,
        ignored_waiting_callback_task_id,
        last_forwarded_sequence: from_sequence.unwrap_or(0),
        emitted_answer_delta: false,
        forwarded_committed_events: ForwardedCommittedEvents::default(),
        eligible_committed_events: ForwardedCommittedEvents::default(),
        closed_proof: None,
    };
    let mut delivery_claims = if delivery_policy == CompatibleDeliveryPolicy::ScopedResponses {
        match refresh_delivery_batch(&mut forwarding, delivery_scope).await {
            Ok(claims) => claims,
            Err(error) => {
                fail_delivery_projection(&forwarding, &error).await;
                return;
            }
        }
    } else {
        Vec::new()
    };
    if !durable_round_replay.is_empty() {
        forwarding.last_forwarded_sequence = -1;
    }
    for event in durable_round_replay {
        if delivery_policy == CompatibleDeliveryPolicy::ScopedResponses
            && is_public_terminal_runtime_event(&event.event_type)
            && forwarding.closed_proof.is_none()
        {
            match refresh_delivery_batch(&mut forwarding, delivery_scope).await {
                Ok(claims) => delivery_claims.extend(claims),
                Err(error) => {
                    release_delivery_claims(settler, delivery_claims);
                    fail_delivery_projection(&forwarding, &error).await;
                    return;
                }
            }
        }
        if is_public_terminal_runtime_event(&event.event_type)
            && !delivery_claims.is_empty()
            && forward_pending_delivery_claims(&mut forwarding, &mut delivery_claims).await
        {
            release_delivery_claims(settler, delivery_claims);
            return;
        }
        let matching_claim = delivery_claims
            .iter()
            .position(|delivery| runtime_delivery_matches_envelope(delivery, &event));
        let receipt = matching_claim.map(|index| {
            let delivery = delivery_claims.remove(index);
            settler.receipt(delivery)
        });
        if forward_ordered_typed_events(&mut forwarding, vec![(event, receipt)]).await {
            release_delivery_claims(settler, delivery_claims);
            return;
        }
    }
    // Database and local stream generations have independent sequence spaces.
    forwarding.last_forwarded_sequence = from_sequence.unwrap_or(0);
    for event in subscription.replay {
        if delivery_policy == CompatibleDeliveryPolicy::ScopedResponses
            && is_public_terminal_runtime_event(&event.event_type)
            && forwarding.closed_proof.is_none()
        {
            match refresh_delivery_batch(&mut forwarding, delivery_scope).await {
                Ok(claims) => delivery_claims.extend(claims),
                Err(error) => {
                    release_delivery_claims(settler, delivery_claims);
                    fail_delivery_projection(&forwarding, &error).await;
                    return;
                }
            }
        }
        if is_public_terminal_runtime_event(&event.event_type)
            && !delivery_claims.is_empty()
            && forward_pending_delivery_claims(&mut forwarding, &mut delivery_claims).await
        {
            return;
        }
        let matching_claim = delivery_claims
            .iter()
            .position(|delivery| runtime_delivery_matches_envelope(delivery, &event));
        let receipt = matching_claim.map(|index| {
            let delivery = delivery_claims.remove(index);
            settler.receipt(delivery)
        });
        if forward_ordered_typed_events(&mut forwarding, vec![(event, receipt)]).await {
            release_delivery_claims(settler, delivery_claims);
            return;
        }
    }
    if forward_pending_delivery_claims(&mut forwarding, &mut delivery_claims).await {
        return;
    }

    while let Some(event) = subscription.live_events.recv().await {
        if forwarding
            .forwarded_committed_events
            .contains(&event.event_type, &event.payload)
        {
            forwarding.last_forwarded_sequence =
                forwarding.last_forwarded_sequence.max(event.sequence);
            continue;
        }
        if is_committed_tool_delivery(&event) {
            if delivery_policy == CompatibleDeliveryPolicy::NativeRunAttachment {
                continue;
            }
            let mut claims = match refresh_delivery_batch(&mut forwarding, delivery_scope).await {
                Ok(claims) => claims,
                Err(error) => {
                    fail_delivery_projection(&forwarding, &error).await;
                    return;
                }
            };
            // A notification wakes the scoped durable batch, never a flow-wide batch.
            // Its payload is projected only if this snapshot proves membership;
            // acked/uncertain facts remain legal exact replays without a claim.
            if forward_pending_delivery_claims(&mut forwarding, &mut claims).await {
                return;
            }
            if forward_ordered_typed_events(&mut forwarding, vec![(event, None)]).await {
                return;
            }
            continue;
        }
        if delivery_policy == CompatibleDeliveryPolicy::ScopedResponses
            && is_public_terminal_runtime_event(&event.event_type)
            && forwarding.closed_proof.is_none()
        {
            let mut claims = match refresh_delivery_batch(&mut forwarding, delivery_scope).await {
                Ok(claims) => claims,
                Err(error) => {
                    fail_delivery_projection(&forwarding, &error).await;
                    return;
                }
            };
            if forward_pending_delivery_claims(&mut forwarding, &mut claims).await {
                return;
            }
        }
        if forward_ordered_typed_events(&mut forwarding, vec![(event, None)]).await {
            return;
        }
    }
}

/// Projection-local failure uses the existing protocol error path. This snapshot
/// is never persisted and never changes the business run/terminal/receipt owner.
async fn fail_delivery_projection(forwarding: &TypedForwarding<'_>, error: &anyhow::Error) {
    warn!(flow_run_id = %forwarding.initial_run.id, %error, "Responses delivery proof unavailable");
    let code = if error
        .downcast_ref::<control_plane::errors::ControlPlaneError>()
        .is_some_and(|e| {
            matches!(
                e,
                control_plane::errors::ControlPlaneError::Conflict(
                    "runtime_event_delivery_scope_unresolved"
                )
            )
        }) {
        "runtime_event_delivery_scope_unresolved"
    } else {
        "runtime_event_delivery_proof_unavailable"
    };
    let _ = forwarding
        .sender
        .send(CompatibleProjectionInput::protocol_failure(
            forwarding.initial_run,
            code,
            forwarding.last_forwarded_sequence.saturating_add(1),
        ))
        .await;
}

async fn refresh_delivery_batch(
    forwarding: &mut TypedForwarding<'_>,
    scope: RuntimeEventDeliveryScope,
) -> anyhow::Result<Vec<RuntimeEventDeliveryClaim>> {
    let batch = match forwarding
        .terminal_dependencies
        .claim_runtime_event_deliveries(
            forwarding.initial_run.id,
            scope,
            forwarding.closed_proof.is_none(),
        )
        .await
    {
        Ok(batch) => batch,
        Err(error) if forwarding.closed_proof.is_some() => {
            // A successful complete proof remains valid through a later storage fault.
            warn!(flow_run_id = %forwarding.initial_run.id, %error, "retaining closed Responses delivery proof after claim failure");
            return Ok(Vec::new());
        }
        Err(error) => return Err(error),
    };
    admit_delivery_batch(forwarding, scope, batch)
}

fn admit_delivery_batch(
    forwarding: &mut TypedForwarding<'_>,
    scope: RuntimeEventDeliveryScope,
    batch: RuntimeEventDeliveryBatch,
) -> anyhow::Result<Vec<RuntimeEventDeliveryClaim>> {
    let valid =
        batch.closed_proof.as_ref().is_none_or(|proof| {
            proof.flow_run_id == forwarding.initial_run.id && proof.scope == scope
        }) && forwarding
            .closed_proof
            .as_ref()
            .is_none_or(|proof| Some(proof) == batch.closed_proof.as_ref());
    if !valid {
        release_delivery_claims(forwarding.settler, batch.claims);
        anyhow::bail!("Responses delivery proof identity changed");
    }
    for event in batch.events {
        if !forwarding
            .eligible_committed_events
            .contains(&event.event_type, &event.payload)
        {
            forwarding
                .eligible_committed_events
                .insert(event.event_type, event.payload);
        }
    }
    if batch.claims.iter().any(|claim| {
        !forwarding
            .eligible_committed_events
            .contains(&claim.event.event_type, &claim.event.payload)
    }) {
        release_delivery_claims(forwarding.settler, batch.claims);
        anyhow::bail!("Responses claimed fact is outside delivery proof");
    }
    forwarding.closed_proof = batch.closed_proof;
    Ok(batch.claims)
}

fn is_committed_tool_delivery(event: &RuntimeEventEnvelope) -> bool {
    event
        .payload
        .get("committed_delivery")
        .and_then(Value::as_bool)
        == Some(true)
}

fn runtime_delivery_matches_envelope(
    delivery: &RuntimeEventDeliveryClaim,
    event: &RuntimeEventEnvelope,
) -> bool {
    delivery.event.event_type == event.event_type && delivery.event.payload == event.payload
}

fn release_delivery_claims(
    settler: &RuntimeEventDeliverySettler,
    deliveries: impl IntoIterator<Item = RuntimeEventDeliveryClaim>,
) {
    for delivery in deliveries {
        settler.release(delivery);
    }
}

async fn forward_pending_delivery_claims(
    forwarding: &mut TypedForwarding<'_>,
    deliveries: &mut Vec<RuntimeEventDeliveryClaim>,
) -> bool {
    // UPDATE ... RETURNING does not inherit the candidate query's ORDER BY.
    deliveries.sort_by_key(|delivery| (delivery.event.sequence, delivery.event.id));
    let mut pending = std::mem::take(deliveries).into_iter();
    while let Some(delivery) = pending.next() {
        let envelope = durable_record_to_runtime_event_envelope(delivery.event.clone());
        let run = Arc::clone(forwarding.initial_run);
        if forwarding
            .forwarded_committed_events
            .contains(&envelope.event_type, &envelope.payload)
        {
            forwarding.settler.release(delivery);
            continue;
        }
        forwarding
            .forwarded_committed_events
            .insert(envelope.event_type.clone(), envelope.payload.clone());
        let receipt = forwarding.settler.receipt(delivery);
        if forwarding
            .sender
            .send(CompatibleProjectionInput {
                run_snapshot: run,
                envelope,
                delivery: Some(receipt),
            })
            .await
            .is_err()
        {
            // The dropped input releases its own receipt; only the claims that
            // never became receipts still need an explicit release.
            release_delivery_claims(forwarding.settler, pending);
            return true;
        }
    }
    false
}

async fn forward_ordered_typed_events(
    forwarding: &mut TypedForwarding<'_>,
    events: Vec<(RuntimeEventEnvelope, Option<RuntimeEventDeliveryReceipt>)>,
) -> bool {
    for (event, receipt) in events {
        // All three input lanes share this guard. Native required_action is a
        // different, supported event; provider inbox items are never Native output.
        if forwarding.delivery_policy == CompatibleDeliveryPolicy::NativeRunAttachment
            && is_committed_tool_delivery(&event)
        {
            continue;
        }
        if is_committed_tool_delivery(&event)
            && !forwarding
                .eligible_committed_events
                .contains(&event.event_type, &event.payload)
        {
            drop(receipt);
            continue;
        }
        let Some(event) = take_ordered_compatible_event(
            event,
            &mut forwarding.last_forwarded_sequence,
            forwarding.ignored_waiting_callback_task_id,
        ) else {
            if let Some(receipt) = receipt {
                // The client cursor is already past this committed fact; the
                // client has consumed it and it must not replay.
                receipt.projected();
            }
            continue;
        };
        if is_committed_tool_delivery(&event) {
            if forwarding
                .forwarded_committed_events
                .contains(&event.event_type, &event.payload)
            {
                // This response already owns the fact. Dropping an unused
                // receipt releases its claim rather than inventing an ACK.
                drop(receipt);
                continue;
            }
            forwarding
                .forwarded_committed_events
                .insert(event.event_type.clone(), event.payload.clone());
        }
        let terminal = is_public_terminal_runtime_event(&event.event_type);
        let run = if terminal {
            let mut run = match load_durable_native_run_for_terminal_projection_with_dependencies(
                forwarding.terminal_dependencies,
                forwarding.initial_run,
            )
            .await
            {
                Ok(run) => run,
                Err(error) => {
                    warn!(
                        flow_run_id = %forwarding.initial_run.id,
                        application_id = %forwarding.initial_run.application_id,
                        error = %error,
                        "typed public terminal blocked because durable run reload failed"
                    );
                    continue;
                }
            };
            if !durable_native_run_matches_terminal(&run, &event.event_type) {
                warn!(
                    flow_run_id = %forwarding.initial_run.id,
                    durable_status = ?run.status,
                    event_type = %event.event_type,
                    "typed public terminal blocked because durable status does not match"
                );
                continue;
            }
            // Reload terminal state once, then share the immutable durable snapshot.
            if forwarding.delivery_policy == CompatibleDeliveryPolicy::ScopedResponses {
                if let Some(round_id) = forwarding.initial_run.metadata.get("response_round_id") {
                    run.metadata["response_round_id"] = round_id.clone();
                }
            }
            let run = Arc::new(run);
            if !forwarding.emitted_answer_delta {
                for answer_event in durable_canonical_partial_runtime_events_from_native_run(&run) {
                    if forwarding
                        .sender
                        .send(CompatibleProjectionInput {
                            run_snapshot: run.clone(),
                            envelope: answer_event,
                            delivery: None,
                        })
                        .await
                        .is_err()
                    {
                        return true;
                    }
                    forwarding.emitted_answer_delta = true;
                }
            }
            run
        } else {
            Arc::clone(forwarding.initial_run)
        };
        forwarding.emitted_answer_delta |= is_answer_presentation_delta(&event);
        if forwarding
            .sender
            .send(CompatibleProjectionInput {
                run_snapshot: run,
                envelope: event,
                delivery: receipt,
            })
            .await
            .is_err()
        {
            return true;
        }
        if terminal {
            return true;
        }
    }
    false
}

#[cfg(test)]
pub(super) struct SubscribedCompatibleRuntimeEventStream<F> {
    pub(super) state: Arc<ApiState>,
    pub(super) initial_run: NativeRunResult,
    pub(super) sse_projection: &'static str,
    pub(super) from_sequence: Option<i64>,
    pub(super) ignored_waiting_callback_task_id: Option<uuid::Uuid>,
    pub(super) subscription: control_plane::ports::RuntimeEventSubscription,
    pub(super) sender: mpsc::Sender<Result<Event, Infallible>>,
    pub(super) mapper: F,
}

#[cfg(test)]
pub(super) async fn send_subscribed_compatible_runtime_event_stream<F>(
    stream: SubscribedCompatibleRuntimeEventStream<F>,
) where
    F: FnMut(&NativeRunResult, RuntimeEventEnvelope) -> Vec<Result<Event, Infallible>>,
{
    let SubscribedCompatibleRuntimeEventStream {
        state,
        initial_run,
        sse_projection,
        from_sequence,
        ignored_waiting_callback_task_id,
        mut subscription,
        sender,
        mut mapper,
    } = stream;
    let mut stats = CompatibleStreamStats::default();
    let mut last_forwarded_sequence = from_sequence.unwrap_or(0);
    let mut last_forwarded_durable_sequence = durable_sequence_for_ignored_waiting_callback(
        state.as_ref(),
        initial_run.id,
        ignored_waiting_callback_task_id,
    )
    .await
    .unwrap_or(0);
    match forward_compatible_runtime_events(CompatibleRuntimeEventsForward {
        state: &state,
        initial_run: &initial_run,
        sender: &sender,
        mapper: &mut mapper,
        stats: &mut stats,
        ignored_waiting_callback_task_id,
        last_forwarded_sequence: &mut last_forwarded_sequence,
        resume_durable_sequence_before_terminal: ignored_waiting_callback_task_id
            .map(|_| &mut last_forwarded_durable_sequence),
        events: subscription.replay,
    })
    .await
    {
        CompatibleForwardOutcome::Terminal { event_type } => {
            debug!(
                flow_run_id = %initial_run.id,
                application_id = %initial_run.application_id,
                "compatible public API stream replay reached terminal event"
            );
            log_compatible_sse_closed(
                sse_projection,
                &initial_run,
                &stats,
                &event_type,
                "replay",
                false,
            );
            return;
        }
        CompatibleForwardOutcome::ClientDisconnected => {
            debug!(
                flow_run_id = %initial_run.id,
                application_id = %initial_run.application_id,
                "compatible public API stream client disconnected during replay"
            );
            log_compatible_sse_closed(
                sse_projection,
                &initial_run,
                &stats,
                "client_disconnected",
                "replay",
                true,
            );
            return;
        }
        CompatibleForwardOutcome::Open => {}
    }

    while let Some(event) = subscription.live_events.recv().await {
        let event_type = event.event_type.clone();
        match forward_compatible_runtime_events(CompatibleRuntimeEventsForward {
            state: &state,
            initial_run: &initial_run,
            sender: &sender,
            mapper: &mut mapper,
            stats: &mut stats,
            ignored_waiting_callback_task_id,
            last_forwarded_sequence: &mut last_forwarded_sequence,
            resume_durable_sequence_before_terminal: ignored_waiting_callback_task_id
                .map(|_| &mut last_forwarded_durable_sequence),
            events: vec![event],
        })
        .await
        {
            CompatibleForwardOutcome::Terminal { event_type: _ } => {
                debug!(
                    flow_run_id = %initial_run.id,
                    application_id = %initial_run.application_id,
                    event_type = %event_type,
                    "compatible public API stream reached terminal event"
                );
                log_compatible_sse_closed(
                    sse_projection,
                    &initial_run,
                    &stats,
                    &event_type,
                    "live",
                    false,
                );
                return;
            }
            CompatibleForwardOutcome::ClientDisconnected => {
                debug!(
                    flow_run_id = %initial_run.id,
                    application_id = %initial_run.application_id,
                    "compatible public API stream client disconnected"
                );
                log_compatible_sse_closed(
                    sse_projection,
                    &initial_run,
                    &stats,
                    "client_disconnected",
                    "live",
                    true,
                );
                return;
            }
            CompatibleForwardOutcome::Open => {}
        }
    }

    match *subscription.closure.borrow() {
        Some(closure) => debug!(
            flow_run_id = %initial_run.id,
            application_id = %initial_run.application_id,
            close_reason = ?closure.reason,
            final_sequence = closure.final_sequence,
            last_forwarded_sequence,
            "compatible public API runtime event stream closed"
        ),
        None => warn!(
            flow_run_id = %initial_run.id,
            application_id = %initial_run.application_id,
            last_forwarded_sequence,
            "compatible public API runtime event receiver ended without a close signal"
        ),
    }

    match drain_compatible_durable_runtime_events(CompatibleDurableRuntimeEventsForward {
        state: &state,
        initial_run: &initial_run,
        sender: &sender,
        mapper: &mut mapper,
        stats: &mut stats,
        ignored_waiting_callback_task_id,
        last_forwarded_durable_sequence: &mut last_forwarded_durable_sequence,
    })
    .await
    {
        CompatibleForwardOutcome::Terminal { event_type } => {
            log_compatible_sse_closed(
                sse_projection,
                &initial_run,
                &stats,
                &event_type,
                "stream_closed_durable_reconcile",
                false,
            );
            return;
        }
        CompatibleForwardOutcome::ClientDisconnected => {
            log_compatible_sse_closed(
                sse_projection,
                &initial_run,
                &stats,
                "client_disconnected",
                "stream_closed_durable_reconcile",
                true,
            );
            return;
        }
        CompatibleForwardOutcome::Open => {}
    }

    match emit_compatible_terminal_fallback(CompatibleTerminalFallback {
        state: &state,
        initial_run: &initial_run,
        sender: &sender,
        mapper: &mut mapper,
        stats: &mut stats,
        trigger: "stream_closed",
        warn_if_not_terminal: true,
        ignored_waiting_callback_task_id,
    })
    .await
    {
        CompatibleTerminalFallbackOutcome::Sent { event_type } => {
            log_compatible_sse_closed(
                sse_projection,
                &initial_run,
                &stats,
                &event_type,
                "stream_closed_terminal_fallback",
                false,
            );
        }
        CompatibleTerminalFallbackOutcome::ClientDisconnected { event_type } => {
            let terminal_reason = event_type.as_deref().unwrap_or("client_disconnected");
            log_compatible_sse_closed(
                sse_projection,
                &initial_run,
                &stats,
                terminal_reason,
                "stream_closed_terminal_fallback",
                true,
            );
        }
        CompatibleTerminalFallbackOutcome::IgnoredWaitingCallback => {
            log_compatible_sse_closed(
                sse_projection,
                &initial_run,
                &stats,
                "ignored_waiting_callback",
                "stream_closed_terminal_fallback",
                false,
            );
        }
        CompatibleTerminalFallbackOutcome::NotTerminal => {
            log_compatible_sse_closed(
                sse_projection,
                &initial_run,
                &stats,
                "stream_closed_before_terminal",
                "stream_closed",
                false,
            );
        }
    }
}

#[cfg(test)]
#[cfg(test)]
pub(super) async fn send_compatible_runtime_event_stream<F>(
    state: Arc<ApiState>,
    initial_run: NativeRunResult,
    sse_projection: &'static str,
    from_sequence: Option<i64>,
    ignored_waiting_callback_task_id: Option<uuid::Uuid>,
    sender: mpsc::Sender<Result<Event, Infallible>>,
    mapper: F,
) where
    F: FnMut(&NativeRunResult, RuntimeEventEnvelope) -> Vec<Result<Event, Infallible>>,
{
    let stream = state.runtime_event_stream.clone();
    let Ok(subscription) = stream.subscribe(initial_run.id, from_sequence).await else {
        warn!(
            flow_run_id = %initial_run.id,
            application_id = %initial_run.application_id,
            "failed to subscribe compatible public API runtime event stream"
        );
        return;
    };
    send_subscribed_compatible_runtime_event_stream(SubscribedCompatibleRuntimeEventStream {
        state,
        initial_run,
        sse_projection,
        from_sequence,
        ignored_waiting_callback_task_id,
        subscription,
        sender,
        mapper,
    })
    .await;
}

#[cfg(test)]
async fn durable_sequence_for_ignored_waiting_callback(
    state: &ApiState,
    run_id: uuid::Uuid,
    ignored_waiting_callback_task_id: Option<uuid::Uuid>,
) -> Option<i64> {
    let ignored_task_id = ignored_waiting_callback_task_id?;
    state
        .store
        .get_original_callback_waiting_boundary(run_id, ignored_task_id)
        .await
        .ok()?
        .map(|boundary| boundary.original.event_sequence)
}

#[cfg(test)]
fn log_compatible_sse_closed(
    sse_projection: &'static str,
    run: &NativeRunResult,
    stats: &CompatibleStreamStats,
    terminal_reason: &str,
    close_trigger: &str,
    client_disconnected: bool,
) {
    info!(
        flow_run_id = %run.id,
        application_id = %run.application_id,
        sse_projection = %sse_projection,
        emitted_content = stats.emitted_content(),
        content_bytes = stats.emitted_content_bytes,
        terminal_reason = %terminal_reason,
        close_trigger = %close_trigger,
        client_disconnected = client_disconnected,
        "compatible public API SSE stream closed"
    );
}

use super::super::stream_terminal_fallback::durable_record_to_runtime_event_envelope;
pub(super) use super::super::stream_terminal_fallback::durable_round_prefix;

#[cfg(test)]
struct CompatibleRuntimeEventsForward<'a, F> {
    state: &'a ApiState,
    initial_run: &'a NativeRunResult,
    sender: &'a mpsc::Sender<Result<Event, Infallible>>,
    mapper: &'a mut F,
    stats: &'a mut CompatibleStreamStats,
    ignored_waiting_callback_task_id: Option<uuid::Uuid>,
    last_forwarded_sequence: &'a mut i64,
    resume_durable_sequence_before_terminal: Option<&'a mut i64>,
    events: Vec<RuntimeEventEnvelope>,
}

#[cfg(test)]
enum CompatibleForwardOutcome {
    Open,
    Terminal { event_type: String },
    ClientDisconnected,
}

pub(super) fn take_ordered_compatible_event(
    event: RuntimeEventEnvelope,
    last_forwarded_sequence: &mut i64,
    ignored_waiting_callback_task_id: Option<uuid::Uuid>,
) -> Option<RuntimeEventEnvelope> {
    if event.sequence <= *last_forwarded_sequence {
        return None;
    }
    *last_forwarded_sequence = event.sequence;
    if is_ignored_waiting_callback(&event, ignored_waiting_callback_task_id) {
        return None;
    }
    Some(event)
}

#[cfg(test)]
async fn forward_compatible_runtime_events<F>(
    forward: CompatibleRuntimeEventsForward<'_, F>,
) -> CompatibleForwardOutcome
where
    F: FnMut(&NativeRunResult, RuntimeEventEnvelope) -> Vec<Result<Event, Infallible>>,
{
    let CompatibleRuntimeEventsForward {
        state,
        initial_run,
        sender,
        mapper,
        stats,
        ignored_waiting_callback_task_id,
        last_forwarded_sequence,
        resume_durable_sequence_before_terminal,
        events,
    } = forward;
    let mut resume_durable_sequence_before_terminal = resume_durable_sequence_before_terminal;

    for event in events {
        let Some(event) = take_ordered_compatible_event(
            event,
            last_forwarded_sequence,
            ignored_waiting_callback_task_id,
        ) else {
            continue;
        };

        let is_terminal = is_public_terminal_runtime_event(&event.event_type);
        if is_terminal && ignored_waiting_callback_task_id.is_some() {
            if let Some(last_forwarded_durable_sequence) =
                resume_durable_sequence_before_terminal.as_deref_mut()
            {
                match drain_compatible_durable_runtime_events(
                    CompatibleDurableRuntimeEventsForward {
                        state,
                        initial_run,
                        sender,
                        mapper,
                        stats,
                        ignored_waiting_callback_task_id,
                        last_forwarded_durable_sequence,
                    },
                )
                .await
                {
                    CompatibleForwardOutcome::Terminal { event_type } => {
                        return CompatibleForwardOutcome::Terminal { event_type };
                    }
                    CompatibleForwardOutcome::ClientDisconnected => {
                        return CompatibleForwardOutcome::ClientDisconnected;
                    }
                    CompatibleForwardOutcome::Open => {}
                }
            }
        }
        match forward_single_compatible_runtime_event(
            state,
            initial_run,
            sender,
            mapper,
            stats,
            event,
        )
        .await
        {
            CompatibleForwardOutcome::Terminal { event_type } => {
                return CompatibleForwardOutcome::Terminal { event_type };
            }
            CompatibleForwardOutcome::ClientDisconnected => {
                return CompatibleForwardOutcome::ClientDisconnected;
            }
            CompatibleForwardOutcome::Open => {}
        }
    }

    CompatibleForwardOutcome::Open
}

#[cfg(test)]
async fn forward_compatible_runtime_events_without_resume_durable_prefix<F>(
    forward: CompatibleRuntimeEventsForward<'_, F>,
) -> CompatibleForwardOutcome
where
    F: FnMut(&NativeRunResult, RuntimeEventEnvelope) -> Vec<Result<Event, Infallible>>,
{
    let CompatibleRuntimeEventsForward {
        state,
        initial_run,
        sender,
        mapper,
        stats,
        ignored_waiting_callback_task_id,
        last_forwarded_sequence,
        resume_durable_sequence_before_terminal: _,
        events,
    } = forward;
    for event in events {
        let Some(event) = take_ordered_compatible_event(
            event,
            last_forwarded_sequence,
            ignored_waiting_callback_task_id,
        ) else {
            continue;
        };

        match forward_single_compatible_runtime_event(
            state,
            initial_run,
            sender,
            mapper,
            stats,
            event,
        )
        .await
        {
            CompatibleForwardOutcome::Terminal { event_type } => {
                return CompatibleForwardOutcome::Terminal { event_type };
            }
            CompatibleForwardOutcome::ClientDisconnected => {
                return CompatibleForwardOutcome::ClientDisconnected;
            }
            CompatibleForwardOutcome::Open => {}
        }
    }

    CompatibleForwardOutcome::Open
}

#[cfg(test)]
async fn forward_single_compatible_runtime_event<F>(
    state: &ApiState,
    initial_run: &NativeRunResult,
    sender: &mpsc::Sender<Result<Event, Infallible>>,
    mapper: &mut F,
    stats: &mut CompatibleStreamStats,
    event: RuntimeEventEnvelope,
) -> CompatibleForwardOutcome
where
    F: FnMut(&NativeRunResult, RuntimeEventEnvelope) -> Vec<Result<Event, Infallible>>,
{
    if !stats.claim_runtime_event(&event) {
        return CompatibleForwardOutcome::Open;
    }
    let is_terminal = is_public_terminal_runtime_event(&event.event_type);
    let terminal_run;
    let run = if is_terminal {
        terminal_run = match load_durable_native_run_for_terminal_projection_with_dependencies(
            &NativeRunTerminalDependencies::new(
                state.store.clone(),
                state.runtime_engine.clone(),
                state.provider_runtime.clone(),
                state.provider_secret_master_key.clone(),
                state.model_billing_require_provider_usage,
                state.infrastructure.provider_transport_store(),
                state.runtime_event_stream.clone(),
            ),
            initial_run,
        )
        .await
        {
            Ok(run) => run,
            Err(error) => {
                warn!(
                    flow_run_id = %initial_run.id,
                    application_id = %initial_run.application_id,
                    error = %error,
                    "compatible public terminal blocked because durable run reload failed"
                );
                return CompatibleForwardOutcome::Open;
            }
        };
        if !durable_native_run_matches_terminal(&terminal_run, &event.event_type) {
            warn!(
                flow_run_id = %initial_run.id,
                durable_status = ?terminal_run.status,
                event_type = %event.event_type,
                "compatible public terminal blocked because durable status does not match"
            );
            return CompatibleForwardOutcome::Open;
        }
        &terminal_run
    } else {
        initial_run
    };
    if is_terminal && !stats.emitted_content() {
        for answer_event in durable_canonical_partial_runtime_events_from_native_run(run) {
            let events = mapper(run, answer_event.clone());
            let emitted_public_event = !events.is_empty();
            if !send_compatible_sse_events(sender, events).await {
                return CompatibleForwardOutcome::ClientDisconnected;
            }
            stats.record_sent_runtime_event(run, &answer_event, emitted_public_event);
        }
    }
    let event_type = event.event_type.clone();
    let events = mapper(run, event.clone());
    let emitted_public_event = !events.is_empty();
    if !send_compatible_sse_events(sender, events).await {
        return CompatibleForwardOutcome::ClientDisconnected;
    }
    stats.record_sent_runtime_event(run, &event, emitted_public_event);
    if is_terminal {
        return CompatibleForwardOutcome::Terminal { event_type };
    }
    CompatibleForwardOutcome::Open
}

#[cfg(test)]
struct CompatibleDurableRuntimeEventsForward<'a, F> {
    state: &'a ApiState,
    initial_run: &'a NativeRunResult,
    sender: &'a mpsc::Sender<Result<Event, Infallible>>,
    mapper: &'a mut F,
    stats: &'a mut CompatibleStreamStats,
    ignored_waiting_callback_task_id: Option<uuid::Uuid>,
    last_forwarded_durable_sequence: &'a mut i64,
}

#[cfg(test)]
async fn drain_compatible_durable_runtime_events<F>(
    forward: CompatibleDurableRuntimeEventsForward<'_, F>,
) -> CompatibleForwardOutcome
where
    F: FnMut(&NativeRunResult, RuntimeEventEnvelope) -> Vec<Result<Event, Infallible>>,
{
    let CompatibleDurableRuntimeEventsForward {
        state,
        initial_run,
        sender,
        mapper,
        stats,
        ignored_waiting_callback_task_id,
        last_forwarded_durable_sequence,
    } = forward;

    if ignored_waiting_callback_task_id.is_some() && *last_forwarded_durable_sequence == 0 {
        if let Some(sequence) = durable_sequence_for_ignored_waiting_callback(
            state,
            initial_run.id,
            ignored_waiting_callback_task_id,
        )
        .await
        {
            *last_forwarded_durable_sequence = sequence;
        } else {
            return CompatibleForwardOutcome::Open;
        }
    }

    let records = match state
        .store
        .list_runtime_events(initial_run.id, *last_forwarded_durable_sequence)
        .await
    {
        Ok(records) => records,
        Err(error) => {
            warn!(
                flow_run_id = %initial_run.id,
                application_id = %initial_run.application_id,
                error = %error,
                "failed to drain compatible public API durable runtime events"
            );
            return CompatibleForwardOutcome::Open;
        }
    };
    let events = records
        .into_iter()
        .map(durable_record_to_runtime_event_envelope)
        .collect::<Vec<_>>();

    forward_compatible_runtime_events_without_resume_durable_prefix(
        CompatibleRuntimeEventsForward {
            state,
            initial_run,
            sender,
            mapper,
            stats,
            ignored_waiting_callback_task_id,
            last_forwarded_sequence: last_forwarded_durable_sequence,
            resume_durable_sequence_before_terminal: None,
            events,
        },
    )
    .await
}

#[cfg(test)]
async fn send_compatible_sse_events(
    sender: &mpsc::Sender<Result<Event, Infallible>>,
    events: Vec<Result<Event, Infallible>>,
) -> bool {
    for sse in events {
        if sender.send(sse).await.is_err() {
            return false;
        }
    }
    true
}

#[path = "event_forwarding/terminal_fallback.rs"]
mod terminal_fallback;

pub(super) use terminal_fallback::append_compatible_resume_terminal_event;
pub(super) use terminal_fallback::is_answer_presentation_delta;
pub(super) use terminal_fallback::is_public_terminal_runtime_event;
use terminal_fallback::*;

#[cfg(test)]
#[path = "tests/forwarding/_tests/committed_event_index.rs"]
mod committed_event_index_tests;
