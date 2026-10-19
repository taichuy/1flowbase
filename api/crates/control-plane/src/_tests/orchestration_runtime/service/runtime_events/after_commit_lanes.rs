use super::*;

#[derive(Clone, Copy)]
enum AfterCommitBehavior {
    Success,
    Error,
    ErrorOnce,
    Timeout,
}

struct RecordingAfterCommitSubscriber {
    behavior: AfterCommitBehavior,
    calls: Arc<Mutex<Vec<RuntimeEventAfterCommitIdempotencyKey>>>,
}

#[async_trait::async_trait]
impl RuntimeEventAfterCommitSubscriber for RecordingAfterCommitSubscriber {
    async fn deliver(&self, delivery: RuntimeEventAfterCommitDelivery) -> anyhow::Result<()> {
        let call_number = {
            let mut calls = self.calls.lock().expect("after-commit calls lock poisoned");
            calls.push(delivery.idempotency_key);
            calls.len()
        };
        match self.behavior {
            AfterCommitBehavior::Success => Ok(()),
            AfterCommitBehavior::Error => anyhow::bail!("subscriber details stay private"),
            AfterCommitBehavior::ErrorOnce if call_number == 1 => {
                anyhow::bail!("subscriber details stay private")
            }
            AfterCommitBehavior::ErrorOnce => Ok(()),
            AfterCommitBehavior::Timeout => {
                tokio::time::sleep(Duration::from_millis(20)).await;
                Ok(())
            }
        }
    }
}

fn after_commit_graph() -> Arc<plugin_framework::extension_bus::EffectiveExtensionGraph> {
    let core = ModuleDescriptor {
        bus_version: ExtensionBusVersion::V1,
        module_id: ModuleId::new(RUNTIME_EVENT_LANE_OWNER_MODULE_ID).unwrap(),
        module_version: ModuleVersion::new("1").unwrap(),
        module_kind: ModuleKind::BootCore,
        activation: ModuleActivationDeclaration::Active,
        dependencies: BTreeSet::new(),
        granted_permissions: BTreeSet::new(),
        extension_points: vec![ExtensionPointDescriptor {
            point_id: ExtensionPointId::new(RUNTIME_EVENT_AFTER_COMMIT_POINT_ID).unwrap(),
            owner_module_id: ModuleId::new(RUNTIME_EVENT_LANE_OWNER_MODULE_ID).unwrap(),
            point_kind: ExtensionPointKind::EventStream,
            contract: ContractDescriptor::new(
                RUNTIME_EVENT_AFTER_COMMIT_CONTRACT_ID,
                RUNTIME_EVENT_LANE_CONTRACT_VERSION,
            )
            .unwrap(),
            scope: ScopeSemantics::Global,
            cardinality: Cardinality::Many,
            ordering: OrderingSemantics::Dependency,
            failure: FailureSemantics::IsolateContribution,
            delivery: DeliverySemantics::AfterCommitDurable,
            lifecycle: LifecycleSemantics::Invocation,
            allowed_permissions: BTreeSet::new(),
            override_policy: OverridePolicy::Sealed,
        }],
        contributions: Vec::new(),
    };
    let subscriber = ModuleDescriptor {
        bus_version: ExtensionBusVersion::V1,
        module_id: ModuleId::new("fixture.runtime-event-subscriber").unwrap(),
        module_version: ModuleVersion::new("1").unwrap(),
        module_kind: ModuleKind::TrustedHost,
        activation: ModuleActivationDeclaration::Active,
        dependencies: BTreeSet::new(),
        granted_permissions: BTreeSet::new(),
        extension_points: Vec::new(),
        contributions: vec![ContributionDescriptor {
            contribution_id: ContributionId::new("fixture.runtime-event.after-commit").unwrap(),
            contributor_module_id: ModuleId::new("fixture.runtime-event-subscriber").unwrap(),
            point_id: ExtensionPointId::new(RUNTIME_EVENT_AFTER_COMMIT_POINT_ID).unwrap(),
            contract_version: ContractVersion::new(RUNTIME_EVENT_LANE_CONTRACT_VERSION).unwrap(),
            required_permissions: BTreeSet::new(),
            mode: ContributionMode::Append,
            ordering: ContributionOrdering::default(),
        }],
    };
    Arc::new(compile_extension_graph(vec![core, subscriber]).unwrap())
}

fn after_commit_lane(
    behavior: AfterCommitBehavior,
    calls: Arc<Mutex<Vec<RuntimeEventAfterCommitIdempotencyKey>>>,
    timeout: Duration,
    max_attempts: u32,
) -> RuntimeEventAfterCommitLane {
    RuntimeEventAfterCommitLane::from_graph(
        after_commit_graph(),
        vec![TrustedRuntimeEventAfterCommitRegistration {
            contribution_id: "fixture.runtime-event.after-commit".to_string(),
            subscriber_id: RuntimeEventAfterCommitSubscriberId::new("fixture.subscriber").unwrap(),
            timeout,
            max_attempts,
            subscriber: Arc::new(RecordingAfterCommitSubscriber { behavior, calls }),
        }],
    )
    .unwrap()
}

fn durable_lane_event(run_id: Uuid, sequence: i64) -> RuntimeEventEnvelope {
    RuntimeEventEnvelope::new(
        run_id,
        sequence,
        RuntimeEventPayload {
            event_type: "node_finished".to_string(),
            source: RuntimeEventSource::Runtime,
            durability: RuntimeEventDurability::DurableRequired,
            persist_required: true,
            trace_visible: true,
            payload: json!({ "type": "node_finished" }),
        },
    )
}

#[tokio::test]
async fn one_flow_uses_required_diagnostic_and_after_commit_lanes_with_distinct_semantics() {
    let run_id = Uuid::now_v7();
    let (required, diagnostic, mut receiver) = RuntimeEventReceiver::bounded_lanes(1);
    required.send(durable_lane_event(run_id, 1)).await.unwrap();
    let blocked_required = required.send(durable_lane_event(run_id, 2));
    tokio::pin!(blocked_required);
    assert!(
        tokio::time::timeout(Duration::from_millis(10), &mut blocked_required)
            .await
            .is_err()
    );

    assert_eq!(receiver.recv().await.unwrap().sequence, 1);
    blocked_required.await.unwrap();
    assert_eq!(receiver.recv().await.unwrap().sequence, 2);

    assert_eq!(
        diagnostic
            .try_send(RuntimeEventEnvelope::new(
                run_id,
                3,
                debug_stream_events::heartbeat()
            ))
            .status,
        RuntimeEventDiagnosticDeliveryStatus::Delivered
    );
    let dropped = diagnostic.try_send(RuntimeEventEnvelope::new(
        run_id,
        4,
        debug_stream_events::heartbeat(),
    ));
    assert_eq!(
        dropped.status,
        RuntimeEventDiagnosticDeliveryStatus::Dropped(
            RuntimeEventDiagnosticDropReason::ReceiverFull
        )
    );

    let repository = crate::orchestration_runtime::test_support::InMemoryOrchestrationRuntimeRepository::with_permissions(vec![]);
    let calls = Arc::new(Mutex::new(Vec::new()));
    let lane = after_commit_lane(
        AfterCommitBehavior::Success,
        Arc::clone(&calls),
        Duration::from_secs(1),
        1,
    );
    let receipts = control_plane::orchestration_runtime::persist_runtime_debug_stream_events_with_after_commit(
        &repository,
        vec![durable_lane_event(run_id, 5)],
        &lane,
    )
    .await
    .unwrap();

    assert_eq!(
        receipts[0].subscribers[0].status,
        RuntimeEventAfterCommitDeliveryStatus::Delivered
    );
    assert_eq!(calls.lock().unwrap().len(), 1);
    assert_eq!(
        repository
            .list_runtime_events(run_id, 0)
            .await
            .unwrap()
            .len(),
        1
    );
}

#[tokio::test]
async fn after_commit_runs_only_after_durable_append_and_suppresses_duplicate_keys() {
    let repository = crate::orchestration_runtime::test_support::InMemoryOrchestrationRuntimeRepository::with_permissions(vec![]);
    let run_id = Uuid::now_v7();
    let event = durable_lane_event(run_id, 7);
    let calls = Arc::new(Mutex::new(Vec::new()));
    let lane = after_commit_lane(
        AfterCommitBehavior::Success,
        Arc::clone(&calls),
        Duration::from_secs(1),
        1,
    );

    repository.fail_next_runtime_event_append();
    assert!(control_plane::orchestration_runtime::persist_runtime_debug_stream_events_with_after_commit(
        &repository,
        vec![event.clone()],
        &lane,
    )
    .await
    .is_err());
    assert!(calls.lock().unwrap().is_empty());

    let first = control_plane::orchestration_runtime::persist_runtime_debug_stream_events_with_after_commit(
        &repository,
        vec![event.clone()],
        &lane,
    )
    .await
    .unwrap();
    let duplicate = control_plane::orchestration_runtime::persist_runtime_debug_stream_events_with_after_commit(
        &repository,
        vec![event],
        &lane,
    )
    .await
    .unwrap();

    assert_eq!(
        first[0].subscribers[0].status,
        RuntimeEventAfterCommitDeliveryStatus::Delivered
    );
    assert_eq!(
        duplicate[0].subscribers[0].status,
        RuntimeEventAfterCommitDeliveryStatus::DuplicateSuppressed
    );
    assert_eq!(calls.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn after_commit_timeout_and_error_retries_are_finite_and_do_not_rollback_append() {
    for (behavior, expected_reason) in [
        (
            AfterCommitBehavior::Timeout,
            RuntimeEventAfterCommitFailureReason::Timeout,
        ),
        (
            AfterCommitBehavior::Error,
            RuntimeEventAfterCommitFailureReason::SubscriberError,
        ),
    ] {
        let repository = crate::orchestration_runtime::test_support::InMemoryOrchestrationRuntimeRepository::with_permissions(vec![]);
        let run_id = Uuid::now_v7();
        let calls = Arc::new(Mutex::new(Vec::new()));
        let lane = after_commit_lane(behavior, Arc::clone(&calls), Duration::from_millis(1), 2);

        let receipts = control_plane::orchestration_runtime::persist_runtime_debug_stream_events_with_after_commit(
            &repository,
            vec![durable_lane_event(run_id, 1)],
            &lane,
        )
        .await
        .unwrap();
        let receipt = &receipts[0].subscribers[0];

        assert_eq!(
            receipt.status,
            RuntimeEventAfterCommitDeliveryStatus::Failed
        );
        assert_eq!(receipt.attempts, 2);
        assert_eq!(receipt.failure_reason, Some(expected_reason));
        assert_eq!(calls.lock().unwrap().len(), 2);
        assert_eq!(
            repository
                .list_runtime_events(run_id, 0)
                .await
                .unwrap()
                .len(),
            1
        );
    }
}

#[tokio::test]
async fn failed_after_commit_claim_is_released_for_replay_and_completed_claim_is_suppressed() {
    let repository = crate::orchestration_runtime::test_support::InMemoryOrchestrationRuntimeRepository::with_permissions(vec![]);
    let run_id = Uuid::now_v7();
    let event = durable_lane_event(run_id, 9);
    let calls = Arc::new(Mutex::new(Vec::new()));
    let lane = after_commit_lane(
        AfterCommitBehavior::ErrorOnce,
        Arc::clone(&calls),
        Duration::from_secs(1),
        1,
    );

    let first = control_plane::orchestration_runtime::persist_runtime_debug_stream_events_with_after_commit(
        &repository,
        vec![event.clone()],
        &lane,
    )
    .await
    .unwrap();
    let replay = control_plane::orchestration_runtime::persist_runtime_debug_stream_events_with_after_commit(
        &repository,
        vec![event.clone()],
        &lane,
    )
    .await
    .unwrap();
    let completed_duplicate = control_plane::orchestration_runtime::persist_runtime_debug_stream_events_with_after_commit(
        &repository,
        vec![event],
        &lane,
    )
    .await
    .unwrap();

    assert_eq!(
        first[0].subscribers[0].status,
        RuntimeEventAfterCommitDeliveryStatus::Failed
    );
    assert_eq!(
        replay[0].subscribers[0].status,
        RuntimeEventAfterCommitDeliveryStatus::Delivered
    );
    assert_eq!(
        completed_duplicate[0].subscribers[0].status,
        RuntimeEventAfterCommitDeliveryStatus::DuplicateSuppressed
    );
    assert_eq!(calls.lock().unwrap().len(), 2);
}

#[tokio::test]
async fn ephemeral_delta_creates_neither_history_nor_after_commit_delivery() {
    let repository = crate::orchestration_runtime::test_support::InMemoryOrchestrationRuntimeRepository::with_permissions(vec![]);
    let run_id = Uuid::now_v7();
    let calls = Arc::new(Mutex::new(Vec::new()));
    let lane = after_commit_lane(
        AfterCommitBehavior::Success,
        Arc::clone(&calls),
        Duration::from_secs(1),
        1,
    );
    let event = RuntimeEventEnvelope::new(
        run_id,
        1,
        debug_stream_events::text_delta("node-llm", Uuid::now_v7(), "history".into()),
    );

    let receipts = control_plane::orchestration_runtime::persist_runtime_debug_stream_events_with_after_commit(
        &repository,
        vec![event],
        &lane,
    )
    .await
    .unwrap();

    assert!(receipts.is_empty());
    assert!(calls.lock().unwrap().is_empty());
    assert_eq!(
        repository
            .list_runtime_events(run_id, 0)
            .await
            .unwrap()
            .len(),
        0
    );
}
