use super::*;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use crate::_tests::RecordingRuntimeEventStream;
use crate::orchestration_runtime::{
    debug_stream_events, test_support::InMemoryOrchestrationRuntimeRepository,
};
use crate::ports::{AppendTerminalIfMissingAndCloseOutcome, RuntimeEventSource};
use serde_json::json;

struct GenerationWriter {
    id: Uuid,
    failed: AtomicBool,
}

impl GenerationWriter {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            id: Uuid::now_v7(),
            failed: AtomicBool::new(false),
        })
    }
}

#[async_trait::async_trait]
impl RuntimeEventTerminalWriter for GenerationWriter {
    fn generation_id(&self) -> Option<Uuid> {
        Some(self.id)
    }

    fn record_persistence_failure(&self) {
        self.failed.store(true, Ordering::SeqCst);
    }

    async fn confirm_terminal_persisted(&self, _sequence: i64) -> Result<()> {
        anyhow::ensure!(
            !self.failed.load(Ordering::SeqCst),
            "sticky persistence failure"
        );
        Ok(())
    }

    async fn append_terminal_if_missing_and_close(
        &self,
        _event: RuntimeEventPayload,
    ) -> Result<AppendTerminalIfMissingAndCloseOutcome> {
        anyhow::bail!("observation forwarder cannot claim terminal authority")
    }
}

fn usage(node_run_id: Uuid) -> RuntimeEventPayload {
    debug_stream_events::usage_snapshot(
        "model",
        node_run_id,
        &plugin_framework::provider_contract::ProviderUsage {
            input_tokens: Some(13),
            output_tokens: Some(7),
            total_tokens: Some(20),
            ..Default::default()
        },
    )
}

#[tokio::test]
async fn required_observation_owner_waits_for_actual_commit_and_preserves_generation_sequence() {
    let repository = InMemoryOrchestrationRuntimeRepository::with_permissions(vec![]);
    let stream = Arc::new(RecordingRuntimeEventStream::default());
    let run = Uuid::now_v7();
    let node = Uuid::now_v7();
    let writer = GenerationWriter::new();
    // Deliberately offset the local stream sequence from the durable sequence.
    let mut earlier = usage(node);
    earlier.persist_required = false;
    stream.append(run, earlier).await.unwrap();
    let (entered, release) = repository.gate_next_runtime_event_append();
    let owner = tokio::spawn({
        let repository = repository.clone();
        let stream = stream.clone();
        let writer = writer.clone();
        async move {
            forward_required_runtime_event(
                &repository,
                stream.as_ref(),
                run,
                usage(node),
                Some(writer.as_ref()),
            )
            .await
        }
    });
    tokio::time::timeout(std::time::Duration::from_secs(1), entered.notified())
        .await
        .unwrap();
    assert!(
        !owner.is_finished(),
        "required owner returned before durable append completed"
    );
    assert!(repository
        .list_runtime_events(run, 0)
        .await
        .unwrap()
        .is_empty());
    assert_eq!(stream.events().len(), 2);
    assert!(stream.events().iter().all(|event| !event.persist_required));
    release.notify_one();
    let envelope = owner.await.unwrap().unwrap();
    let rows = repository.list_runtime_events(run, 0).await.unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].event_type, "usage_snapshot");
    assert_eq!(rows[0].node_run_id, Some(node));
    assert_eq!(rows[0].payload["usage"], usage(node).payload["usage"]);
    assert_eq!(rows[0].sequence, 1);
    for field in ["stream_sequence", "sequence_start", "sequence_end"] {
        assert_eq!(rows[0].payload[field], json!(envelope.sequence));
    }
    assert_eq!(envelope.sequence, 2);
    assert_eq!(rows[0].payload["stream_generation_id"], json!(writer.id));
    crate::orchestration_runtime::runtime_event_persister::persist_runtime_debug_stream_events(
        &repository,
        stream.events(),
    )
    .await
    .unwrap();
    assert_eq!(
        repository.list_runtime_events(run, 0).await.unwrap().len(),
        1,
        "async persister duplicated the observation"
    );
    assert!(!writer.failed.load(Ordering::SeqCst));
}

#[tokio::test]
async fn required_observation_failure_waits_for_write_and_remains_sticky_after_later_success() {
    let repository = InMemoryOrchestrationRuntimeRepository::with_permissions(vec![]);
    let stream = Arc::new(RecordingRuntimeEventStream::default());
    let run = Uuid::now_v7();
    let node = Uuid::now_v7();
    let writer = GenerationWriter::new();
    let (entered, release) = repository.gate_next_runtime_event_append();
    repository.fail_next_runtime_event_append();
    let owner = tokio::spawn({
        let repository = repository.clone();
        let stream = stream.clone();
        let writer = writer.clone();
        async move {
            forward_required_runtime_event(
                &repository,
                stream.as_ref(),
                run,
                usage(node),
                Some(writer.as_ref()),
            )
            .await
        }
    });
    tokio::time::timeout(std::time::Duration::from_secs(1), entered.notified())
        .await
        .unwrap();
    assert!(!owner.is_finished());
    assert!(!writer.failed.load(Ordering::SeqCst));
    release.notify_one();
    let error = owner.await.unwrap().unwrap_err();
    assert_eq!(error.to_string(), "simulated runtime event append failure");
    assert!(repository
        .list_runtime_events(run, 0)
        .await
        .unwrap()
        .is_empty());
    assert!(writer.failed.load(Ordering::SeqCst));
    forward_required_runtime_event(
        &repository,
        stream.as_ref(),
        run,
        usage(node),
        Some(writer.as_ref()),
    )
    .await
    .unwrap();
    assert_eq!(
        repository.list_runtime_events(run, 0).await.unwrap().len(),
        1
    );
    assert!(writer.confirm_terminal_persisted(2).await.is_err());
    crate::orchestration_runtime::runtime_event_persister::persist_runtime_debug_stream_events(
        &repository,
        stream.events(),
    )
    .await
    .unwrap();
    assert_eq!(
        repository.list_runtime_events(run, 0).await.unwrap().len(),
        1,
        "failed observation must not move to async retry"
    );
}

#[tokio::test]
async fn excluded_native_items_and_delta_facts_do_not_consume_a_repository_write() {
    let repository = InMemoryOrchestrationRuntimeRepository::with_permissions(vec![]);
    let stream = RecordingRuntimeEventStream::default();
    let run = Uuid::now_v7();
    let node = Uuid::now_v7();
    let writer = GenerationWriter::new();
    repository.fail_next_runtime_event_append();
    let mut native_done =
        debug_stream_events::provider_output_item_done("model", node, 0, json!({"type":"message"}));
    // This is the copy published after the native output's existing owner commits it.
    native_done.persist_required = false;
    let mut excluded_usage = usage(node);
    excluded_usage.persist_required = false;
    let mut events = vec![
        native_done,
        excluded_usage,
        debug_stream_events::provider_output_item_added("model", node, 0, json!({})),
        debug_stream_events::reasoning_signature_delta("model", node, "signature".into()),
        debug_stream_events::provider_responses_output_delta("model", node, json!({})),
        debug_stream_events::provider_native_event(
            "model",
            node,
            "openai.responses".into(),
            json!({}),
        ),
    ];
    for event_type in [
        "text_delta",
        "reasoning_delta",
        "tool_call_delta",
        "mcp_call_delta",
    ] {
        events.push(RuntimeEventPayload {
            event_type: event_type.into(),
            source: RuntimeEventSource::Provider,
            durability: crate::ports::RuntimeEventDurability::Ephemeral,
            persist_required: true,
            trace_visible: false,
            payload: json!({"type": event_type, "delta": "fragment"}),
        });
    }
    for event in events {
        forward_required_runtime_event(&repository, &stream, run, event, Some(writer.as_ref()))
            .await
            .unwrap();
    }
    assert!(repository
        .list_runtime_events(run, 0)
        .await
        .unwrap()
        .is_empty());
    assert!(!writer.failed.load(Ordering::SeqCst));
    crate::orchestration_runtime::runtime_event_persister::persist_runtime_debug_stream_events(
        &repository,
        stream.events(),
    )
    .await
    .unwrap();
    let error = forward_required_runtime_event(
        &repository,
        &stream,
        run,
        usage(node),
        Some(writer.as_ref()),
    )
    .await
    .unwrap_err();
    assert_eq!(
        error.to_string(),
        "simulated runtime event append failure",
        "excluded events consumed the pending repository failure"
    );
}
