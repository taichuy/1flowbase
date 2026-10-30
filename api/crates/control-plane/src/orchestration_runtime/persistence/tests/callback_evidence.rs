use super::*;

// AC-002/005: exercise the production waiting-state writer with one reused node row.
// The repository fixture enforces the same recovery-key uniqueness as PostgreSQL.
#[tokio::test]
async fn native_reused_node_persists_distinct_callback_wait_occurrences() {
    use crate::orchestration_runtime::{OrchestrationRuntimeService, StartFlowDebugRunCommand};
    use crate::ports::{CreateNodeRunInput, OrchestrationRuntimeRepository};
    use orchestration_runtime::execution_state::{CheckpointSnapshot, PendingCallbackTask};
    let service = OrchestrationRuntimeService::for_tests();
    let seeded = service
        .seed_application_with_flow("native wait persistence")
        .await;
    let started = service
        .start_flow_debug_run(StartFlowDebugRunCommand {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            input_payload: json!({"node-start":{"query":"test"}}),
            document_snapshot: None,
            debug_session_id: None,
        })
        .await
        .unwrap();
    let mut run = started.flow_run;
    let node = service
        .repository
        .create_node_run(&CreateNodeRunInput {
            flow_run_id: run.id,
            node_id: "node-llm".into(),
            node_type: "llm".into(),
            node_alias: "LLM".into(),
            status: domain::NodeRunStatus::Running,
            input_payload: json!({}),
            debug_payload: json!({}),
            started_at: OffsetDateTime::now_utc(),
        })
        .await
        .unwrap();
    for index in 0..3 {
        let payload =
            json!({"tool_calls":[{"id":format!("call-{index}"),"name":"read","arguments":{}}]});
        let mut node_trace = trace(&node.node_id, payload.clone(), None);
        node_trace.metrics_payload = json!({"usage":{"input_tokens":(index+1)*11,"output_tokens":(index+1)*3,"input_cache_hit_tokens":index*2}});
        let outcome = FlowDebugExecutionOutcome {
            stop_reason: ExecutionStopReason::WaitingCallback(PendingCallbackTask {
                node_id: node.node_id.clone(),
                node_alias: "LLM".into(),
                callback_kind: "llm_tool_calls".into(),
                request_payload: payload.clone(),
            }),
            variable_pool: Map::new(),
            checkpoint_snapshot: Some(CheckpointSnapshot {
                container_state: None,
                next_node_index: 0,
                variable_pool: Map::new(),
                active_node_ids: vec![node.node_id.clone()],
            }),
            operation_terminal: None,
            node_traces: vec![node_trace],
        };
        let prepared = [(node.node_id.clone(), node.clone())].into_iter().collect();
        run = super::super::persist_flow_debug_outcome(
            &service.repository,
            super::super::PersistFlowDebugOutcomeInput {
                responses_round: None,
                scope_id: Uuid::nil(),
                application_name: "native wait persistence",
                task_queue: None,
                application_id: seeded.application_id,
                flow_run: &run,
                compiled_plan: None,
                outcome: &outcome,
                prepared_node_runs: Some(&prepared),
                answer_presentation: None,
                trigger_event_type: "fixture",
                trigger_event_payload: json!({}),
                base_started_at: OffsetDateTime::now_utc(),
                waiting_node_resume: None,
                resume_claim_id: None,
                resume_claim_token: None,
                tool_delivery_events: Vec::new(),
            },
        )
        .await
        .unwrap()
        .flow_run;
    }
    let detail = service
        .repository
        .get_application_run_detail(seeded.application_id, run.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        detail.flow_run.status,
        domain::FlowRunStatus::WaitingCallback
    );
    assert_eq!(detail.node_runs.len(), 1);
    assert_eq!(detail.callback_tasks.len(), 3);
    assert_eq!(detail.checkpoints.len(), 3);
    for task in &detail.callback_tasks {
        let call = task.request_payload["tool_calls"][0]["id"]
            .as_str()
            .unwrap();
        let index: u64 = call.strip_prefix("call-").unwrap().parse().unwrap();
        let baseline = &task.request_payload["native_usage_baseline"];
        assert_eq!(baseline["prompt_tokens"], (index + 1) * 11);
        assert_eq!(baseline["completion_tokens"], (index + 1) * 3);
        assert_eq!(baseline["total_tokens"], (index + 1) * 14);
        assert_eq!(baseline["input_cache_hit_tokens"], index * 2);
    }

    assert!(detail
        .callback_tasks
        .iter()
        .all(|task| task.node_run_id == node.id));
}

// A provider fault after the tool call was generated must persist as a plain
// failed run; the buffered tool candidate was never committed or delivered.
#[tokio::test]
async fn provider_fault_after_tool_call_discards_uncommitted_candidates() {
    use crate::orchestration_runtime::{OrchestrationRuntimeService, StartFlowDebugRunCommand};
    use crate::ports::{CreateNodeRunInput, OrchestrationRuntimeRepository};
    use orchestration_runtime::execution_state::NodeExecutionFailure;
    let service = OrchestrationRuntimeService::for_tests();
    let seeded = service
        .seed_application_with_flow("provider fault after tool call")
        .await;
    let started = service
        .start_flow_debug_run(StartFlowDebugRunCommand {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            input_payload: json!({"node-start":{"query":"test"}}),
            document_snapshot: None,
            debug_session_id: None,
        })
        .await
        .unwrap();
    let run = started.flow_run;
    let node = service
        .repository
        .create_node_run(&CreateNodeRunInput {
            flow_run_id: run.id,
            node_id: "node-llm".into(),
            node_type: "llm".into(),
            node_alias: "LLM".into(),
            status: domain::NodeRunStatus::Running,
            input_payload: json!({}),
            debug_payload: json!({}),
            started_at: OffsetDateTime::now_utc(),
        })
        .await
        .unwrap();
    let error_payload = json!({"message":"provider_transport_receipt_invalid"});
    let outcome = FlowDebugExecutionOutcome {
        stop_reason: ExecutionStopReason::Failed(NodeExecutionFailure {
            node_id: node.node_id.clone(),
            node_alias: "LLM".into(),
            error_payload: error_payload.clone(),
        }),
        variable_pool: Map::new(),
        checkpoint_snapshot: None,
        operation_terminal: None,
        node_traces: vec![trace(&node.node_id, json!({}), Some(error_payload.clone()))],
    };
    let prepared = [(node.node_id.clone(), node.clone())].into_iter().collect();
    let candidate = crate::orchestration_runtime::debug_stream_events::provider_output_item_done(
        &node.node_id,
        node.id,
        0,
        json!({"type":"custom_tool_call","call_id":"call-lost","name":"exec","input":"pwd"}),
    );
    let persisted = super::super::persist_flow_debug_outcome(
        &service.repository,
        super::super::PersistFlowDebugOutcomeInput {
            responses_round: None,
            scope_id: Uuid::nil(),
            application_name: "provider fault after tool call",
            task_queue: None,
            application_id: seeded.application_id,
            flow_run: &run,
            compiled_plan: None,
            outcome: &outcome,
            prepared_node_runs: Some(&prepared),
            answer_presentation: None,
            trigger_event_type: "fixture",
            trigger_event_payload: json!({}),
            base_started_at: OffsetDateTime::now_utc(),
            waiting_node_resume: None,
            resume_claim_id: None,
            resume_claim_token: None,
            tool_delivery_events: vec![candidate],
        },
    )
    .await
    .expect("a provider fault persists as a failed run, not an invariant error");
    assert_eq!(persisted.flow_run.status, domain::FlowRunStatus::Failed);
    assert_eq!(
        persisted
            .flow_run
            .error_payload
            .as_ref()
            .map(|e| e["message"].clone()),
        Some(json!("provider_transport_receipt_invalid"))
    );
    assert!(
        !persisted
            .stream_events
            .iter()
            .any(|event| event.event_type == "provider_output_item_done"),
        "an uncommitted tool candidate must not be streamed"
    );
    let detail = service
        .repository
        .get_application_run_detail(seeded.application_id, run.id)
        .await
        .unwrap()
        .unwrap();
    assert!(detail.callback_tasks.is_empty());
}

#[tokio::test]
async fn semantic_round_evidence_is_atomic_with_callback_and_checkpoint() {
    use crate::orchestration_runtime::{OrchestrationRuntimeService, StartFlowDebugRunCommand};
    use crate::ports::{CreateNodeRunInput, OrchestrationRuntimeRepository};
    use orchestration_runtime::execution_state::{CheckpointSnapshot, PendingCallbackTask};
    let service = OrchestrationRuntimeService::for_tests();
    let seeded = service
        .seed_application_with_flow("semantic wait evidence")
        .await;
    let started = service
        .start_flow_debug_run(StartFlowDebugRunCommand {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            input_payload: json!({"node-start":{"query":"test"}}),
            document_snapshot: None,
            debug_session_id: None,
        })
        .await
        .unwrap();
    let mut run = started.flow_run;
    let node = service
        .repository
        .create_node_run(&CreateNodeRunInput {
            flow_run_id: run.id,
            node_id: "node-llm".into(),
            node_type: "llm".into(),
            node_alias: "LLM".into(),
            status: domain::NodeRunStatus::Running,
            input_payload: json!({}),
            debug_payload: json!({}),
            started_at: OffsetDateTime::now_utc(),
        })
        .await
        .unwrap();
    let prefix = crate::application_public_api::compat::openai::history::completed_history(
        &json!({"input":"work"}),
        None,
        &[],
    )
    .unwrap()
    .unwrap();
    let round_id = Uuid::now_v7();
    for index in 0..2 {
        let payload =
            json!({"tool_calls":[{"id":format!("call-{index}"),"name":"read","arguments":{}}]});
        let outcome = FlowDebugExecutionOutcome {
            stop_reason: ExecutionStopReason::WaitingCallback(PendingCallbackTask {
                node_id: node.node_id.clone(),
                node_alias: "LLM".into(),
                callback_kind: "llm_tool_calls".into(),
                request_payload: payload.clone(),
            }),
            variable_pool: Map::new(),
            checkpoint_snapshot: Some(CheckpointSnapshot {
                container_state: None,
                next_node_index: 0,
                variable_pool: [(node.node_id.clone(), json!({"__llm_tool_callback": {"pending_tool_calls":payload["tool_calls"],"history":[],"system":[]}}))].into_iter().collect(),
                active_node_ids: vec![node.node_id.clone()],
            }),
            operation_terminal: None,
            node_traces: vec![trace(&node.node_id, payload, None)],
        };
        let prepared = [(node.node_id.clone(), node.clone())].into_iter().collect();
        run = super::super::persist_flow_debug_outcome(
            &service.repository,
            super::super::PersistFlowDebugOutcomeInput {
                responses_round: Some((if index == 0 { run.id } else { round_id }, prefix.clone())),
                scope_id: Uuid::nil(),
                application_name: "semantic wait evidence",
                task_queue: None,
                application_id: seeded.application_id,
                flow_run: &run,
                compiled_plan: None,
                outcome: &outcome,
                prepared_node_runs: Some(&prepared),
                answer_presentation: None,
                trigger_event_type: "fixture",
                trigger_event_payload: json!({}),
                base_started_at: OffsetDateTime::now_utc(),
                waiting_node_resume: None,
                resume_claim_id: None,
                resume_claim_token: None,
                tool_delivery_events: Vec::new(),
            },
        )
        .await
        .unwrap()
        .flow_run;
    }
    let detail = service
        .repository
        .get_application_run_detail(seeded.application_id, run.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        detail.flow_run.status,
        domain::FlowRunStatus::WaitingCallback
    );
    assert_eq!(detail.node_runs.len(), 1);
    let ids = detail
        .callback_tasks
        .iter()
        .map(|task| {
            task.request_payload["responses_round"]["response_id"]
                .as_str()
                .unwrap()
        })
        .collect::<std::collections::HashSet<_>>();
    assert_eq!(ids.len(), 2);
    for task in &detail.callback_tasks {
        let round = &task.request_payload["responses_round"];
        assert!(round["history"].is_object());
        assert_eq!(
            round["output"][0]["call_id"],
            task.request_payload["tool_calls"][0]["id"]
        );
        assert!(task.request_payload.get("native_response").is_none());
    }
    for checkpoint in &detail.checkpoints {
        let snapshot = crate::orchestration_runtime::checkpoint_snapshot_from_record_with_context(
            &service.repository,
            checkpoint,
            run.id,
        )
        .await
        .unwrap();
        let evidence =
            &snapshot.variable_pool[&node.node_id]["__llm_tool_callback"]["responses_round"];
        assert!(detail
            .callback_tasks
            .iter()
            .any(|task| task.request_payload["responses_round"] == *evidence));
    }
    assert_eq!(detail.callback_tasks.len(), 2);
    assert_eq!(detail.checkpoints.len(), 2);
    assert!(detail
        .callback_tasks
        .iter()
        .all(|task| task.node_run_id == node.id));
}
