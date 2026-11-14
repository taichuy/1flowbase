use super::*;

pub(super) async fn compatible_round_replay(
    dependencies: &CompatibilityExecutionDependencies,
    initial_run: &NativeRunResult,
    round_id: uuid::Uuid,
) -> Result<(Option<i64>, Vec<RuntimeEventEnvelope>), NativeApiError> {
    let live_round_start = live_compatible_round_start(dependencies, initial_run, round_id).await;
    if live_round_start.is_some() {
        return Ok((live_round_start, Vec::new()));
    }
    match durable_compatible_round_replay(dependencies, initial_run, round_id).await? {
        DurableCompatibleRoundReplay::Ready(replay) => Ok((None, replay)),
        DurableCompatibleRoundReplay::PendingIdentity => Err(service_error(anyhow::anyhow!(
            "anchored callback round identity marker pending"
        ))),
    }
}

pub(super) enum DurableCompatibleRoundReplay {
    Ready(Vec<RuntimeEventEnvelope>),
    PendingIdentity,
}

pub(super) enum DurableCompatibleRoundIdentity {
    Legacy,
    Generation { id: String, after_sequence: i64 },
    Pending,
}

pub(super) async fn durable_compatible_round_replay(
    dependencies: &CompatibilityExecutionDependencies,
    initial_run: &NativeRunResult,
    round_id: uuid::Uuid,
) -> Result<DurableCompatibleRoundReplay, NativeApiError> {
    durable_callback_replay(
        &native::native_run_terminal_dependencies(&dependencies.native),
        initial_run,
        round_id,
    )
    .await
}

pub(super) async fn durable_callback_replay(
    dependencies: &NativeRunTerminalDependencies,
    initial_run: &NativeRunResult,
    round_id: uuid::Uuid,
) -> Result<DurableCompatibleRoundReplay, NativeApiError> {
    let boundary = dependencies
        .callback_waiting_boundary(initial_run.id, round_id)
        .await
        .map_err(service_error)?
        .ok_or_else(|| {
            service_error(
                control_plane::errors::ControlPlaneError::Conflict(
                    "runtime_event_delivery_scope_unresolved",
                )
                .into(),
            )
        })?;
    let window = dependencies
        .replay_window(initial_run.id)
        .await
        .map_err(service_error)?;
    let identity_through = window.map(|window| window.through_sequence);
    let through = boundary
        .next
        .map(|next| next.event_sequence)
        .map(|upper| identity_through.map_or(upper, |high| high.min(upper)))
        .or(identity_through);
    let original_boundary = boundary.original.event_sequence;
    let next_waiting_sequence = boundary.next.map(|next| next.event_sequence);
    // The latest window may belong to a later callback. Resolve this round's
    // anchor with bounded pages, retaining only its scalar generation identity.
    let (generation, replay_boundary) = match durable_compatible_round_generation(
        dependencies,
        initial_run.id,
        round_id,
        original_boundary,
        next_waiting_sequence,
        identity_through,
    )
    .await?
    {
        DurableCompatibleRoundIdentity::Legacy => (None, original_boundary),
        DurableCompatibleRoundIdentity::Generation { id, .. } => {
            // A diagnostic marker can commit after the original tools+upper.
            // Generation selects the producer; only the business anchor selects
            // the durable suffix. Local/marker sequence is never its lower bound.
            (Some(id), original_boundary)
        }
        DurableCompatibleRoundIdentity::Pending => {
            return Ok(DurableCompatibleRoundReplay::PendingIdentity);
        }
    };
    let mut cursor = replay_boundary;
    let mut records = Vec::new();
    loop {
        let page = dependencies
            .callback_round_page(initial_run.id, cursor, through, 64)
            .await
            .map_err(service_error)?;
        let Some(last) = page.last() else {
            break;
        };
        if last.sequence <= cursor {
            return Err(service_error(anyhow::anyhow!(
                "durable round cursor did not advance"
            )));
        }
        cursor = last.sequence;
        for record in page {
            if record.event_type == "runtime_stream_opened" {
                continue;
            }
            if record.payload["response_round_id"]
                .as_str()
                .is_some_and(|value| value != round_id.to_string())
                || generation.as_ref().is_some_and(|generation| {
                    record.payload["stream_generation_id"]
                        .as_str()
                        .is_some_and(|value| value != generation)
                })
            {
                continue;
            }
            if !respects_callback_waiting_boundary(&record, next_waiting_sequence) {
                continue;
            }
            let terminal = event_forwarding::is_public_terminal_runtime_event(&record.event_type);
            records.push(record);
            if terminal {
                break;
            }
        }
        if records.last().is_some_and(|record| {
            event_forwarding::is_public_terminal_runtime_event(&record.event_type)
        }) {
            break;
        }
        if through.is_some_and(|through| cursor >= through) {
            break;
        }
    }
    let mut replay = event_forwarding::durable_round_prefix(records);
    if !replay.is_empty() && replay[0].event_type != "flow_started" {
        replay.insert(
            0,
            RuntimeEventEnvelope::new(
                initial_run.id,
                0,
                debug_stream_events::flow_started(initial_run.id),
            ),
        );
    }
    Ok(DurableCompatibleRoundReplay::Ready(replay))
}

// A diagnostic waiting copy cannot terminate this selected callback response.
// Only its next original business stop grants waiting-terminal authority.
fn respects_callback_waiting_boundary(
    record: &domain::RuntimeEventRecord,
    next_waiting_sequence: Option<i64>,
) -> bool {
    !matches!(
        record.event_type.as_str(),
        "waiting_callback" | "waiting_human"
    ) || next_waiting_sequence.is_some_and(|upper| record.sequence == upper)
}

async fn durable_compatible_round_generation(
    dependencies: &NativeRunTerminalDependencies,
    run_id: uuid::Uuid,
    round_id: uuid::Uuid,
    boundary: i64,
    next_waiting_sequence: Option<i64>,
    through: Option<i64>,
) -> Result<DurableCompatibleRoundIdentity, NativeApiError> {
    let mut cursor = boundary;
    let mut anchor_generation = None;
    let mut saw_anchor = false;
    let mut current_anchor = None;
    let mut current_anchor_sequence = boundary;
    let mut output_generation = None;
    let mut ambiguous_output_generation = false;
    let mut marker_identity = None;
    let mut legacy_terminal = false;
    loop {
        let page = dependencies
            .callback_round_page(run_id, cursor, through, 64)
            .await
            .map_err(service_error)?;
        let Some(last) = page.last() else {
            break;
        };
        if last.sequence <= cursor {
            return Err(service_error(anyhow::anyhow!(
                "durable round anchor cursor did not advance"
            )));
        }
        cursor = last.sequence;
        for record in page {
            if record.event_type == "runtime_stream_opened" {
                saw_anchor = true;
                anchor_generation = record.payload["stream_generation_id"]
                    .as_str()
                    .map(str::to_owned);
                current_anchor = anchor_generation.clone();
                current_anchor_sequence = record.sequence;
                continue;
            }
            if record.event_type == "flow_started"
                && record.payload["response_round_id"] == json!(round_id)
            {
                let generation = record.payload["stream_generation_id"]
                    .as_str()
                    .map(str::to_owned)
                    .or_else(|| anchor_generation.take());
                if generation.is_none() && saw_anchor {
                    return Err(service_error(anyhow::anyhow!(
                        "anchored callback round generation missing"
                    )));
                }
                if saw_anchor && generation.as_deref() != current_anchor.as_deref() {
                    continue; // A late older producer cannot replace a newer anchored marker.
                }
                // A reclaimed execution establishes a later generation for
                // the same callback. Retain its latest explicit marker.
                marker_identity = Some(match generation {
                    Some(generation) => DurableCompatibleRoundIdentity::Generation {
                        id: generation,
                        after_sequence: current_anchor_sequence,
                    },
                    None => DurableCompatibleRoundIdentity::Legacy,
                });
                anchor_generation = None;
                continue;
            }
            // Old producers could omit flow_started while committing exact
            // host-owned output with both identities. Only those facts can
            // recover the association; an anchor or terminal alone cannot.
            if record.event_type == "provider_output_item_done"
                && record.trust_level == domain::RuntimeTrustLevel::HostFact
                && record.payload["response_round_id"] == json!(round_id)
            {
                if let Some(generation) = record.payload["stream_generation_id"]
                    .as_str()
                    .filter(|generation| current_anchor.as_deref() == Some(*generation))
                {
                    if output_generation
                        .as_ref()
                        .is_some_and(|(prior, _)| prior != generation)
                    {
                        ambiguous_output_generation = true;
                    }
                    output_generation = Some((generation.to_owned(), current_anchor_sequence));
                }
            }
            // Historical unanchored rounds have no identity marker. Keep their
            // existing recovery path, without assigning a later round's anchor.
            if !saw_anchor
                && record.payload["stream_generation_id"].is_null()
                && record.payload["response_round_id"].is_null()
                && event_forwarding::is_public_terminal_runtime_event(&record.event_type)
                && respects_callback_waiting_boundary(&record, next_waiting_sequence)
            {
                legacy_terminal = true;
            }
            // An untagged marker can borrow only an adjacent anchor, never an
            // unrelated earlier generation encountered during the scan.
            anchor_generation = None;
        }
        if through.is_some_and(|through| cursor >= through) {
            break;
        }
    }
    if let Some(identity) = marker_identity {
        if let (
            DurableCompatibleRoundIdentity::Generation {
                after_sequence: marker_boundary,
                ..
            },
            Some((id, output_boundary)),
        ) = (&identity, &output_generation)
        {
            if output_boundary > marker_boundary {
                // A newer explicit same-round output must not replay an older
                // execution merely because that producer wrote its marker.
                return Ok(if ambiguous_output_generation {
                    DurableCompatibleRoundIdentity::Pending
                } else {
                    DurableCompatibleRoundIdentity::Generation {
                        id: id.clone(),
                        after_sequence: *output_boundary,
                    }
                });
            }
        }
        return Ok(identity);
    }
    if legacy_terminal {
        return Ok(DurableCompatibleRoundIdentity::Legacy);
    }
    if !ambiguous_output_generation {
        if let Some((id, after_sequence)) = output_generation {
            return Ok(DurableCompatibleRoundIdentity::Generation { id, after_sequence });
        }
    }
    Ok(if saw_anchor {
        DurableCompatibleRoundIdentity::Pending
    } else {
        DurableCompatibleRoundIdentity::Legacy
    })
}

pub(super) async fn live_compatible_round_start(
    dependencies: &CompatibilityExecutionDependencies,
    initial_run: &NativeRunResult,
    round_id: uuid::Uuid,
) -> Option<i64> {
    let mut cursor = None;
    let mut round_start = None;
    loop {
        let events = dependencies
            .native
            .runtime_event_stream
            .replay(initial_run.id, cursor, 64)
            .await
            .ok()?;
        if events.is_empty() {
            return round_start;
        }
        for event in &events {
            if event.event_type == "flow_started"
                && event.payload["response_round_id"] == json!(round_id)
            {
                round_start = Some(event.sequence.saturating_sub(1));
            } else if event_forwarding::is_public_terminal_runtime_event(&event.event_type) {
                round_start = None;
            }
        }
        cursor = events.last().map(|event| event.sequence);
        if events.len() < 64 {
            return round_start;
        }
    }
}
