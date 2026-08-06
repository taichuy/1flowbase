use super::*;
use crate::orchestration_runtime::debug_stream_events;
use control_plane::ports::{
    AppendTerminalIfMissingAndCloseOutcome, RuntimeEventClosure, RuntimeEventStreamPolicy,
    RuntimeEventSubscription, RuntimeEventTrimPolicy,
};
use std::sync::{Arc, Mutex};

#[test]
fn streaming_deltas_request_history_without_requiring_ephemeral_delivery() {
    let node_run_id = Uuid::now_v7();
    let provider_delta = debug_stream_events::text_delta("node-llm", node_run_id, "A".into());
    let answer_delta =
        debug_stream_events::answer_reasoning_delta("node-answer", "B".into(), 0, None, None, None);

    for event in [provider_delta, answer_delta] {
        assert_eq!(event.durability, RuntimeEventDurability::Ephemeral);
        assert!(event.persist_required);
    }
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
async fn runtime_debug_event_persister_flushes_delta_batch_on_time_window() {
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
    .expect("delta batch should flush while the runtime stream remains open");

    assert_eq!(persisted.len(), 1);
    assert_eq!(persisted[0].payload["text"], "及时落盘");
    stream
        .close_run(run_id, RuntimeEventCloseReason::Finished)
        .await
        .unwrap();
    handle.await.unwrap();
}

#[tokio::test]
async fn durably_persisted_lifecycle_event_is_not_queued_for_persistence_again() {
    let service = OrchestrationRuntimeService::for_tests();
    let seeded = service
        .seed_application_with_flow("Lifecycle Persistence Owner")
        .await;
    let stream =
        std::sync::Arc::new(crate::_tests::support::RecordingRuntimeEventStream::default());
    let service = service.with_runtime_event_stream(stream.clone());

    service
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
}

#[tokio::test]
async fn runtime_event_persister_coalesces_text_delta_runtime_events() {
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
    assert_eq!(runtime_events.len(), 1);
    assert_eq!(runtime_events[0].event_type, "text_delta");
    assert_eq!(runtime_events[0].node_run_id, Some(node_run_id));
    assert_eq!(
        runtime_events[0].layer,
        domain::RuntimeEventLayer::RuntimeItem
    );
    assert_eq!(runtime_events[0].source, domain::RuntimeEventSource::Host);
    assert_eq!(
        runtime_events[0].visibility,
        domain::RuntimeEventVisibility::Workspace
    );
    assert_eq!(
        runtime_events[0].durability,
        domain::RuntimeEventDurability::Durable
    );
    assert_eq!(runtime_events[0].payload["text"], "退款摘要");
    let run_events = repository.events_for_flow_run(run_id);
    assert!(run_events.is_empty());
}

#[tokio::test]
async fn runtime_event_persister_persists_delta_cursor_and_artifact_metadata() {
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
    assert_eq!(runtime_events.len(), 1);
    let event = &runtime_events[0];
    assert_eq!(event.node_run_id, Some(node_run_id));
    assert_eq!(event.event_type, "text_delta");
    assert_eq!(event.payload["event_type"], "text_delta");
    assert_eq!(event.payload["node_run_id"], node_run_id.to_string());
    assert_eq!(event.payload["content_type"], "text");
    assert_eq!(event.payload["stream_sequence"], 8);
    assert_eq!(event.payload["sequence_start"], 7);
    assert_eq!(event.payload["sequence_end"], 8);
    assert_eq!(
        event.payload["event_ids"],
        json!([format!("{run_id}:7"), format!("{run_id}:8")])
    );
    assert_eq!(event.payload["truncated"], true);
    assert_eq!(event.payload["truncation"]["reason"], "max_bytes");
    assert_eq!(event.payload["truncation"]["original_bytes"], 200);
    assert_eq!(
        event.payload["content_refs"],
        json!(["runtime_artifact:inline:chunk-1"])
    );
    assert_eq!(
        event.payload["artifact_refs"],
        json!([
            "runtime_artifact:inline:chunk-1",
            "runtime_artifact:object:chunk-2"
        ])
    );
}

#[tokio::test]
async fn runtime_event_persister_coalesces_reasoning_delta_separately_from_text() {
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
    assert_eq!(runtime_events.len(), 2);
    assert_eq!(runtime_events[0].event_type, "reasoning_delta");
    assert_eq!(runtime_events[0].payload["text"], "先分析");
    assert_eq!(runtime_events[1].event_type, "text_delta");
    assert_eq!(runtime_events[1].payload["text"], "结果");
}

#[tokio::test]
async fn runtime_event_persister_flushes_pending_delta_before_cancelled_terminal_event() {
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
    assert_eq!(runtime_events.len(), 2);
    assert_eq!(runtime_events[0].event_type, "text_delta");
    assert_eq!(runtime_events[0].payload["text"], "正在回答");
    assert_eq!(runtime_events[1].event_type, "flow_cancelled");
    assert_eq!(runtime_events[1].payload["stream_sequence"], 9);
    assert_eq!(runtime_events[1].payload["sequence_start"], 9);
    assert_eq!(runtime_events[1].payload["sequence_end"], 9);
    assert_eq!(
        runtime_events[1].layer,
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

#[tokio::test]
async fn live_provider_reasoning_signature_delta_is_transient_in_runtime_event_stream() {
    let service = OrchestrationRuntimeService::for_tests_with_provider_events(vec![
        plugin_framework::provider_contract::ProviderStreamEvent::ReasoningDelta {
            delta: "先分析".into(),
        },
        plugin_framework::provider_contract::ProviderStreamEvent::ReasoningSignatureDelta {
            signature: "opaque-signature-fixture".into(),
        },
        plugin_framework::provider_contract::ProviderStreamEvent::TextDelta {
            delta: "结果".into(),
        },
    ]);
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

    let detail = service
        .continue_flow_debug_run(ContinueFlowDebugRunCommand {
            application_id: seeded.application_id,
            flow_run_id: detail.flow_run.id,
            workspace_id: Uuid::nil(),
        })
        .await
        .unwrap();

    let llm_node = detail
        .node_runs
        .iter()
        .find(|node_run| node_run.node_id == "node-llm")
        .expect("llm node run should be persisted");
    assert_eq!(llm_node.output_payload["text"], "<think>先分析</think>结果");
    assert!(llm_node.output_payload.get("reasoning_content").is_none());
    assert!(llm_node.output_payload.get("attempts").is_none());
    assert!(llm_node.output_payload.get("event_count").is_none());
    assert!(llm_node.output_payload.get("provider_code").is_none());
    assert_eq!(
        llm_node.output_payload["provider_route"]["provider_code"],
        "fixture_provider"
    );
    assert!(llm_node.debug_payload.get("reasoning_content").is_none());
    assert!(llm_node.debug_payload.get("provider_route").is_none());

    let events = stream.events();
    assert!(events
        .iter()
        .any(|event| event.event_type == "reasoning_delta" && event.payload["text"] == "先分析"));
    let signature_event = events
        .iter()
        .find(|event| event.event_type == "reasoning_signature_delta")
        .expect("reasoning signature should reach the live runtime stream");
    assert_eq!(
        signature_event.payload["signature"],
        "opaque-signature-fixture"
    );
    assert_eq!(
        signature_event.durability,
        RuntimeEventDurability::Ephemeral
    );
    assert!(!signature_event.persist_required);
    assert!(!signature_event.trace_visible);
    assert!(events.iter().any(|event| event.event_type == "text_delta"));
}

#[tokio::test]
async fn live_provider_text_delta_with_think_tags_is_split_into_reasoning_and_answer() {
    let service = OrchestrationRuntimeService::for_tests_with_provider_events(vec![
        plugin_framework::provider_contract::ProviderStreamEvent::TextDelta {
            delta: "<think>先分析".into(),
        },
        plugin_framework::provider_contract::ProviderStreamEvent::TextDelta {
            delta: "用户问题</think>正式回答".into(),
        },
    ]);
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

    let events = stream.events();
    let reasoning_text = events
        .iter()
        .filter(|event| event.event_type == "reasoning_delta")
        .filter(|event| event.payload["presentation"]["kind"].as_str() == Some("answer"))
        .filter_map(|event| event.payload["text"].as_str())
        .collect::<String>();
    let answer_text = events
        .iter()
        .filter(|event| event.event_type == "text_delta")
        .filter(|event| event.payload["presentation"]["kind"].as_str() == Some("answer"))
        .filter_map(|event| event.payload["text"].as_str())
        .collect::<String>();

    assert_eq!(reasoning_text, "先分析用户问题");
    assert_eq!(answer_text, "正式回答");
    assert!(!events.iter().any(|event| event
        .payload
        .get("text")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|text| text.contains("<think>") || text.contains("</think>"))));
}

#[tokio::test]
async fn fast_stream_provider_events_are_durably_persisted_to_runtime_observability() {
    use plugin_framework::provider_contract::{
        ProviderFinishReason, ProviderStreamEvent, ProviderToolCall, ProviderUsage,
    };

    let service = OrchestrationRuntimeService::for_tests_with_provider_events(vec![
        ProviderStreamEvent::TextDelta {
            delta: "hello".to_string(),
        },
        ProviderStreamEvent::ToolCallCommit {
            call: ProviderToolCall {
                id: "call-1".to_string(),
                name: "lookup_policy".to_string(),
                arguments: json!({ "query": "refund" }),
                provider_metadata: json!({}),
            },
        },
        ProviderStreamEvent::UsageSnapshot {
            usage: ProviderUsage {
                input_tokens: Some(10),
                output_tokens: Some(5),
                reasoning_tokens: None,
                input_cache_hit_tokens: None,
                input_cache_miss_tokens: None,
                cache_read_tokens: None,
                cache_write_tokens: None,
                total_tokens: Some(15),
            },
        },
        ProviderStreamEvent::Finish {
            reason: ProviderFinishReason::Stop,
        },
    ]);
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

    let live_usage = stream
        .events()
        .into_iter()
        .find(|event| event.event_type == "usage_snapshot")
        .expect("typed provider usage should be projected to the live runtime stream");
    assert_eq!(live_usage.payload["usage"]["input_tokens"], json!(10));
    assert_eq!(live_usage.payload["usage"]["total_tokens"], json!(15));

    let runtime_event_types = service
        .list_runtime_events(detail.flow_run.id, 0)
        .await
        .into_iter()
        .map(|event| event.event_type)
        .collect::<Vec<_>>();
    assert!(
        runtime_event_types.iter().any(|event_type| event_type == "text_delta"),
        "provider text deltas should still be written to durable runtime_events: {runtime_event_types:?}"
    );
    assert!(
        runtime_event_types
            .iter()
            .any(|event_type| event_type == "tool_call_commit"),
        "provider tool commits should still be written to durable runtime_events: {runtime_event_types:?}"
    );
    assert!(
        runtime_event_types
            .iter()
            .any(|event_type| event_type == "usage_snapshot"),
        "provider usage snapshots should still be written to durable runtime_events: {runtime_event_types:?}"
    );
    assert!(
        runtime_event_types.iter().any(|event_type| event_type == "finish"),
        "provider finish events should still be written to durable runtime_events: {runtime_event_types:?}"
    );

    let capability_invocations = service
        .list_capability_invocations(detail.flow_run.id)
        .await;
    assert!(
        capability_invocations
            .iter()
            .any(|invocation| invocation.capability_id.contains("lookup_policy")),
        "provider tool commits should still create capability intent records: {capability_invocations:?}"
    );
}

#[tokio::test]
async fn provider_error_after_live_delta_drains_runtime_event_stream_forwarding() {
    let service = OrchestrationRuntimeService::for_tests_with_live_events_then_error(vec![
        plugin_framework::provider_contract::ProviderStreamEvent::TextDelta {
            delta: "partial before error".to_string(),
        },
    ]);
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

    let failed_detail = service
        .continue_flow_debug_run(ContinueFlowDebugRunCommand {
            application_id: seeded.application_id,
            flow_run_id: detail.flow_run.id,
            workspace_id: Uuid::nil(),
        })
        .await
        .unwrap();

    assert_eq!(failed_detail.flow_run.status, domain::FlowRunStatus::Failed);
    let error_payload = failed_detail
        .flow_run
        .error_payload
        .as_ref()
        .expect("provider failure must retain its canonical error");
    assert_eq!(
        error_payload["error_code"],
        json!("provider_invalid_response")
    );
    assert_eq!(
        error_payload["message"],
        json!("provider failed after live events"),
        "the durable error must keep the provider runtime message"
    );
    let event_types = stream
        .events()
        .into_iter()
        .map(|event| event.event_type)
        .collect::<Vec<_>>();
    let text_delta_index = event_types
        .iter()
        .position(|event_type| event_type == "text_delta")
        .expect("text_delta should be appended before provider error returns");
    let flow_failed_index = event_types
        .iter()
        .position(|event_type| event_type == "flow_failed")
        .expect("failed run should append flow_failed");
    assert!(
        text_delta_index < flow_failed_index,
        "text_delta should be drained before flow_failed: {event_types:?}"
    );
}

#[tokio::test]
async fn provider_error_after_live_delta_does_not_project_failure_as_answer() {
    let service = OrchestrationRuntimeService::for_tests_with_live_events_then_error(vec![
        plugin_framework::provider_contract::ProviderStreamEvent::TextDelta {
            delta: "partial before error".to_string(),
        },
    ]);
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

    let failed_detail = service
        .continue_flow_debug_run(ContinueFlowDebugRunCommand {
            application_id: seeded.application_id,
            flow_run_id: detail.flow_run.id,
            workspace_id: Uuid::nil(),
        })
        .await
        .unwrap();

    assert_eq!(failed_detail.flow_run.status, domain::FlowRunStatus::Failed);
    assert!(
        failed_detail
            .flow_run
            .output_payload
            .get("answer")
            .is_none(),
        "provider failure text must not become an Answer"
    );
    let llm_node = node_run(&failed_detail, "node-llm");
    assert_eq!(llm_node.status, domain::NodeRunStatus::Failed);
    assert!(llm_node.output_payload.get("text").is_none());
    assert!(llm_node.output_payload.get("usage").is_none());
    assert!(llm_node.output_payload.get("tool_calls").is_none());
    assert_eq!(
        llm_node.error_payload.as_ref().unwrap()["error_code"],
        json!("provider_invalid_response")
    );
    assert!(failed_detail
        .node_runs
        .iter()
        .all(|node_run| node_run.node_id != "node-answer"));
    let durable_events = service
        .list_runtime_events(failed_detail.flow_run.id, 0)
        .await;
    assert_eq!(
        durable_events
            .iter()
            .filter(|event| event.event_type == "flow_failed")
            .count(),
        1
    );
    assert!(durable_events
        .iter()
        .all(|event| event.event_type != "flow_finished"));
    let live_events = stream.events();
    let presented_answer = live_events
        .iter()
        .filter(|event| event.event_type == "text_delta")
        .filter(|event| event.payload["presentation"]["kind"] == json!("answer"))
        .filter_map(|event| event.payload["text"].as_str())
        .collect::<String>();
    assert_eq!(presented_answer, "partial before error");
}

#[tokio::test]
async fn successful_live_debug_run_emits_flow_lifecycle_and_closes_runtime_stream() {
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

    let event_types = stream
        .events()
        .into_iter()
        .map(|event| event.event_type)
        .collect::<Vec<_>>();
    assert!(event_types
        .iter()
        .any(|event_type| event_type == "flow_started"));
    assert!(event_types
        .iter()
        .any(|event_type| event_type == "flow_finished"));
    let durable_event_types = service
        .list_runtime_events(detail.flow_run.id, 0)
        .await
        .into_iter()
        .map(|event| event.event_type)
        .collect::<Vec<_>>();
    assert!(
        durable_event_types
            .iter()
            .any(|event_type| event_type == "flow_started"),
        "flow lifecycle should be durable: {durable_event_types:?}"
    );
    assert!(
        durable_event_types
            .iter()
            .any(|event_type| event_type == "flow_finished"),
        "flow lifecycle should be durable: {durable_event_types:?}"
    );
    let node_finished_events = stream
        .events()
        .into_iter()
        .filter(|event| event.event_type == "node_finished")
        .collect::<Vec<_>>();
    assert!(!node_finished_events.is_empty());
    for event in node_finished_events {
        assert!(
            event.payload.get("debug_payload").is_none(),
            "runtime stream must not expose persisted debug payload"
        );
        assert_no_pending_debug_ref(&event.payload);
    }
    assert_eq!(
        stream.close_calls(),
        vec![(
            detail.flow_run.id,
            crate::ports::RuntimeEventCloseReason::Finished
        )]
    );
}
