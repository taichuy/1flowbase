use super::*;
use crate::orchestration_runtime::debug_stream_events;
use control_plane::ports::{
    AppendTerminalIfMissingAndCloseOutcome, RuntimeEventAfterCommitDelivery,
    RuntimeEventAfterCommitDeliveryStatus, RuntimeEventAfterCommitFailureReason,
    RuntimeEventAfterCommitIdempotencyKey, RuntimeEventAfterCommitLane,
    RuntimeEventAfterCommitSubscriber, RuntimeEventAfterCommitSubscriberId, RuntimeEventClosure,
    RuntimeEventDiagnosticDeliveryStatus, RuntimeEventDiagnosticDropReason, RuntimeEventReceiver,
    RuntimeEventStreamPolicy, RuntimeEventSubscription, RuntimeEventTrimPolicy,
    TrustedRuntimeEventAfterCommitRegistration, RUNTIME_EVENT_AFTER_COMMIT_CONTRACT_ID,
    RUNTIME_EVENT_AFTER_COMMIT_POINT_ID, RUNTIME_EVENT_LANE_CONTRACT_VERSION,
    RUNTIME_EVENT_LANE_OWNER_MODULE_ID,
};
use plugin_framework::extension_bus::{
    compile_extension_graph, Cardinality, ContractDescriptor, ContractVersion,
    ContributionDescriptor, ContributionId, ContributionMode, ContributionOrdering,
    DeliverySemantics, ExtensionBusVersion, ExtensionPointDescriptor, ExtensionPointId,
    ExtensionPointKind, FailureSemantics, LifecycleSemantics, ModuleActivationDeclaration,
    ModuleDescriptor, ModuleId, ModuleKind, ModuleVersion, OrderingSemantics, OverridePolicy,
    ScopeSemantics,
};
use std::{
    collections::BTreeSet,
    sync::{Arc, Mutex},
    time::Duration,
};

#[tokio::test]
async fn streaming_provider_and_answer_deltas_remain_live_without_durable_history() {
    let repository = crate::orchestration_runtime::test_support::InMemoryOrchestrationRuntimeRepository::with_permissions(vec![]);
    let stream = OpenTestRuntimeEventStream::default();
    let run_id = Uuid::now_v7();
    let node_run_id = Uuid::now_v7();
    stream
        .open_run(run_id, RuntimeEventStreamPolicy::debug_default())
        .await
        .unwrap();
    let provider_delta = debug_stream_events::text_delta("node-llm", node_run_id, "A".into());
    let answer_delta =
        debug_stream_events::answer_reasoning_delta("node-answer", "B".into(), 0, None, None, None);
    let mut projected = Vec::new();
    for event in [provider_delta, answer_delta] {
        assert_eq!(event.durability, RuntimeEventDurability::Ephemeral);
        projected.push(stream.append(run_id, event).await.unwrap());
    }
    let live = stream.replay(run_id, Some(0), 10).await.unwrap();
    assert_eq!(live.len(), 2);
    assert_eq!(live[0].payload["text"], "A");
    assert_eq!(live[1].payload["text"], "B");
    assert_eq!(
        live[1].payload["presentation"]["answer_node_id"],
        "node-answer"
    );
    control_plane::orchestration_runtime::persist_runtime_debug_stream_events(
        &repository,
        projected,
    )
    .await
    .unwrap();
    assert!(repository
        .list_runtime_events(run_id, 0)
        .await
        .unwrap()
        .is_empty());
    assert!(repository.events_for_flow_run(run_id).is_empty());
}

#[tokio::test]
async fn completed_nodes_finish_before_the_next_slow_node_returns() {
    let service = OrchestrationRuntimeService::for_tests_with_provider_delay(
        std::time::Duration::from_millis(250),
    );
    let seeded = service
        .seed_application_with_flow("Live node completion")
        .await;
    let stream = Arc::new(crate::_tests::support::RecordingRuntimeEventStream::default());
    let service = service.with_runtime_event_stream(stream.clone());
    let started = service
        .start_flow_debug_run(StartFlowDebugRunCommand {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            input_payload: json!({ "node-start": { "query": "hello" } }),
            document_snapshot: None,
            debug_session_id: None,
        })
        .await
        .unwrap();
    let execution = service.continue_flow_debug_run(ContinueFlowDebugRunCommand {
        application_id: seeded.application_id,
        flow_run_id: started.flow_run.id,
        workspace_id: Uuid::nil(),
    });
    tokio::pin!(execution);

    tokio::time::timeout(std::time::Duration::from_secs(1), async {
        loop {
            let events = stream.events();
            let started = events
                .iter()
                .enumerate()
                .filter(|(_, event)| event.event_type == "node_started")
                .collect::<Vec<_>>();
            if let [first, second, ..] = started.as_slice() {
                let first_node_run_id = first.1.node_run_id;
                assert!(events[first.0 + 1..second.0].iter().any(|event| {
                    event.event_type == "node_finished"
                        && event.node_run_id == first_node_run_id
                        && event.payload["status"] == "succeeded"
                }));
                break;
            }

            tokio::select! {
                result = &mut execution => panic!("flow completed before live node ordering could be observed: {result:?}"),
                _ = tokio::time::sleep(std::time::Duration::from_millis(5)) => {}
            }
        }
    })
    .await
    .expect("a completed node should be visible while the Provider node is still running");

    let detail = execution.await.unwrap();
    let finished_node_run_ids = stream
        .events()
        .into_iter()
        .filter(|event| event.event_type == "node_finished")
        .filter_map(|event| event.node_run_id)
        .collect::<Vec<_>>();
    let unique_finished_node_run_ids = finished_node_run_ids
        .iter()
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        finished_node_run_ids.len(),
        unique_finished_node_run_ids.len(),
        "terminal persistence must not emit duplicate node_finished events"
    );
    assert_eq!(unique_finished_node_run_ids.len(), detail.node_runs.len());
}

#[derive(Default)]
struct OpenTestRuntimeEventStream {
    events: Mutex<Vec<RuntimeEventEnvelope>>,
    subscribers: Mutex<Vec<tokio::sync::mpsc::UnboundedSender<RuntimeEventEnvelope>>>,
    closure: Mutex<Option<RuntimeEventClosure>>,
    terminal_claim: Mutex<()>,
}

#[async_trait::async_trait]
impl RuntimeEventStream for OpenTestRuntimeEventStream {
    async fn open_run(
        &self,
        _run_id: Uuid,
        _policy: RuntimeEventStreamPolicy,
    ) -> anyhow::Result<()> {
        let _terminal_claim = self
            .terminal_claim
            .lock()
            .expect("terminal claim lock should be available");
        *self
            .closure
            .lock()
            .expect("closure lock should be available") = None;
        Ok(())
    }

    async fn append(
        &self,
        run_id: Uuid,
        event: RuntimeEventPayload,
    ) -> anyhow::Result<RuntimeEventEnvelope> {
        let _terminal_claim = self
            .terminal_claim
            .lock()
            .expect("terminal claim lock should be available");
        if self
            .closure
            .lock()
            .expect("closure lock should be available")
            .is_some()
        {
            anyhow::bail!("runtime event stream is closed");
        }
        let envelope = {
            let mut events = self.events.lock().expect("events lock should be available");
            let envelope = RuntimeEventEnvelope::new(run_id, events.len() as i64 + 1, event);
            events.push(envelope.clone());
            envelope
        };
        self.subscribers
            .lock()
            .expect("subscribers lock should be available")
            .retain(|sender| sender.send(envelope.clone()).is_ok());
        Ok(envelope)
    }

    async fn append_terminal_if_missing_and_close(
        &self,
        run_id: Uuid,
        event: RuntimeEventPayload,
    ) -> anyhow::Result<AppendTerminalIfMissingAndCloseOutcome> {
        let incoming_reason = RuntimeEventCloseReason::from_terminal_event_type(&event.event_type)
            .ok_or_else(|| {
                anyhow::anyhow!("runtime event stream terminal append requires a terminal event")
            })?;
        let _terminal_claim = self
            .terminal_claim
            .lock()
            .expect("terminal claim lock should be available");
        let existing_terminal_reason = self
            .events
            .lock()
            .expect("events lock should be available")
            .iter()
            .find_map(|existing| {
                (existing.run_id == run_id)
                    .then(|| {
                        RuntimeEventCloseReason::from_terminal_event_type(&existing.event_type)
                    })
                    .flatten()
            });
        let is_closed = self
            .closure
            .lock()
            .expect("closure lock should be available")
            .is_some();
        if is_closed {
            if existing_terminal_reason.is_some() {
                return Ok(AppendTerminalIfMissingAndCloseOutcome::ExistingTerminal);
            }
            anyhow::bail!("runtime event stream is closed without a terminal event");
        }

        let (outcome, close_reason) = if let Some(existing_reason) = existing_terminal_reason {
            (
                AppendTerminalIfMissingAndCloseOutcome::ExistingTerminal,
                existing_reason,
            )
        } else {
            let envelope = {
                let mut events = self.events.lock().expect("events lock should be available");
                let envelope = RuntimeEventEnvelope::new(run_id, events.len() as i64 + 1, event);
                events.push(envelope.clone());
                envelope
            };
            self.subscribers
                .lock()
                .expect("subscribers lock should be available")
                .retain(|sender| sender.send(envelope.clone()).is_ok());
            (
                AppendTerminalIfMissingAndCloseOutcome::Appended,
                incoming_reason,
            )
        };
        let final_sequence = self
            .events
            .lock()
            .expect("events lock should be available")
            .last()
            .map(|existing| existing.sequence)
            .unwrap_or(0);
        *self
            .closure
            .lock()
            .expect("closure lock should be available") = Some(RuntimeEventClosure {
            reason: close_reason,
            final_sequence,
        });
        self.subscribers
            .lock()
            .expect("subscribers lock should be available")
            .clear();
        Ok(outcome)
    }

    async fn subscribe(
        &self,
        _run_id: Uuid,
        from_sequence: Option<i64>,
    ) -> anyhow::Result<RuntimeEventSubscription> {
        let replay = self
            .events
            .lock()
            .expect("events lock should be available")
            .iter()
            .filter(|event| from_sequence.is_none_or(|sequence| event.sequence > sequence))
            .cloned()
            .collect();
        let closure = *self
            .closure
            .lock()
            .expect("closure lock should be available");
        let (_closure_sender, closure_receiver) = tokio::sync::watch::channel(closure);
        let (sender, live_events) = tokio::sync::mpsc::unbounded_channel();
        if closure.is_none() {
            self.subscribers
                .lock()
                .expect("subscribers lock should be available")
                .push(sender);
        }
        Ok(RuntimeEventSubscription {
            terminal_writer: std::sync::Arc::new(UnusedTerminalWriter),
            replay,
            live_events: crate::ports::RuntimeEventReceiver::from_unbounded(live_events),
            closure: closure_receiver,
        })
    }

    async fn replay(
        &self,
        _run_id: Uuid,
        from_sequence: Option<i64>,
        limit: usize,
    ) -> anyhow::Result<Vec<RuntimeEventEnvelope>> {
        Ok(self
            .events
            .lock()
            .expect("events lock should be available")
            .iter()
            .filter(|event| from_sequence.is_none_or(|sequence| event.sequence > sequence))
            .take(limit)
            .cloned()
            .collect())
    }

    async fn close_run(
        &self,
        _run_id: Uuid,
        reason: RuntimeEventCloseReason,
    ) -> anyhow::Result<()> {
        let _terminal_claim = self
            .terminal_claim
            .lock()
            .expect("terminal claim lock should be available");
        let final_sequence = self
            .events
            .lock()
            .expect("events lock should be available")
            .last()
            .map(|event| event.sequence)
            .unwrap_or(0);
        *self
            .closure
            .lock()
            .expect("closure lock should be available") = Some(RuntimeEventClosure {
            reason,
            final_sequence,
        });
        self.subscribers
            .lock()
            .expect("subscribers lock should be available")
            .clear();
        Ok(())
    }

    async fn trim(&self, _run_id: Uuid, _policy: RuntimeEventTrimPolicy) -> anyhow::Result<()> {
        Ok(())
    }
}

#[tokio::test]
async fn runtime_debug_event_persister_flushes_usage_fact_on_time_window_without_delta_rows() {
    let repository =
        crate::orchestration_runtime::test_support::InMemoryOrchestrationRuntimeRepository::with_permissions(vec![]);
    let stream = std::sync::Arc::new(OpenTestRuntimeEventStream::default());
    let run_id = Uuid::now_v7();
    let node_run_id = Uuid::now_v7();
    stream
        .open_run(run_id, RuntimeEventStreamPolicy::debug_default())
        .await
        .unwrap();
    let handle = control_plane::orchestration_runtime::spawn_runtime_debug_event_persister(
        repository.clone(),
        stream.clone(),
        run_id,
    );
    stream
        .append(
            run_id,
            debug_stream_events::text_delta("node-llm", node_run_id, "及时落盘".into()),
        )
        .await
        .unwrap();

    let live = stream.replay(run_id, Some(0), 10).await.unwrap();
    assert_eq!(live[0].event_type, "text_delta");
    assert_eq!(live[0].payload["text"], "及时落盘");
    stream
        .append(
            run_id,
            debug_stream_events::usage_snapshot(
                "node-llm",
                node_run_id,
                &plugin_framework::provider_contract::ProviderUsage {
                    input_tokens: Some(10),
                    output_tokens: Some(5),
                    total_tokens: Some(15),
                    ..Default::default()
                },
            ),
        )
        .await
        .unwrap();

    let persisted = tokio::time::timeout(std::time::Duration::from_millis(500), async {
        loop {
            let events = repository.list_runtime_events(run_id, 0).await.unwrap();
            if !events.is_empty() {
                break events;
            }
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("usage fact should flush while the runtime stream remains open");

    assert_eq!(persisted.len(), 1);
    assert_eq!(persisted[0].event_type, "usage_snapshot");
    assert_eq!(persisted[0].payload["usage"]["total_tokens"], 15);
    assert!(repository.events_for_flow_run(run_id).is_empty());
    stream
        .close_run(run_id, RuntimeEventCloseReason::Finished)
        .await
        .unwrap();
    handle.await.unwrap();
}

#[tokio::test]
async fn assistant_activity_sequence_is_kept_by_durably_persisted_lifecycle_events() {
    let service = OrchestrationRuntimeService::for_tests();
    let seeded = service
        .seed_application_with_flow("Lifecycle Persistence Owner")
        .await;
    let stream =
        std::sync::Arc::new(crate::_tests::support::RecordingRuntimeEventStream::default());
    let service = service.with_runtime_event_stream(stream.clone());

    let started = service
        .start_flow_debug_run(StartFlowDebugRunCommand {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            input_payload: json!({ "node-start": { "query": "hello" } }),
            document_snapshot: None,
            debug_session_id: None,
        })
        .await
        .unwrap();

    let flow_started = stream
        .events()
        .into_iter()
        .find(|event| event.event_type == "flow_started")
        .expect("flow_started should be delivered to the runtime stream");
    assert!(!flow_started.persist_required);
    assert_eq!(flow_started.durability, RuntimeEventDurability::Ephemeral);
    let durable_flow_started = service
        .list_runtime_events(started.flow_run.id, 0)
        .await
        .into_iter()
        .find(|event| event.event_type == "flow_started")
        .expect("flow_started should remain durable");
    assert_eq!(
        durable_flow_started.payload["stream_sequence"],
        flow_started.sequence
    );
    assert_eq!(
        durable_flow_started.payload["sequence_start"],
        flow_started.sequence
    );
    assert_eq!(
        durable_flow_started.payload["sequence_end"],
        flow_started.sequence
    );
}

#[tokio::test]
async fn runtime_event_persister_does_not_archive_text_fragments() {
    let repository =
        crate::orchestration_runtime::test_support::InMemoryOrchestrationRuntimeRepository::with_permissions(vec![]);
    let run_id = Uuid::now_v7();
    let node_run_id = Uuid::now_v7();
    let events = vec![
        runtime_text_delta(run_id, node_run_id, "退"),
        runtime_text_delta(run_id, node_run_id, "款"),
        runtime_text_delta(run_id, node_run_id, "摘要"),
    ];

    control_plane::orchestration_runtime::persist_runtime_debug_stream_events(&repository, events)
        .await
        .unwrap();

    let runtime_events = repository.list_runtime_events(run_id, 0).await.unwrap();
    assert!(runtime_events.is_empty());
    assert!(repository.events_for_flow_run(run_id).is_empty());
}

#[tokio::test]
async fn runtime_event_persister_keeps_live_delta_metadata_out_of_durable_rows() {
    let repository =
        crate::orchestration_runtime::test_support::InMemoryOrchestrationRuntimeRepository::with_permissions(vec![]);
    let run_id = Uuid::now_v7();
    let node_run_id = Uuid::now_v7();
    let events = vec![
        runtime_text_delta_with_payload(
            run_id,
            7,
            json!({
                "type": "text_delta",
                "node_run_id": node_run_id,
                "node_id": "node-llm",
                "text": "退",
                "text_ref": "runtime_artifact:inline:chunk-1",
                "truncation": {
                    "truncated": true,
                    "reason": "max_bytes",
                    "original_bytes": 200
                }
            }),
        ),
        runtime_text_delta_with_payload(
            run_id,
            8,
            json!({
                "type": "text_delta",
                "node_run_id": node_run_id,
                "node_id": "node-llm",
                "text": "款",
                "artifact_refs": ["runtime_artifact:object:chunk-2"]
            }),
        ),
    ];

    control_plane::orchestration_runtime::persist_runtime_debug_stream_events(&repository, events)
        .await
        .unwrap();

    let runtime_events = repository.list_runtime_events(run_id, 0).await.unwrap();
    assert!(runtime_events.is_empty());
    assert!(repository.events_for_flow_run(run_id).is_empty());
}

#[tokio::test]
async fn runtime_event_persister_does_not_archive_reasoning_fragments() {
    let repository =
        crate::orchestration_runtime::test_support::InMemoryOrchestrationRuntimeRepository::with_permissions(vec![]);
    let run_id = Uuid::now_v7();
    let node_run_id = Uuid::now_v7();
    let events = vec![
        runtime_reasoning_delta(run_id, node_run_id, "先"),
        runtime_reasoning_delta(run_id, node_run_id, "分析"),
        runtime_text_delta(run_id, node_run_id, "结"),
        runtime_text_delta(run_id, node_run_id, "果"),
    ];

    control_plane::orchestration_runtime::persist_runtime_debug_stream_events(&repository, events)
        .await
        .unwrap();

    let runtime_events = repository.list_runtime_events(run_id, 0).await.unwrap();
    assert!(runtime_events.is_empty());
    assert!(repository.events_for_flow_run(run_id).is_empty());
}

#[tokio::test]
async fn assistant_activity_sequence_ends_reasoning_at_a_precommitted_visible_boundary() {
    let repository =
        crate::orchestration_runtime::test_support::InMemoryOrchestrationRuntimeRepository::with_permissions(vec![]);
    let run_id = Uuid::now_v7();
    let node_run_id = Uuid::now_v7();
    let mut tool = RuntimeEventEnvelope::new(
        run_id,
        4,
        debug_stream_events::assistant_tool_call_started(
            run_id,
            node_run_id,
            "node-llm",
            json!({
                "id": "call-1",
                "name": "lookup",
                "arguments": {}
            }),
            json!({ "total_tokens": 12 }),
        ),
    );
    tool.persist_required = false;
    tool.durability = RuntimeEventDurability::Ephemeral;

    control_plane::orchestration_runtime::persist_runtime_debug_stream_events(
        &repository,
        vec![
            runtime_reasoning_delta_with_sequence(run_id, node_run_id, 3, "先检查"),
            tool,
            runtime_reasoning_delta_with_sequence(run_id, node_run_id, 5, "继续检查"),
        ],
    )
    .await
    .unwrap();

    let reasoning = repository
        .list_runtime_events(run_id, 0)
        .await
        .unwrap()
        .into_iter()
        .filter(|event| event.event_type == "reasoning_delta")
        .collect::<Vec<_>>();
    assert!(reasoning.is_empty());
}

#[tokio::test]
async fn runtime_event_persister_preserves_cancelled_terminal_without_delta_rows() {
    let repository =
        crate::orchestration_runtime::test_support::InMemoryOrchestrationRuntimeRepository::with_permissions(vec![]);
    let run_id = Uuid::now_v7();
    let node_run_id = Uuid::now_v7();
    let terminal = RuntimeEventEnvelope::new(
        run_id,
        9,
        RuntimeEventPayload {
            event_type: "flow_cancelled".to_string(),
            source: RuntimeEventSource::Runtime,
            durability: RuntimeEventDurability::DurableRequired,
            persist_required: true,
            trace_visible: true,
            payload: json!({
                "type": "flow_cancelled",
                "run_id": run_id,
                "status": "cancelled",
                "reason": "manual_stop"
            }),
        },
    );

    control_plane::orchestration_runtime::persist_runtime_debug_stream_events(
        &repository,
        vec![
            runtime_text_delta_with_payload(
                run_id,
                7,
                json!({
                    "type": "text_delta",
                    "node_run_id": node_run_id,
                    "node_id": "node-llm",
                    "text": "正在"
                }),
            ),
            runtime_text_delta_with_payload(
                run_id,
                8,
                json!({
                    "type": "text_delta",
                    "node_run_id": node_run_id,
                    "node_id": "node-llm",
                    "text": "回答"
                }),
            ),
            terminal,
        ],
    )
    .await
    .unwrap();

    let runtime_events = repository.list_runtime_events(run_id, 0).await.unwrap();
    assert_eq!(runtime_events.len(), 1);
    assert_eq!(runtime_events[0].event_type, "flow_cancelled");
    assert_eq!(runtime_events[0].payload["stream_sequence"], 9);
    assert_eq!(runtime_events[0].payload["sequence_start"], 9);
    assert_eq!(runtime_events[0].payload["sequence_end"], 9);
    assert_eq!(
        runtime_events[0].layer,
        domain::RuntimeEventLayer::AgentTransition
    );
}

#[tokio::test]
async fn runtime_debug_event_persister_stops_after_incomplete_terminal_event() {
    let repository =
        crate::orchestration_runtime::test_support::InMemoryOrchestrationRuntimeRepository::with_permissions(vec![]);
    let stream = std::sync::Arc::new(OpenTestRuntimeEventStream::default());
    let run_id = Uuid::now_v7();
    stream
        .open_run(run_id, RuntimeEventStreamPolicy::debug_default())
        .await
        .expect("open the debug stream");
    let mut handle = control_plane::orchestration_runtime::spawn_runtime_debug_event_persister(
        repository.clone(),
        stream.clone(),
        run_id,
    );

    stream
        .append(
            run_id,
            debug_stream_events::flow_incomplete(
                run_id,
                json!({ "answer": "partial output at the limit" }),
            ),
        )
        .await
        .expect("publish incomplete terminal");

    let completion = tokio::time::timeout(std::time::Duration::from_millis(250), &mut handle).await;
    if completion.is_err() {
        handle.abort();
    }
    assert!(
        matches!(completion, Ok(Ok(()))),
        "incomplete must flush the batch and stop the persister: {completion:?}"
    );
    let persisted = repository
        .list_runtime_events(run_id, 0)
        .await
        .expect("read persisted debug events");
    assert!(
        persisted
            .iter()
            .any(|event| event.event_type == "flow_incomplete"),
        "incomplete terminal must be durable before the persister stops"
    );
}

#[tokio::test]
async fn runtime_event_stream_fallback_projects_the_durable_terminal_winner() {
    let stream =
        std::sync::Arc::new(crate::_tests::support::RecordingRuntimeEventStream::default());
    let service = OrchestrationRuntimeService::for_tests();
    let seeded = service
        .seed_application_with_flow("Runtime stream durable fallback")
        .await;
    let run = service
        .seed_published_running_run_with_output(&seeded, json!({ "answer": "partial" }))
        .await;
    let winner = service
        .finalize_published_run_missing_stream_terminal(
            FinalizePublishedRunMissingStreamTerminalCommand {
                application_id: seeded.application_id,
                flow_run_id: run.id,
            },
        )
        .await
        .expect("durable recovery should commit the terminal winner");

    stream
        .append(
            run.id,
            RuntimeEventPayload {
                event_type: "flow_started".to_string(),
                source: RuntimeEventSource::Runtime,
                durability: RuntimeEventDurability::DurableRequired,
                persist_required: true,
                trace_visible: true,
                payload: json!({
                    "type": "flow_started",
                    "run_id": run.id,
                }),
            },
        )
        .await
        .unwrap();

    control_plane::orchestration_runtime::project_runtime_event_stream_terminal(
        stream.clone(),
        &winner,
    )
    .await;

    let events = stream.events();
    assert_eq!(
        events.last().map(|event| event.event_type.as_str()),
        Some("flow_failed")
    );
    assert_eq!(
        events.last().map(|event| event.payload["error"].clone()),
        Some(json!("runtime event stream ended without a terminal event"))
    );
    assert_eq!(
        events
            .last()
            .map(|event| event.payload["error_payload"]["message"].clone()),
        Some(json!("runtime event stream ended without a terminal event"))
    );
    assert_eq!(
        stream.close_calls(),
        vec![(run.id, RuntimeEventCloseReason::Failed)]
    );
}

#[tokio::test]
async fn live_provider_delta_is_appended_to_runtime_event_stream() {
    let service = OrchestrationRuntimeService::for_tests();
    let seeded = service.seed_application_with_flow("Support Agent").await;
    let stream =
        std::sync::Arc::new(crate::_tests::support::RecordingRuntimeEventStream::default());
    let service = service.with_runtime_event_stream(stream.clone());

    let detail = service
        .start_flow_debug_run(StartFlowDebugRunCommand {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            input_payload: serde_json::json!({ "node-start": { "query": "hello" } }),
            document_snapshot: None,
            debug_session_id: None,
        })
        .await
        .unwrap();

    service
        .continue_flow_debug_run(ContinueFlowDebugRunCommand {
            application_id: seeded.application_id,
            flow_run_id: detail.flow_run.id,
            workspace_id: Uuid::nil(),
        })
        .await
        .unwrap();

    assert!(stream
        .events()
        .iter()
        .any(|event| event.event_type == "text_delta"));
}

#[tokio::test]
async fn answer_template_static_text_is_projected_as_answer_presentation_delta() {
    let service = OrchestrationRuntimeService::for_tests();
    let seeded = service
        .seed_application_with_second_llm_failure_flow("Support Agent")
        .await;
    let stream =
        std::sync::Arc::new(crate::_tests::support::RecordingRuntimeEventStream::default());
    let service = service.with_runtime_event_stream(stream.clone());

    let started = service
        .start_flow_debug_run(StartFlowDebugRunCommand {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            input_payload: serde_json::json!({ "node-start": { "query": "hello" } }),
            document_snapshot: None,
            debug_session_id: None,
        })
        .await
        .unwrap();
    let detail = service
        .continue_flow_debug_run(ContinueFlowDebugRunCommand {
            application_id: seeded.application_id,
            flow_run_id: started.flow_run.id,
            workspace_id: Uuid::nil(),
        })
        .await
        .unwrap();

    let presentation_text = stream
        .events()
        .into_iter()
        .filter(|event| event.event_type == "text_delta")
        .filter(|event| event.payload["presentation"]["kind"].as_str() == Some("answer"))
        .filter_map(|event| {
            event
                .payload
                .get("text")
                .and_then(serde_json::Value::as_str)
                .map(str::to_string)
        })
        .collect::<String>();

    assert_eq!(presentation_text, detail.flow_run.output_payload["answer"]);
    assert!(
        presentation_text.contains("\n----\n"),
        "Answer Presentation should include static template text: {presentation_text}"
    );
}

#[tokio::test]
async fn ac_005_success_materializes_answer_after_durable_terminal_without_durable_deltas() {
    let service = OrchestrationRuntimeService::for_tests();
    let seeded = service.seed_application_with_flow("Support Agent").await;
    let stream =
        std::sync::Arc::new(crate::_tests::support::RecordingRuntimeEventStream::default());
    let service = service.with_runtime_event_stream(stream.clone());

    let started = service
        .start_flow_debug_run(StartFlowDebugRunCommand {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            input_payload: json!({ "node-start": { "query": "hello" } }),
            document_snapshot: None,
            debug_session_id: None,
        })
        .await
        .unwrap();
    let detail = service
        .continue_flow_debug_run(ContinueFlowDebugRunCommand {
            application_id: seeded.application_id,
            flow_run_id: started.flow_run.id,
            workspace_id: Uuid::nil(),
        })
        .await
        .unwrap();

    let durable_events = service.list_runtime_events(detail.flow_run.id, 0).await;
    assert!(durable_events
        .iter()
        .any(|event| event.event_type == "flow_finished"));
    assert!(!durable_events
        .iter()
        .any(|event| { event.payload["presentation"]["kind"].as_str() == Some("answer") }));

    let live_events = stream.events();
    let presentation_text = live_events
        .iter()
        .filter(|event| event.payload["presentation"]["kind"].as_str() == Some("answer"))
        .filter(|event| event.event_type == "text_delta")
        .filter_map(|event| event.payload["text"].as_str())
        .collect::<String>();
    let answer_terminal_position = live_events
        .iter()
        .rposition(|event| event.event_type == "flow_finished")
        .unwrap();
    let last_presentation_position = live_events
        .iter()
        .rposition(|event| event.payload["presentation"]["kind"].as_str() == Some("answer"))
        .unwrap();

    assert_eq!(presentation_text, detail.flow_run.output_payload["answer"]);
    assert!(last_presentation_position < answer_terminal_position);
}

#[tokio::test]
async fn success_persistence_failure_never_projects_a_success_terminal() {
    let service = OrchestrationRuntimeService::for_tests();
    let seeded = service
        .seed_application_with_flow("Terminal persistence barrier")
        .await;
    let stream =
        std::sync::Arc::new(crate::_tests::support::RecordingRuntimeEventStream::default());
    let service = service.with_runtime_event_stream(stream.clone());
    let started = service
        .start_flow_debug_run(StartFlowDebugRunCommand {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            input_payload: json!({ "node-start": { "query": "hello" } }),
            document_snapshot: None,
            debug_session_id: None,
        })
        .await
        .expect("run should start");
    service.fail_next_terminal_runtime_event_append().await;

    let _ = service
        .continue_flow_debug_run(ContinueFlowDebugRunCommand {
            application_id: seeded.application_id,
            flow_run_id: started.flow_run.id,
            workspace_id: Uuid::nil(),
        })
        .await;

    assert!(stream
        .events()
        .iter()
        .all(|event| event.event_type != "flow_finished"));
    assert!(service
        .list_runtime_events(started.flow_run.id, 0)
        .await
        .iter()
        .all(|event| event.event_type != "flow_finished"));
}

#[path = "runtime_events/resource_budgets.rs"]
mod resource_budgets;

pub(super) use support::UnusedTerminalWriter;

#[path = "runtime_events/after_commit_lanes.rs"]
mod after_commit_lanes;

mod live_provider_events;
#[path = "runtime_events/support.rs"]
mod support;
