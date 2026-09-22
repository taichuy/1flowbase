use super::*;
use control_plane::ports::{AppendTerminalIfMissingAndCloseOutcome, RuntimeEventSubscription};

fn waiting_run() -> NativeRunResult {
    let mut run = native_run();
    run.status = NativeRunStatus::Waiting;
    run.required_action = Some(NativeRequiredAction {
        action_type: "submit_tool_outputs".into(),
        payload: json!({
            "callback_task_id": Uuid::now_v7(),
            "callback_kind": "llm_tool_calls",
            "node_run_id": Uuid::now_v7(),
            "tool_calls": [{ "id": "call_old", "name": "lookup", "arguments": {} }]
        }),
    });
    run
}

async fn receive_remaining(
    subscription: &mut RuntimeEventSubscription,
) -> Vec<RuntimeEventEnvelope> {
    tokio::time::timeout(Duration::from_secs(1), async {
        let mut events = Vec::new();
        while let Some(event) = subscription.live_events.recv().await {
            events.push(event);
        }
        events
    })
    .await
    .expect("generation must deliver its events and close")
}

#[tokio::test]
async fn late_resume_success_and_error_cannot_close_reopened_generation() {
    for late_error in [false, true] {
        let stream = LocalRuntimeEventStream::new();
        let old_run = waiting_run();
        stream
            .open_run(old_run.id, RuntimeEventStreamPolicy::debug_default())
            .await
            .unwrap();
        let mut old = stream.subscribe(old_run.id, None).await.unwrap();
        // Native persistence wins first; the adapter has not returned yet.
        let old_terminal = terminal_runtime_event_from_native_run(&old_run).unwrap();
        let old_payload = RuntimeEventPayload {
            event_type: old_terminal.event_type,
            source: old_terminal.source,
            durability: old_terminal.durability,
            persist_required: old_terminal.persist_required,
            trace_visible: old_terminal.trace_visible,
            payload: old_terminal.payload,
        };
        stream
            .append_terminal_if_missing_and_close(old_run.id, old_payload.clone())
            .await
            .unwrap();
        let old_closure = *old.closure.borrow();
        stream
            .open_run(old_run.id, RuntimeEventStreamPolicy::debug_default())
            .await
            .unwrap();
        let mut next = stream.subscribe(old_run.id, None).await.unwrap();
        stream
            .append(old_run.id, debug_stream_events::flow_started(old_run.id))
            .await
            .unwrap();
        let before = stream.replay(old_run.id, None, usize::MAX).await.unwrap();

        if late_error {
            // Same capability and payload construction as the resume error branch.
            assert_eq!(
                old.terminal_writer
                    .append_terminal_if_missing_and_close(debug_stream_events::flow_failed(
                        old_run.id,
                        json!({ "message": "late error" })
                    ),)
                    .await
                    .unwrap(),
                AppendTerminalIfMissingAndCloseOutcome::ExistingTerminal
            );
        } else {
            append_compatible_resume_terminal_event(old.terminal_writer.as_ref(), &old_run).await;
        }
        assert_eq!(*old.closure.borrow(), old_closure);
        assert!(
            next.closure.borrow().is_none(),
            "late adapter must not close next round"
        );
        assert_eq!(
            stream.replay(old_run.id, None, usize::MAX).await.unwrap(),
            before
        );
        assert_eq!(
            old.terminal_writer
                .append_terminal_if_missing_and_close(old_payload)
                .await
                .unwrap(),
            AppendTerminalIfMissingAndCloseOutcome::ExistingTerminal
        );
        assert_eq!(
            receive_remaining(&mut old).await.len(),
            1,
            "old terminal is idempotent"
        );

        stream
            .append(
                old_run.id,
                debug_stream_events::answer_text_delta(
                    "assistant",
                    "next round text".into(),
                    0,
                    None,
                    None,
                    None,
                ),
            )
            .await
            .unwrap();
        let mut next_run = old_run.clone();
        next_run.status = NativeRunStatus::Succeeded;
        next_run.required_action = None;
        append_compatible_resume_terminal_event(next.terminal_writer.as_ref(), &next_run).await;
        let delivered = receive_remaining(&mut next).await;
        assert_eq!(
            delivered
                .iter()
                .map(|event| event.event_type.as_str())
                .collect::<Vec<_>>(),
            vec!["flow_started", "text_delta", "flow_finished"]
        );
        assert_eq!(delivered[1].text.as_deref(), Some("next round text"));
        assert_eq!(
            delivered
                .iter()
                .map(|event| event.sequence)
                .collect::<Vec<_>>(),
            vec![1, 2, 3]
        );
        assert_eq!(
            next.closure.borrow().unwrap().reason,
            RuntimeEventCloseReason::Finished
        );
    }
}

#[tokio::test]
async fn resume_terminal_fallback_closes_own_generation_without_native_consumer() {
    // Partial/idempotent callbacks can return a waiting result without native event publication.
    let stream = LocalRuntimeEventStream::new();
    let run = waiting_run();
    stream
        .open_run(run.id, RuntimeEventStreamPolicy::debug_default())
        .await
        .unwrap();
    let subscription = stream.subscribe(run.id, None).await.unwrap();
    let writer = subscription.terminal_writer.clone();
    drop(subscription);
    append_compatible_resume_terminal_event(writer.as_ref(), &run).await;
    append_compatible_resume_terminal_event(writer.as_ref(), &run).await;
    let closed = stream.subscribe(run.id, None).await.unwrap();
    assert_eq!(closed.replay.len(), 1);
    assert_eq!(closed.replay[0].event_type, "waiting_callback");
    assert_eq!(
        closed.replay[0].payload["callback_task_id"],
        run.required_action.as_ref().unwrap().payload["callback_task_id"]
    );
    assert_eq!(
        closed.closure.borrow().unwrap().reason,
        RuntimeEventCloseReason::WaitingCallback
    );
}

#[tokio::test]
async fn resume_error_fallback_closes_own_generation() {
    let stream = LocalRuntimeEventStream::new();
    let run = native_run();
    stream
        .open_run(run.id, RuntimeEventStreamPolicy::debug_default())
        .await
        .unwrap();
    let mut subscription = stream.subscribe(run.id, None).await.unwrap();
    assert_eq!(
        subscription
            .terminal_writer
            .append_terminal_if_missing_and_close(debug_stream_events::flow_failed(
                run.id,
                json!({ "message": "resume failed" })
            ),)
            .await
            .unwrap(),
        AppendTerminalIfMissingAndCloseOutcome::Appended
    );
    let events = receive_remaining(&mut subscription).await;
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].event_type, "flow_failed");
    assert_eq!(
        subscription.closure.borrow().unwrap().reason,
        RuntimeEventCloseReason::Failed
    );
}

#[tokio::test]
async fn closed_generation_without_terminal_cannot_write_into_replacement() {
    let stream = LocalRuntimeEventStream::new();
    let run = native_run();
    stream
        .open_run(run.id, RuntimeEventStreamPolicy::debug_default())
        .await
        .unwrap();
    let old = stream.subscribe(run.id, None).await.unwrap();
    stream
        .close_run(run.id, RuntimeEventCloseReason::Failed)
        .await
        .unwrap();
    stream
        .open_run(run.id, RuntimeEventStreamPolicy::debug_default())
        .await
        .unwrap();
    let next = stream.subscribe(run.id, None).await.unwrap();
    assert!(old
        .terminal_writer
        .append_terminal_if_missing_and_close(debug_stream_events::flow_failed(
            run.id,
            json!({ "message": "late failure" })
        ),)
        .await
        .is_err());
    assert!(next.closure.borrow().is_none());
    assert!(stream
        .replay(run.id, None, usize::MAX)
        .await
        .unwrap()
        .is_empty());
    next.terminal_writer
        .append_terminal_if_missing_and_close(debug_stream_events::flow_finished(run.id, json!({})))
        .await
        .unwrap();
}
