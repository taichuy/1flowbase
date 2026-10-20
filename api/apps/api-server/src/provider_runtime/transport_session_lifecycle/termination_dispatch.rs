use super::*;

impl<C: TransportClock + 'static> TransportSessionCoordinator<C> {
    pub(crate) async fn shutdown(&self, timeout: Duration) {
        self.shutdown.store(true, Ordering::Release);
        self.invocation_changed.notify_waiters();
        self.registry
            .lock()
            .await
            .terminate_all(TerminationKind::Shutdown);
        self.dispatch_pending_events().await;
        self.shutdown_notify.notify_waiters();
        let handle = self
            .scheduler
            .lock()
            .expect("transport scheduler lock")
            .take();
        if let Some(handle) = handle {
            let _ = tokio::time::timeout(timeout, handle).await;
        }
    }

    pub(super) async fn fault_invocation(
        &self,
        lease: &InvocationLease,
        primary: Option<PrimaryTransportFailure>,
    ) {
        let result = {
            let mut registry = self.registry.lock().await;
            let result = registry.finish_invocation(lease, InvocationCompletion::Faulted);
            registry.maintain();
            result
        };
        if let Err(error) = result {
            tracing::warn!(%error, generation = lease.fence.generation.get(),
                "failed invocation did not change a stale or closed transport session");
        }
        let _dispatcher = self.dispatcher.lock().await;
        self.dispatch_pending_events_locked_with_primary(
            primary.map(|failure| (lease.fence.clone(), failure)),
        )
        .await;
        self.invocation_changed.notify_waiters();
    }

    pub(super) async fn maintain_and_dispatch(&self) {
        self.registry.lock().await.maintain();
        self.dispatch_pending_events().await;
    }

    pub(super) async fn dispatch_pending_events(&self) {
        let _dispatcher = self.dispatcher.lock().await;
        self.dispatch_pending_events_locked().await;
    }

    pub(super) async fn dispatch_pending_events_locked(&self) {
        self.dispatch_pending_events_locked_with_primary(None).await;
    }

    pub(super) async fn dispatch_pending_events_locked_with_primary(
        &self,
        primary: Option<(TransportFence, PrimaryTransportFailure)>,
    ) {
        let (new_commands, snapshot) = {
            let mut registry = self.registry.lock().await;
            let commands = registry
                .drain_events()
                .into_iter()
                .filter_map(|event| lifecycle_command(&registry, event))
                .collect::<Vec<_>>();
            (commands, registry.safe_snapshot())
        };
        if !new_commands.is_empty() {
            self.invocation_changed.notify_waiters();
        }
        let commands = {
            let mut pending = self
                .pending_commands
                .lock()
                .expect("transport pending command lock");
            pending.retain(|task| {
                snapshot
                    .sessions
                    .iter()
                    .any(|session| session.fence == task.fence)
                    || snapshot
                        .tombstones
                        .iter()
                        .any(|receipt| receipt.fence == task.fence)
            });
            for command in new_commands {
                if let Some(existing) = pending.iter_mut().find(|task| task.fence == command.fence)
                {
                    // Drain -> Close / logical termination upgrades the same physical task.
                    // A started control budget, attempts, and exhausted state are retained.
                    if command.command.action == ProviderTransportSessionAction::Close {
                        existing.command.action = ProviderTransportSessionAction::Close;
                    }
                    if command.terminal_close {
                        existing.terminal_close = true;
                        existing.termination = command.termination;
                    }
                } else {
                    pending.push_back(command);
                }
            }
            if let Some((fence, primary)) = primary {
                if let Some(task) = pending.iter_mut().find(|task| task.fence == fence) {
                    task.primary_failure.get_or_insert(primary);
                }
            }
            pending.drain(..).collect::<Vec<_>>()
        };
        let mut retained = VecDeque::new();
        for mut task in commands {
            if let Some(notice) = task.termination.take() {
                self.publish_termination(notice);
            }
            let never_dispatched = self
                .registry
                .lock()
                .await
                .settle_never_dispatched(&task.fence)
                .unwrap_or(false);
            if never_dispatched {
                task.state = CloseTaskState::Released;
                task.last_blocker = None;
                self.invocation_changed.notify_waiters();
            }
            let snapshot = self.registry.lock().await.safe_snapshot();
            if task.command.action == ProviderTransportSessionAction::Close {
                // A queued Close may outlive confirmation that its exact physical
                // generation was released. Consume that proof before retry/exhaustion
                // checks; terminal notification above remains independently owned.
                let evidence = snapshot
                    .sessions
                    .iter()
                    .find(|session| session.fence == task.fence)
                    .and_then(|session| session.closure_evidence.as_ref())
                    .or_else(|| {
                        snapshot
                            .tombstones
                            .iter()
                            .find(|receipt| receipt.fence == task.fence)
                            .and_then(|receipt| receipt.closure_evidence.as_ref())
                    });
                if let Some(evidence) = evidence.filter(|evidence| evidence.local_released) {
                    task.command.worker_incarnation = Some(evidence.identity.worker_incarnation);
                    task.state = CloseTaskState::Released;
                    task.last_blocker = None;
                }
            }
            if task.state != CloseTaskState::Pending {
                retained.push_back(task);
                continue;
            }
            let now = snapshot.observed_at;
            if !task.terminal_close
                && task.command.action == ProviderTransportSessionAction::Drain
                && task.attempts == 0
            {
                match drain_disposition(&snapshot, &task.fence) {
                    DrainDisposition::Stale => continue,
                    DrainDisposition::Defer => {
                        // No control command can run while the current invocation owns
                        // the socket. Start its bounded close budget once it finishes.
                        task.deferred_while_inflight = true;
                        retained.push_back(task);
                        continue;
                    }
                    DrainDisposition::Ready => {}
                }
            }
            if task.deferred_while_inflight && task.attempts == 0 {
                let logical_ttl = snapshot
                    .sessions
                    .iter()
                    .find(|session| session.fence == task.fence)
                    .map(|session| session.logical_ttl)
                    .unwrap_or(CONTROL_OVERALL_DEADLINE);
                task.overall_deadline =
                    transport_add(now, CONTROL_OVERALL_DEADLINE.min(logical_ttl));
                task.deferred_while_inflight = false;
            }
            if now >= task.overall_deadline || task.attempts >= CONTROL_MAX_ATTEMPTS {
                task.state = CloseTaskState::Exhausted;
                task.last_blocker = Some("provider_physical_connection_close_exhausted");
                tracing::warn!(
                    task_id = %task.task_id, primary = ?task.primary_failure,
                    first_control_failure = task.first_control_failure,
                    generation = task.fence.generation.get(),
                    attempts = task.attempts,
                    overall_deadline_ms = task.overall_deadline.as_millis(),
                    "provider close task exhausted"
                );
                retained.push_back(task);
                continue;
            }
            if now < task.next_due {
                retained.push_back(task);
                continue;
            }
            if !task.terminal_close {
                match task.command.action {
                    ProviderTransportSessionAction::Drain => {
                        match drain_disposition(&snapshot, &task.fence) {
                            DrainDisposition::Stale => continue,
                            DrainDisposition::Defer => {
                                retained.push_back(task);
                                continue;
                            }
                            DrainDisposition::Ready => {}
                        }
                    }
                    ProviderTransportSessionAction::Close => {
                        if !snapshot.sessions.iter().any(|session| {
                            session.fence == task.fence
                                && matches!(
                                    session.state,
                                    TransportSessionState::Faulted
                                        | TransportSessionState::IdleReleased
                                )
                        }) {
                            continue;
                        }
                    }
                }
            }
            task.command.deadline_unix_ms = i64::try_from(
                transport_add(now, CONTROL_DEADLINE)
                    .min(task.overall_deadline)
                    .as_millis(),
            )
            .unwrap_or(i64::MAX);
            task.attempts += 1;
            // The host owns the wire deadline and retirement after a dispatched timeout.
            // Dropping this future externally could leave a late unary response in stdio.
            let result = self
                .runtime
                .transport_session(&task.target_id, task.command.clone())
                .await;
            let evidence = match result {
                Ok(receipt) => accepted_closure_evidence(&task, &receipt),
                Err(error) => Err(control_error_code(&error)),
            };
            let released = match evidence {
                Ok(evidence) => {
                    task.command.worker_incarnation = Some(evidence.identity.worker_incarnation);
                    tracing::info!(task_id = %task.task_id, generation = task.fence.generation.get(),
                        worker_incarnation = evidence.identity.worker_incarnation, source = ?evidence.source,
                        local_released = evidence.local_released, peer_ack = ?evidence.peer_close_acknowledged,
                        no_ack_reason = ?evidence.no_ack_reason, attempt = task.attempts,
                        attempt_deadline_ms = task.command.deadline_unix_ms,
                        overall_deadline_ms = task.overall_deadline.as_millis(),
                        "provider close physical evidence observed");
                    match self
                        .registry
                        .lock()
                        .await
                        .record_closure_evidence(&task.fence, &evidence)
                    {
                        Ok(()) => {
                            task.last_blocker = if evidence.local_released {
                                None
                            } else {
                                Some("provider_transport_close_not_released")
                            };
                            if let Some(code) = task.last_blocker {
                                task.first_control_failure.get_or_insert(code);
                            }
                            evidence.local_released
                        }
                        Err(_) => {
                            task.last_blocker = Some("provider_transport_close_stale_evidence");
                            false
                        }
                    }
                }
                Err(code) => {
                    task.first_control_failure.get_or_insert(code);
                    task.last_blocker = Some(code);
                    false
                }
            };
            if released {
                task.state = CloseTaskState::Released;
            } else {
                let completed_at = self.registry.lock().await.safe_snapshot().observed_at;
                if task.attempts >= CONTROL_MAX_ATTEMPTS || completed_at >= task.overall_deadline {
                    task.state = CloseTaskState::Exhausted;
                } else {
                    task.next_due = transport_add(completed_at, close_retry_delay(&task))
                        .min(task.overall_deadline);
                }
                tracing::warn!(task_id = %task.task_id, primary = ?task.primary_failure,
                    first_control_failure = task.first_control_failure,
                    generation = task.fence.generation.get(), attempts = task.attempts,
                    next_due_ms = task.next_due.as_millis(), attempt_deadline_ms = task.command.deadline_unix_ms,
                    state = ?task.state, blocker = task.last_blocker, overall_deadline_ms = task.overall_deadline.as_millis(),
                    "provider close task retained its bounded recovery outcome");
            }
            retained.push_back(task);
        }
        self.pending_commands
            .lock()
            .expect("transport pending command lock")
            .extend(retained);
    }

    pub(super) fn close_task_exhausted(&self, fence: &TransportFence) -> bool {
        self.pending_commands
            .lock()
            .expect("transport pending command lock")
            .iter()
            .any(|task| task.fence == *fence && task.state == CloseTaskState::Exhausted)
    }

    pub(super) fn recovery_admission_error(
        &self,
        fence: &TransportFence,
        code: &'static str,
    ) -> anyhow::Error {
        let tasks = self
            .pending_commands
            .lock()
            .expect("transport pending command lock");
        let Some(task) = tasks.iter().find(|task| task.fence == *fence) else {
            return transport_error(code);
        };
        let mut error = ProviderRuntimeError::new(
            ProviderRuntimeErrorKind::ProviderTransportAdmissionFailed,
            code,
        );
        error.provider_details = Some(serde_json::json!({
            "transport_recovery": {
                "task_id": task.task_id, "generation": fence.generation.get(),
                "worker_incarnation": task.command.worker_incarnation,
                "primary_error": task.primary_failure.map(|failure| serde_json::json!({
                    "kind": failure.kind, "code": failure.code, "invocation_sequence": failure.invocation_sequence,
                })),
                "first_control_failure": task.first_control_failure, "recovery_blocker": task.last_blocker,
                "attempts": task.attempts, "overall_deadline_unix_ms": task.overall_deadline.as_millis(),
            }
        }));
        PluginFrameworkError::runtime(error).into()
    }

    pub(super) fn publish_termination(&self, mut notice: TransportTerminationNotice) {
        let scopes = self.connection_scopes.lock().expect("transport scopes");
        for scope in scopes.values().filter_map(Weak::upgrade) {
            let state = scope.state.lock().expect("transport connection bindings");
            if let Some(lease) = state.leases.get(notice.fence.session_id.as_str()) {
                if lease.fence == notice.fence {
                    notice.connection_scope_id = Some(scope.id.clone());
                    notice.invocation_sequence = Some(lease.sequence());
                    break;
                }
            }
        }
        let _ = self.notices.send(notice);
    }
}
