use super::*;

impl<C: TransportClock + 'static> TransportSessionCoordinator<C> {
    pub(crate) async fn prepare(
        &self,
        target_id: &str,
        input: &mut ProviderInvocationInput,
        context: &ProviderRuntimeExecutionContext,
    ) -> anyhow::Result<Option<PreparedTransportInvocation>> {
        let connection_scope_id = take_transport_connection_scope(input);
        // Cursor-free HTTP fallback is meaningful even without a logical session.
        // A validated WebSocket delivery below overrides this default carrier.
        let predecessor_transport = if input.protocol == "openai_responses"
            && input.native_transport.as_ref().is_some_and(|native| {
                native
                    .wire_body
                    .get("previous_response_id")
                    .is_some_and(|v| !v.is_null())
            }) {
            input.client_transport
        } else {
            None
        };
        if input.protocol == "openai_responses" {
            input.client_transport = input.native_transport.as_ref().and_then(|native| {
                let has_cursor = native
                    .wire_body
                    .get("previous_response_id")
                    .is_some_and(|value| !value.is_null());
                if has_cursor {
                    predecessor_transport
                } else {
                    Some(ProviderClientTransport::Http)
                }
            });
        }
        let Some(protocol_session_id) = protocol_session_id(input) else {
            return Ok(None);
        };
        let scope_requested = connection_scope_id.is_some();

        let owner_id = TransportOwnerId::new(protocol_session_id.to_string())?;
        let session_id = TransportSessionId::new(hash_identity(&[
            protocol_session_id,
            &context.workspace_id.to_string(),
            context
                .actor_id
                .map(|id| id.to_string())
                .as_deref()
                .unwrap_or("anonymous"),
            &input.provider_instance_id,
            &input.protocol,
            &input.model,
        ]))?;
        if self.shutdown.load(Ordering::Acquire) {
            return Err(admission_error(
                "transport_session_shutdown",
                admission_details(&session_id, None, None, None, scope_requested),
            ));
        }
        let carrier = self
            .admission_scope(connection_scope_id.as_deref(), &session_id)?
            .map(|scope| scope.carrier);
        // Delivery ownership and wire transport are independent host facts. An
        // HTTP scope must not turn a cursor-free fallback back into WebSocket;
        // cursor-based HTTP continuations retain the existing provider policy.
        // A new delivery cannot change an owned incremental cursor's upstream
        // carrier. Explicit force_* node policy still wins in the selector below.
        if predecessor_transport.is_none() && carrier == Some(ProviderClientTransport::Websocket) {
            input.client_transport = Some(ProviderClientTransport::Websocket);
        }
        let transport = selected_responses_transport(input)?;
        let recovery_directive = input
            .recovery_directive()
            .map_err(|_| transport_error("provider_recovery_directive_invalid"))?;
        let target = TransportRuntimeTargetId::new(target_id.to_string())?;
        let provider_id = TransportProviderId::new(input.provider_instance_id.clone())?;
        let invocation_deadline = u64::try_from(context.deadline_unix_ms)
            .ok()
            .map(TransportInstant::from_millis);

        let mut handoff_deadline = None;
        let mut handoff_started_at = None;
        loop {
            // Register before observing state; completion between inspection and await
            // must wake this waiter. Never retain registry/dispatcher locks while waiting.
            let changed = self.invocation_changed.notified();
            tokio::pin!(changed);
            changed.as_mut().enable();
            let _dispatcher = if let Some(deadline) = handoff_deadline {
                let _ = self.admission_scope(connection_scope_id.as_deref(), &session_id)?;
                if tokio::time::Instant::now() >= deadline {
                    return Err(transport_error("transport_invocation_deadline_exceeded"));
                }
                tokio::select! {
                    result = tokio::time::timeout_at(deadline, self.dispatcher.lock()) => {
                        result.map_err(|_| transport_error("transport_invocation_deadline_exceeded"))?
                    }
                    _ = changed.as_mut() => continue,
                }
            } else {
                self.dispatcher.lock().await
            };
            let connection_scope =
                self.admission_scope(connection_scope_id.as_deref(), &session_id)?;
            if handoff_deadline.is_none() {
                // Initial admission retains its existing control owner. A resumed
                // handoff must not drain or await provider Close work: cancelling
                // that dispatched future could lose its command/stdio retirement.
                // Existing finish/maintenance/shutdown owners dispatch those events.
                self.registry.lock().await.maintain();
                self.dispatch_pending_events_locked().await;
            }

            let mut registry = if let Some(deadline) = handoff_deadline {
                tokio::select! {
                    result = tokio::time::timeout_at(deadline, self.registry.lock()) => {
                        result.map_err(|_| transport_error("transport_invocation_deadline_exceeded"))?
                    }
                    _ = changed.as_mut() => continue,
                }
            } else {
                self.registry.lock().await
            };
            if handoff_deadline.is_some() {
                // Leave any newly generated lifecycle events queued for their
                // existing owner; admission still checks the maintained state.
                registry.maintain();
            }
            let now = registry.safe_snapshot().observed_at;
            if invocation_deadline.is_some_and(|deadline| deadline <= now)
                || handoff_deadline.is_some_and(|deadline| tokio::time::Instant::now() >= deadline)
            {
                return Err(transport_error("transport_invocation_deadline_exceeded"));
            }
            let fence = if let Some(fence) = registry.fence(&session_id) {
                let mut state = registry.state(&fence)?;
                if matches!(
                    state,
                    TransportSessionState::Active | TransportSessionState::Orphaned
                ) {
                    // Unbinding the delivery is a fact, not a verdict. Only the
                    // execution state decides whether a successor may start here, and
                    // a second execution is never opened while its predecessor runs.
                    // A separate delivery with the same owner identity and target
                    // waits for actual completion even while the old socket is open.
                    // Compaction can open that delivery before the previous socket
                    // finishes closing. Its timing must not turn serialization into
                    // a retryable busy error. The bound delivery still rejects a
                    // second concurrent invocation of its own.
                    let inflight = registry.invocation_inflight(&fence)?;
                    if inflight {
                        if registry.runtime_target_id(&fence)? == &target
                            && connection_scope.as_ref().is_some_and(|scope| {
                                !scope
                                    .state
                                    .lock()
                                    .expect("transport connection bindings")
                                    .leases
                                    .contains_key(session_id.as_str())
                            })
                        {
                            let predecessor_deadline = registry
                                .invocation_deadline(&fence)?
                                .expect("the registry lock preserves the inflight invocation");
                            let deadline = *handoff_deadline.get_or_insert_with(|| {
                                let started_at = tokio::time::Instant::now();
                                handoff_started_at = Some(started_at);
                                started_at
                                    + handoff_wait_budget(
                                        now,
                                        invocation_deadline,
                                        predecessor_deadline,
                                    )
                            });
                            drop(registry);
                            drop(_dispatcher);
                            if tokio::time::timeout_at(deadline, changed).await.is_err() {
                                tracing::warn!(
                                    session_id = %session_id.as_str(),
                                    handoff_wait_ms = handoff_started_at
                                        .map(|started_at: tokio::time::Instant| started_at.elapsed().as_millis())
                                        .unwrap_or(0),
                                    "provider transport handoff timed out behind an inflight invocation"
                                );
                                return Err(transport_error(
                                    "transport_invocation_deadline_exceeded",
                                ));
                            }
                            continue;
                        }
                        if state == TransportSessionState::Orphaned {
                            return Err(admission_error(
                                "transport_session_inflight_unbound",
                                admission_details(
                                    &session_id,
                                    Some(&fence),
                                    Some(state),
                                    Some(true),
                                    true,
                                ),
                            ));
                        }
                    }
                }
                let bound_target = registry.runtime_target_id(&fence)?.clone();
                let target_changed = bound_target != target;
                if target_changed && registry.invocation_inflight(&fence)? {
                    return Err(admission_error(
                        "transport_session_busy",
                        admission_details(
                            &session_id,
                            Some(&fence),
                            Some(state),
                            Some(true),
                            scope_requested,
                        ),
                    ));
                }
                // Admission and the read-only exit probe share the dispatcher and
                // registry owner. An in-flight call keeps its completion owner;
                // only the following turn can fence a confirmed dead worker.
                if !registry.invocation_inflight(&fence)? {
                    if let Some(evidence) = self
                        .runtime
                        .transport_worker_exit_evidence(
                            bound_target.as_str(),
                            fence.session_id.as_str(),
                            fence.generation.get(),
                        )
                        .await
                        .map_err(anyhow::Error::from)?
                    {
                        registry
                            .record_worker_exit_evidence(&fence, &evidence)
                            .map_err(|_| {
                                transport_error("provider_transport_worker_exit_evidence_invalid")
                            })?;
                        state = registry.state(&fence)?;
                    }
                }
                let now = registry.safe_snapshot().observed_at;
                if invocation_deadline.is_some_and(|deadline| deadline <= now)
                    || handoff_deadline
                        .is_some_and(|deadline| tokio::time::Instant::now() >= deadline)
                {
                    return Err(transport_error("transport_invocation_deadline_exceeded"));
                }
                if target_changed
                    && matches!(
                        state,
                        TransportSessionState::Active
                            | TransportSessionState::WaitingTool
                            | TransportSessionState::IdleAffinity
                            | TransportSessionState::Orphaned
                    )
                {
                    validate_fault_successor(recovery_directive.as_ref(), now)?;
                    // Target is a physical-generation property. Retire the old
                    // target through its existing control owner before rebinding;
                    // never open a parallel session by adding a version to the key.
                    registry.transition(&fence, TransportSessionState::Draining)?;
                    drop(registry);
                    drop(_dispatcher);
                    continue;
                }
                if matches!(
                    state,
                    TransportSessionState::Faulted | TransportSessionState::IdleReleased
                ) || (state == TransportSessionState::Draining
                    && registry.physical_generation_settled(&fence)?)
                {
                    validate_fault_successor(recovery_directive.as_ref(), now)?;
                    if self.close_task_exhausted(&fence) {
                        return Err(self.recovery_admission_error(
                            &fence,
                            "provider_physical_connection_close_exhausted",
                        ));
                    }
                    // Only the next invocation gets a fresh physical generation.
                    // Rotation keeps the original logical deadline and sequence.
                    let next = registry
                        .rotate_generation_to(&fence, target.clone())
                        .map_err(|error| {
                            if matches!(error, RegistryError::InvalidTransition { .. }) {
                                self.recovery_admission_error(
                                    &fence,
                                    "provider_physical_connection_close_pending",
                                )
                            } else {
                                map_registry_use_error(error)
                            }
                        })?;
                    registry.activate(&next)?;
                    next
                } else {
                    if matches!(
                        state,
                        TransportSessionState::Draining | TransportSessionState::Closing
                    ) || target_changed
                    {
                        if self.close_task_exhausted(&fence) {
                            return Err(self.recovery_admission_error(
                                &fence,
                                "provider_physical_connection_close_exhausted",
                            ));
                        }
                        return Err(self.recovery_admission_error(
                            &fence,
                            if state == TransportSessionState::Draining {
                                "provider_connection_draining"
                            } else {
                                "provider_connection_closing"
                            },
                        ));
                    }
                    fence
                }
            } else {
                if let Some(receipt) = registry.tombstone(&session_id) {
                    // A disconnected delivery and an elapsed logical lifetime can
                    // both be followed by a new bounded generation, but only after
                    // the old invocation and physical transport are fenced out.
                    if !matches!(
                        receipt.kind,
                        TerminationKind::DeadlineExceeded(DeadlineKind::LogicalAbsolute)
                            | TerminationKind::OwnerOrphaned
                    ) || receipt
                        .unsettled_invocation
                        .as_ref()
                        .is_some_and(|lease| now < lease.deadline())
                    {
                        return Err(self.recovery_admission_error(
                            &receipt.fence,
                            termination_code(receipt.kind),
                        ));
                    }
                    if receipt.owner_id != owner_id || receipt.provider_id != provider_id {
                        return Err(admission_error(
                            "transport_session_evicted",
                            admission_details(
                                &session_id,
                                Some(&receipt.fence),
                                Some(receipt.previous_state),
                                Some(false),
                                scope_requested,
                            ),
                        ));
                    }
                    validate_fault_successor(recovery_directive.as_ref(), now)?;
                    if self.close_task_exhausted(&receipt.fence) {
                        return Err(self.recovery_admission_error(
                            &receipt.fence,
                            "provider_physical_connection_close_exhausted",
                        ));
                    }
                    if !(receipt.never_dispatched_settled
                        || receipt
                            .closure_evidence
                            .as_ref()
                            .is_some_and(|evidence| evidence.local_released))
                    {
                        return Err(self.recovery_admission_error(
                            &receipt.fence,
                            "provider_physical_connection_close_pending",
                        ));
                    }
                    // The expired, completed generation remains as a diagnostic
                    // tombstone. The new admission allocates a distinct bounded
                    // generation, so delayed results cannot mutate the successor.
                }
                let fence = registry
                    .admit(AdmissionRequest {
                        session_id: session_id.clone(),
                        owner_id,
                        provider_id,
                        runtime_target_id: target,
                        provider_hard_deadline: None,
                    })
                    .map_err(map_registry_admission_error)?;
                registry.activate(&fence)?;
                fence
            };
            let lease = registry
                .begin_invocation(
                    &fence,
                    InvocationRequest {
                        deadline: invocation_deadline,
                    },
                )
                .map_err(map_registry_use_error)?;
            let cleanup_registry = self.registry.clone();
            let cleanup_changed = self.invocation_changed.clone();
            let cleanup_lease = lease.clone();
            let cleanup_scope = connection_scope.as_ref().map(Arc::downgrade);
            let prepared = PreparedTransportInvocation {
                lease: lease.clone(),
                transport,
                recovery_directive: recovery_directive.clone(),
                on_abandon: Some(Box::new(move || {
                    // Cancellation cannot lose the reservation owner. The runtime owns
                    // retirement once dispatched; never-dispatched settlement is separate.
                    if let Some(scope) = cleanup_scope.and_then(|scope| scope.upgrade()) {
                        let mut state = scope.state.lock().expect("transport connection bindings");
                        let key = cleanup_lease.fence.session_id.as_str();
                        if state
                            .leases
                            .get(key)
                            .is_some_and(|bound| bound == &cleanup_lease)
                        {
                            state.leases.remove(key);
                        }
                    }
                    tokio::spawn(async move {
                        let mut registry = cleanup_registry.lock().await;
                        let _ = registry
                            .finish_invocation(&cleanup_lease, InvocationCompletion::Faulted);
                        let _ = registry.settle_never_dispatched(&cleanup_lease.fence);
                        registry.maintain();
                        drop(registry);
                        cleanup_changed.notify_waiters();
                    });
                })),
            };
            let physical_deadline_unix_ms =
                i64::try_from(registry.physical_hard_deadline(&fence)?.as_millis())
                    .unwrap_or(i64::MAX);
            input
                .set_transport_session_directive(ProviderTransportSessionDirective {
                    worker_incarnation: None,
                    logical_session_id: session_id.as_str().to_string(),
                    generation: fence.generation.get(),
                    task_id: format!("invocation-{}-{}", fence.generation.get(), lease.sequence()),
                    state: ProviderLogicalSessionState::Active,
                    physical_deadline_unix_ms,
                })
                .map_err(|_| transport_error("transport_session_directive_invalid"))?;
            drop(registry);
            {
                let mut scopes = self.connection_scopes.lock().expect("transport scopes");
                scopes.retain(|_, scope| {
                    let Some(scope) = scope.upgrade() else {
                        return false;
                    };
                    scope
                        .state
                        .lock()
                        .expect("transport connection bindings")
                        .leases
                        .remove(lease.fence.session_id.as_str());
                    true
                });
            }
            if let Some(scope) = connection_scope {
                scope
                    .state
                    .lock()
                    .expect("transport connection bindings")
                    .leases
                    .insert(lease.fence.session_id.as_str().into(), lease.clone());
            }
            // Handoff has committed its lease: return it without an await that
            // could strand it behind an unrelated Close. Control ownership is
            // the same at successful exit as it is at admission entry.
            if handoff_deadline.is_none() {
                self.dispatch_pending_events_locked().await;
            }
            if let Some(started_at) = handoff_started_at {
                tracing::info!(
                    session_id = %session_id.as_str(),
                    generation = fence.generation.get(),
                    handoff_wait_ms = started_at.elapsed().as_millis(),
                    "provider transport handoff completed"
                );
            }
            return Ok(Some(prepared));
        }
    }
}
