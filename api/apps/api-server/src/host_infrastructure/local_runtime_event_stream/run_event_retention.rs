use super::*;

impl RetainedRuntimeEvents {
    pub(super) fn push(&mut self, event: RetainedRuntimeEvent) {
        self.sequence_order_may_be_unsorted |= self
            .events
            .back()
            .is_some_and(|previous| previous.sequence > event.sequence);
        self.ephemeral_count += usize::from(!event.is_required());
        self.bytes = self.bytes.saturating_add(event.wire.len());
        if let Some(reason) = event.terminal_reason {
            self.terminal_reason.get_or_insert(reason);
            self.terminal_sequence.get_or_insert(event.sequence);
        }
        self.events.push_back(Arc::new(event));
    }

    fn after_sequence<'a>(
        &self,
        events: &'a VecDeque<Arc<RetainedRuntimeEvent>>,
        sequence: i64,
    ) -> impl Iterator<Item = &'a Arc<RetainedRuntimeEvent>> {
        // The producer normally advances monotonically. Preserve the original
        // filter even if its signed sequence counter ever wraps in release mode.
        let start = if self.sequence_order_may_be_unsorted {
            0
        } else {
            events.partition_point(|event| event.sequence <= sequence)
        };
        events
            .range(start..)
            .filter(move |event| event.sequence > sequence)
    }

    fn first_ephemeral_from(&self, start: usize) -> Option<usize> {
        if self.ephemeral_count == 0 {
            return None;
        }
        self.events
            .range(start..)
            .position(|event| !event.is_required())
            .map(|offset| start + offset)
    }
}

impl LocalRunEventStream {
    pub(super) fn close(&self, reason: RuntimeEventCloseReason) {
        let _ring = self.ring.lock().expect("runtime event ring lock poisoned");
        if !self.is_closed() {
            let final_sequence = self.next_sequence.load(Ordering::SeqCst) - 1;
            *self
                .closed_at
                .lock()
                .expect("runtime event closed_at lock poisoned") = Some(OffsetDateTime::now_utc());
            self.closed_sender.send_replace(Some(RuntimeEventClosure {
                reason,
                final_sequence,
            }));
            self.release_live_broadcast();
        }
    }

    pub(super) fn append_terminal_if_missing_and_close(
        &self,
        run_id: Uuid,
        event: RuntimeEventPayload,
    ) -> Result<AppendTerminalIfMissingAndCloseOutcome> {
        let incoming_reason = RuntimeEventCloseReason::from_terminal_event_type(&event.event_type)
            .ok_or_else(|| {
                anyhow!("runtime event stream terminal append requires a terminal event")
            })?;
        // A terminal must reach its subscribers regardless of diagnostic budgets.
        let run = self;

        let outcome = {
            // `ring` is the stream's serialization point for append and close. Holding it for
            // the terminal check, optional append, and closure prevents concurrent EOF recovery
            // retries from observing the same missing terminal.
            let mut ring = run.ring.lock().expect("runtime event ring lock poisoned");
            let existing_terminal_reason = ring.terminal_reason;

            if run.is_closed() {
                if existing_terminal_reason.is_some() {
                    return Ok(AppendTerminalIfMissingAndCloseOutcome::ExistingTerminal);
                }
                return Err(anyhow!(
                    "runtime event stream is closed without a terminal event"
                ));
            }

            if ring.retired && existing_terminal_reason.is_none() {
                return Err(anyhow!(
                    "runtime event stream is closed without a terminal event"
                ));
            }

            let (outcome, close_reason) = if let Some(existing_reason) = existing_terminal_reason {
                (
                    AppendTerminalIfMissingAndCloseOutcome::ExistingTerminal,
                    existing_reason,
                )
            } else {
                let sequence = run.next_sequence.load(Ordering::SeqCst);
                let envelope = RuntimeEventEnvelope::new(run_id, sequence, event);
                let retained = RetainedRuntimeEvent::encode(&envelope)?;
                let retained_bytes = retained.wire.len();
                run.make_room_for(&mut ring, retained_bytes, retained.is_required())?;
                run.next_sequence.store(sequence + 1, Ordering::SeqCst);
                *run.last_event_at
                    .lock()
                    .expect("runtime event last event lock poisoned") = envelope.occurred_at;
                ring.push(retained);
                (
                    AppendTerminalIfMissingAndCloseOutcome::Appended,
                    incoming_reason,
                )
            };

            let final_sequence = run.next_sequence.load(Ordering::SeqCst) - 1;
            *run.closed_at
                .lock()
                .expect("runtime event closed_at lock poisoned") = Some(OffsetDateTime::now_utc());
            run.closed_sender.send_replace(Some(RuntimeEventClosure {
                reason: close_reason,
                final_sequence,
            }));
            // Subscribers backfill the retained terminal when they observe closure.
            run.release_live_broadcast();
            outcome
        };
        Ok(outcome)
    }

    pub(super) fn new(
        policy: RuntimeEventStreamPolicy,
        broadcast_capacity: usize,
        recoverable_retention: TimeDuration,
    ) -> Self {
        let (broadcaster, _) = broadcast::channel(broadcast_capacity);
        let (closed_sender, _) = watch::channel(None);
        let now = OffsetDateTime::now_utc();
        Self {
            generation_id: Uuid::now_v7(),
            recoverable_retention,
            next_sequence: AtomicI64::new(1),
            ring: Mutex::new(RetainedRuntimeEvents::default()),
            broadcaster: Mutex::new(Some(broadcaster)),
            closed_sender,
            policy,
            closed_at: Mutex::new(None),
            last_event_at: Mutex::new(now),
            subscribers: Mutex::new(Vec::new()),
        }
    }

    pub(super) fn retire_recoverable_payloads(&self) -> bool {
        let mut ring = self.ring.lock().expect("runtime event ring lock poisoned");
        if ring.confirmed_at.is_none() || ring.persistence_failed || !self.is_closed() {
            return false;
        }
        self.retire_payloads_locked(&mut ring);
        true
    }

    pub(super) fn retire_payloads(&self) {
        let mut ring = self.ring.lock().expect("runtime event ring lock poisoned");
        self.retire_payloads_locked(&mut ring);
    }

    pub(super) fn retire_payloads_locked(&self, ring: &mut RetainedRuntimeEvents) {
        if ring.retired {
            return;
        }
        let subscribers = self
            .subscribers
            .lock()
            .expect("runtime event subscribers lock poisoned")
            .iter()
            .filter_map(Weak::upgrade)
            .collect::<Vec<_>>();
        let mut events = std::mem::take(&mut ring.events);
        ring.bytes = 0;
        ring.ephemeral_count = 0;
        ring.retired = true;
        if let Some(minimum_sequence) = subscribers
            .iter()
            .map(|subscriber| subscriber.sequence.load(Ordering::Acquire))
            .min()
        {
            events.retain(|event| event.sequence > minimum_sequence);
            let events = Arc::new(events);
            for subscriber in subscribers {
                *subscriber
                    .retired_events
                    .lock()
                    .expect("runtime event backfill lock poisoned") = Some(Arc::clone(&events));
            }
        }
        // Wake orphan forwarding tasks as well as closed generations. A missing
        // terminal stays missing; dropping the broadcaster is not a terminal fact.
        self.release_live_broadcast();
    }

    pub(super) fn subscribe_backfill(
        &self,
        from_sequence: Option<i64>,
    ) -> Result<(Arc<LiveBackfill>, Vec<RuntimeEventEnvelope>)> {
        let requested_sequence = from_sequence.unwrap_or(0);
        let (subscriber, retained) = {
            let ring = self.ring.lock().expect("runtime event ring lock poisoned");
            if ring.retired {
                return Err(anyhow!("runtime event replay expired"));
            }
            Self::validate_replay_cursor(
                &ring,
                requested_sequence,
                self.next_sequence.load(Ordering::SeqCst),
            )?;
            let subscriber = Arc::new(LiveBackfill {
                sequence: AtomicI64::new(requested_sequence),
                retired_events: Mutex::new(None),
            });
            let mut subscribers = self
                .subscribers
                .lock()
                .expect("runtime event subscribers lock poisoned");
            subscribers.retain(|subscriber| subscriber.strong_count() > 0);
            subscribers.push(Arc::downgrade(&subscriber));
            let retained = ring
                .after_sequence(&ring.events, requested_sequence)
                .cloned()
                .collect::<Vec<_>>();
            (subscriber, retained)
        };
        let replay = retained
            .iter()
            .map(|event| event.decode())
            .collect::<Result<Vec<_>>>()?;
        Ok((subscriber, replay))
    }

    pub(super) fn validate_replay_cursor(
        ring: &RetainedRuntimeEvents,
        requested_sequence: i64,
        next_sequence: i64,
    ) -> Result<()> {
        if let Some(front) = ring.events.front() {
            if requested_sequence < front.sequence - 1 {
                return Err(anyhow!("runtime event replay expired"));
            }
        } else if requested_sequence < next_sequence - 1 {
            return Err(anyhow!("runtime event replay expired"));
        }
        Ok(())
    }

    pub(super) fn subscribe_live(&self) -> broadcast::Receiver<RuntimeEventEnvelope> {
        self.broadcaster
            .lock()
            .expect("runtime event broadcaster lock poisoned")
            .as_ref()
            .map(broadcast::Sender::subscribe)
            .unwrap_or_else(|| broadcast::channel(1).1)
    }

    pub(super) fn broadcast(&self, event: RuntimeEventEnvelope) {
        if let Some(sender) = self
            .broadcaster
            .lock()
            .expect("runtime event broadcaster lock poisoned")
            .as_ref()
        {
            let _ = sender.send(event);
        }
    }

    pub(super) fn release_live_broadcast(&self) {
        self.broadcaster
            .lock()
            .expect("runtime event broadcaster lock poisoned")
            .take();
    }

    pub(super) fn retention_duration(&self) -> TimeDuration {
        match self.closed_sender.borrow().map(|closure| closure.reason) {
            Some(RuntimeEventCloseReason::WaitingHuman)
            | Some(RuntimeEventCloseReason::WaitingCallback) => WAITING_RUN_RETENTION,
            Some(_) => self.policy.ttl,
            None => ORPHAN_RUN_RETENTION,
        }
    }

    pub(super) fn retention_deadline(&self) -> OffsetDateTime {
        let ring = self.ring.lock().expect("runtime event ring lock poisoned");
        self.retention_deadline_locked(&ring)
    }

    pub(super) fn retention_deadline_locked(&self, ring: &RetainedRuntimeEvents) -> OffsetDateTime {
        let start = self
            .closed_at
            .lock()
            .expect("runtime event closed_at lock poisoned")
            .unwrap_or_else(|| {
                *self
                    .last_event_at
                    .lock()
                    .expect("runtime event last event lock poisoned")
            });
        let normal_deadline = start
            .checked_add(self.retention_duration())
            .unwrap_or(time::PrimitiveDateTime::MAX.assume_utc());
        match ring
            .confirmed_at
            .filter(|_| !ring.persistence_failed && self.is_closed())
        {
            Some(confirmed_at) => normal_deadline.min(
                confirmed_at
                    .checked_add(self.recoverable_retention)
                    .unwrap_or(time::PrimitiveDateTime::MAX.assume_utc()),
            ),
            None => normal_deadline,
        }
    }

    pub(super) fn retire_if_expired(&self, now: OffsetDateTime) -> bool {
        // Proof/failure changes and the retirement claim share the ring lock.
        let mut ring = self.ring.lock().expect("runtime event ring lock poisoned");
        if now < self.retention_deadline_locked(&ring) {
            return false;
        }
        self.retire_payloads_locked(&mut ring);
        true
    }

    pub(super) fn is_closed(&self) -> bool {
        self.closed_sender.borrow().is_some()
    }

    pub(super) fn replay_from_ring(
        &self,
        from_sequence: Option<i64>,
        limit: usize,
    ) -> Result<Vec<RuntimeEventEnvelope>> {
        let requested_sequence = from_sequence.unwrap_or(0);
        let retained = {
            let ring = self.ring.lock().expect("runtime event ring lock poisoned");
            Self::validate_replay_cursor(
                &ring,
                requested_sequence,
                self.next_sequence.load(Ordering::SeqCst),
            )?;
            ring.after_sequence(&ring.events, requested_sequence)
                .take(limit)
                .cloned()
                .collect::<Vec<_>>()
        };
        retained.iter().map(|event| event.decode()).collect()
    }

    pub(super) fn events_after_sequence(
        &self,
        sequence: i64,
        limit: usize,
        subscriber: &LiveBackfill,
    ) -> Result<Vec<RuntimeEventEnvelope>> {
        let retained = {
            // Retirement holds this same lock while handing off the ring, so a page
            // sees either the hot ring or the consumer-owned retired backlog.
            let ring = self.ring.lock().expect("runtime event ring lock poisoned");
            let retired_events = subscriber
                .retired_events
                .lock()
                .expect("runtime event backfill lock poisoned");
            let events = retired_events.as_deref().unwrap_or(&ring.events);
            ring.after_sequence(events, sequence)
                .take(limit)
                .cloned()
                .collect::<Vec<_>>()
        };
        retained.iter().map(|event| event.decode()).collect()
    }

    pub(super) fn remove_retained_event(
        ring: &mut RetainedRuntimeEvents,
        index: usize,
    ) -> Result<()> {
        if let Some(event) = ring.events.remove(index) {
            ring.bytes = ring.bytes.saturating_sub(event.wire.len());
            ring.ephemeral_count -= usize::from(!event.is_required());
            if event.terminal_reason.is_some() {
                ring.terminal_reason = ring.events.iter().find_map(|event| event.terminal_reason);
                ring.terminal_sequence = ring
                    .events
                    .iter()
                    .find(|event| event.terminal_reason.is_some())
                    .map(|event| event.sequence);
            }
        }
        Ok(())
    }

    pub(super) fn make_room_for(
        &self,
        ring: &mut RetainedRuntimeEvents,
        incoming_bytes: usize,
        required_delivery: bool,
    ) -> Result<()> {
        match self.policy.overflow_behavior {
            RuntimeEventOverflowBehavior::DropOldEphemeralKeepRequired => {
                let mut search_from = 0;
                while ring.events.len() >= self.policy.max_events
                    || ring.bytes.saturating_add(incoming_bytes) > self.policy.max_bytes
                {
                    let Some(index) = ring.first_ephemeral_from(search_from) else {
                        // These budgets trim diagnostics. Required delivery is governed by
                        // live-lane backpressure and generation reclamation, not body size.
                        if required_delivery {
                            break;
                        }
                        let capacity = if ring.events.len() >= self.policy.max_events {
                            "event"
                        } else {
                            "byte"
                        };
                        return Err(anyhow!("runtime event stream {capacity} capacity exceeded"));
                    };
                    Self::remove_retained_event(ring, index)?;
                    search_from = index;
                }
            }
        }
        Ok(())
    }

    pub(super) fn trim_to_policy(&self, ring: &mut RetainedRuntimeEvents) -> Result<()> {
        let mut search_from = 0;
        while ring.events.len() > self.policy.max_events || ring.bytes > self.policy.max_bytes {
            let Some(index) = ring.first_ephemeral_from(search_from) else {
                break;
            };
            Self::remove_retained_event(ring, index)?;
            search_from = index;
        }
        Ok(())
    }
}
