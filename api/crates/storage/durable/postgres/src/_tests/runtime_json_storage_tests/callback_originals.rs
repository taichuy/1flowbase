use super::*;

#[tokio::test]
async fn runtime_json_tool_arguments_survive_callback_privacy_and_stream_reads() {
    let (store, run, app, scope) = seed().await;
    let calls = json!([{
        "id": "call_original", "name": "fixture",
        "arguments": {
            "text": "before\0after", "literal": "before\\u0000after",
            "key\0tail": "real key", "key\\u0000tail": "literal key"
        }
    }]);
    let request = json!({
        "tool_calls": calls,
        "provider_metadata": {"private": "provider\0metadata"},
        "internal_context": "must stay private"
    });
    let external = json!({"private": "external\0context"});
    let summary = json!({"tool_calls": calls});
    let node = store
        .create_node_run(&CreateNodeRunInput {
            flow_run_id: run,
            node_id: "tool-original".into(),
            node_type: "llm".into(),
            node_alias: "Original tool".into(),
            status: domain::NodeRunStatus::WaitingCallback,
            input_payload: json!({}),
            debug_payload: json!({}),
            started_at: OffsetDateTime::now_utc(),
        })
        .await
        .unwrap();
    store
        .create_checkpoint(&CreateCheckpointInput {
            flow_run_id: run,
            node_run_id: Some(node.id),
            status: "waiting".into(),
            reason: "callback".into(),
            locator_payload: json!({"node_id":"tool-original"}),
            variable_snapshot: json!({}),
            external_ref_payload: None,
        })
        .await
        .unwrap();
    let callback = store
        .create_callback_task(&CreateCallbackTaskInput {
            flow_run_id: run,
            node_run_id: node.id,
            callback_kind: "llm_tool_calls".into(),
            request_payload: request.clone(),
            external_ref_payload: Some(external.clone()),
        })
        .await
        .unwrap();
    assert_eq!(callback.request_payload, summary);
    assert_eq!(callback.external_ref_payload, None);

    // The unrestricted repository read still owns the entire persisted original.
    let full = store.get_callback_task(callback.id).await.unwrap().unwrap();
    assert_eq!(full.request_payload, request);
    assert_eq!(full.external_ref_payload, Some(external.clone()));
    let published = store
        .get_published_callback_task(callback.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(published.request_payload, summary);
    assert_eq!(published.external_ref_payload, None);
    let context = store
        .get_callback_resume_context(app, callback.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(context.callback_task.request_payload, summary);
    assert_eq!(context.callback_task.external_ref_payload, None);
    let pending = store
        .get_published_run_stream_state(app, run)
        .await
        .unwrap()
        .unwrap()
        .latest_pending_callback
        .unwrap();
    assert_eq!(pending.tool_calls, Some(calls.clone()));
    assert_eq!(pending.request_payload, None);

    let complete = CompleteCallbackTaskInput {
        callback_task_id: callback.id,
        response_payload: json!({"tool_results":[{"tool_call_id":"call_original","output":"done\0exact"}]}),
        completed_at: OffsetDateTime::now_utc(),
    };
    for _ in 0..2 {
        let completed = store.complete_callback_task(&complete).await.unwrap();
        assert_eq!(completed.request_payload, summary);
        assert_eq!(completed.external_ref_payload, None);
        assert_eq!(
            completed.response_payload,
            Some(complete.response_payload.clone())
        );
    }

    let waiting_callback_id = Uuid::now_v7();
    let waiting = store
        .persist_waiting_state(&PersistWaitingStateInput {
            checkpoint_id: Uuid::now_v7(),
            scope_id: scope,
            application_id: app,
            flow_run_id: run,
            node_run_id: node.id,
            expected_status: domain::FlowRunStatus::Running,
            output_payload: json!({}),
            checkpoint_status: "waiting".into(),
            checkpoint_reason: "callback".into(),
            locator_payload: json!({"node_id":"tool-original","next_node_index":0}),
            variable_snapshot: json!({}),
            checkpoint_external_ref_payload: None,
            context_content: json!({"tool_arguments": calls}),
            parent_context_version_id: None,
            context_transition_kind: domain::ContextTransitionKind::Initial,
            recovery_idempotency_key: "nul-tool-waiting".into(),
            resume_claim_id: None,
            resume_claim_token: None,
            waiting_event: AppendRuntimeEventInput {
                flow_run_id: run,
                node_run_id: Some(node.id),
                span_id: None,
                parent_span_id: None,
                event_type: "waiting_callback".into(),
                layer: domain::RuntimeEventLayer::AgentTransition,
                source: domain::RuntimeEventSource::Host,
                trust_level: domain::RuntimeTrustLevel::HostFact,
                item_id: None,
                ledger_ref: None,
                payload: json!({"text":"waiting\0exact"}),
                visibility: domain::RuntimeEventVisibility::User,
                durability: domain::RuntimeEventDurability::Durable,
            },
            tool_delivery_events: vec![],
            kind: PersistWaitingKind::Callback(PersistWaitingCallbackTaskInput {
                id: waiting_callback_id,
                callback_kind: "llm_tool_calls".into(),
                request_payload: request.clone(),
                external_ref_payload: Some(external),
            }),
        })
        .await
        .unwrap()
        .unwrap();
    let waiting_callback = waiting.callback_task.unwrap();
    assert_eq!(waiting_callback.request_payload, summary);
    assert_eq!(waiting_callback.external_ref_payload, None);
    let waiting_context = store
        .get_callback_resume_context(app, waiting_callback_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(waiting_context.callback_task.request_payload, summary);
    let pending = store
        .get_published_run_stream_state(app, run)
        .await
        .unwrap()
        .unwrap()
        .latest_pending_callback
        .unwrap();
    assert_eq!(pending.id, waiting_callback_id);
    assert_eq!(pending.tool_calls, Some(calls));
    assert_eq!(pending.request_payload, None);
    let full = store
        .get_callback_task(waiting_callback_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(full.request_payload, request);
}
