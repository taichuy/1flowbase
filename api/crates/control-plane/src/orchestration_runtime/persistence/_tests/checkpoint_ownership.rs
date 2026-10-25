use super::super::{
    checkpoint_locator::RECOVERY_CONTEXT_MARKER, checkpoint_snapshot_from_record_with_context,
    prepare_recovery_checkpoint,
};
use crate::{
    orchestration_runtime::OrchestrationRuntimeService,
    ports::{
        AppendContextVersionInput, OrchestrationRuntimeRepository, PutCanonicalRuntimeContentInput,
    },
};
use serde_json::{json, Value};
use uuid::Uuid;

#[tokio::test]
async fn successor_checkpoint_re_roots_materialized_lineage_without_losing_frozen_values() {
    let service = OrchestrationRuntimeService::for_tests();
    let seeded = service
        .seed_waiting_callback_run("Checkpoint ownership")
        .await;
    let context = service
        .repository
        .get_callback_resume_context(seeded.application_id, seeded.callback_task_id)
        .await
        .unwrap()
        .unwrap();
    let mut checkpoint = context.checkpoint;
    let parent = Uuid::parse_str(
        checkpoint.locator_payload["context_version_id"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    // Exercise a sparse lineage, not only a self-contained checkpoint.
    let content = service
        .repository
        .put_canonical_runtime_content(&PutCanonicalRuntimeContentInput {
            scope_id: Uuid::nil(),
            application_id: seeded.application_id,
            content: json!({"format":"runtime_delta_v1", "set":{
            "env":{"frozen":"predecessor environment"},
            "sys":{"native_inference_recovery":{"failed_flow_run_id": seeded.flow_run_id}},
            "node-upstream":{"side_effect_receipt":"already committed"}
        }, "remove":[]}),
        })
        .await
        .unwrap();
    let delta = service
        .repository
        .append_context_version(&AppendContextVersionInput {
            scope_id: Uuid::nil(),
            application_id: seeded.application_id,
            flow_run_id: seeded.flow_run_id,
            parent_context_version_id: Some(parent),
            sequence: 1,
            transition_kind: domain::ContextTransitionKind::Callback,
            transition_actor: domain::ContextTransitionActor::Host,
            declared_compaction_provenance: None,
            actual_content_id: content.id,
        })
        .await
        .unwrap();
    checkpoint.locator_payload["context_version_id"] = json!(delta.id);
    let same_run = checkpoint_snapshot_from_record_with_context(
        &service.repository,
        &checkpoint,
        seeded.flow_run_id,
    )
    .await
    .unwrap();
    let same_prepared = prepare_recovery_checkpoint(&service.repository, "node-tool", &same_run)
        .await
        .unwrap();
    assert_eq!(same_prepared.parent_context_version_id, Some(delta.id));
    assert_eq!(same_prepared.context_content["format"], "runtime_delta_v1");
    assert_eq!(
        same_run.variable_pool["env"]["frozen"],
        "predecessor environment"
    );
    assert_eq!(
        same_run.variable_pool["node-start"]["query"], "order_123",
        "the original frozen input must survive the sparse delta"
    );

    let successor = checkpoint_snapshot_from_record_with_context(
        &service.repository,
        &checkpoint,
        Uuid::now_v7(),
    )
    .await
    .unwrap();
    let mut frozen_values = same_run.variable_pool.clone();
    frozen_values.remove(RECOVERY_CONTEXT_MARKER);
    assert_eq!(successor.variable_pool, frozen_values);
    assert_eq!(successor.next_node_index, same_run.next_node_index);
    assert_eq!(successor.active_node_ids, same_run.active_node_ids);
    let prepared = prepare_recovery_checkpoint(&service.repository, "node-tool", &successor)
        .await
        .unwrap();
    assert_eq!(
        prepared.parent_context_version_id, None,
        "a successor cannot reference another run's projection"
    );
    assert_eq!(prepared.context_content["format"], "runtime_snapshot_v1");
    assert_eq!(
        prepared.context_content["variable_pool"],
        Value::Object(frozen_values)
    );
    assert_eq!(
        prepared.variable_snapshot[RECOVERY_CONTEXT_MARKER]["sequence"],
        0
    );
}

#[tokio::test]
async fn successor_checkpoint_re_roots_legacy_inline_snapshot_too() {
    let service = OrchestrationRuntimeService::for_tests();
    let source_id = Uuid::now_v7();
    let mut checkpoint = super::checkpoint_record(
        json!({"node_id":"node-tool", "next_node_index":1, "active_node_ids":["node-tool"]}),
        json!({"env":{"frozen":true}, "node-tool":{"history":[1,2]},
            RECOVERY_CONTEXT_MARKER:{"context_version_id":Uuid::now_v7(), "sequence":9}}),
    );
    checkpoint.flow_run_id = source_id;
    let same_run =
        checkpoint_snapshot_from_record_with_context(&service.repository, &checkpoint, source_id)
            .await
            .unwrap();
    assert_eq!(
        Value::Object(same_run.variable_pool),
        checkpoint.variable_snapshot
    );
    let successor = checkpoint_snapshot_from_record_with_context(
        &service.repository,
        &checkpoint,
        Uuid::now_v7(),
    )
    .await
    .unwrap();
    let mut expected = checkpoint.variable_snapshot.as_object().unwrap().clone();
    expected.remove(RECOVERY_CONTEXT_MARKER);
    assert_eq!(successor.variable_pool, expected);
    let prepared = prepare_recovery_checkpoint(&service.repository, "node-tool", &successor)
        .await
        .unwrap();
    assert_eq!(prepared.parent_context_version_id, None);
    assert_eq!(prepared.context_content["format"], "runtime_snapshot_v1");
}

#[tokio::test]
async fn recovery_metadata_rejects_uuid_before_sequence_and_missing_lineage() {
    let service = OrchestrationRuntimeService::for_tests();
    let mut snapshot = orchestration_runtime::execution_state::CheckpointSnapshot {
        next_node_index: 2,
        active_node_ids: vec!["node-tool".into()],
        variable_pool: serde_json::Map::from_iter([(
            RECOVERY_CONTEXT_MARKER.into(),
            json!({"context_version_id":"invalid", "sequence":i64::MAX}),
        )]),
    };
    let original = snapshot.clone();
    let error = prepare_recovery_checkpoint(&service.repository, "node-tool", &snapshot)
        .await
        .err()
        .unwrap();
    assert_eq!(
        error.to_string(),
        "runtime recovery context marker must contain a UUID"
    );
    assert_eq!(snapshot, original);

    snapshot.variable_pool[RECOVERY_CONTEXT_MARKER] =
        json!({"context_version_id":Uuid::nil(), "sequence":0});
    let original = snapshot.clone();
    let error = prepare_recovery_checkpoint(&service.repository, "node-tool", &snapshot)
        .await
        .err()
        .unwrap();
    assert_eq!(error.to_string(), "context version not found");
    assert_eq!(snapshot, original);
}

#[cfg(debug_assertions)]
#[tokio::test]
#[should_panic(expected = "attempt to add with overflow")]
async fn recovery_sequence_overflow_still_precedes_lineage_loading() {
    let service = OrchestrationRuntimeService::for_tests();
    let snapshot = orchestration_runtime::execution_state::CheckpointSnapshot {
        next_node_index: 0,
        active_node_ids: Vec::new(),
        variable_pool: serde_json::Map::from_iter([(
            RECOVERY_CONTEXT_MARKER.into(),
            json!({"context_version_id":Uuid::nil(), "sequence":i64::MAX}),
        )]),
    };
    let _ = prepare_recovery_checkpoint(&service.repository, "node-tool", &snapshot).await;
}

#[tokio::test]
async fn recovery_metadata_keeps_nonstring_marker_defaults_and_negative_sequence() {
    let service = OrchestrationRuntimeService::for_tests();
    for (marker, sequence) in [
        (Value::Null, 0),
        (json!([]), 0),
        (json!({"context_version_id":42, "sequence":"9"}), 0),
        (json!({"context_version_id":null, "sequence":-2}), -1),
        (json!({"sequence":4}), 5),
        (json!({"sequence":u64::MAX}), 0),
    ] {
        let snapshot = orchestration_runtime::execution_state::CheckpointSnapshot {
            next_node_index: 3,
            active_node_ids: vec!["node-tool".into(), "node-answer".into()],
            variable_pool: serde_json::Map::from_iter([
                (RECOVERY_CONTEXT_MARKER.into(), marker),
                ("node-tool".into(), json!({"text":"exact\0value"})),
            ]),
        };
        let original = snapshot.clone();
        let prepared = prepare_recovery_checkpoint(&service.repository, "node-tool", &snapshot)
            .await
            .unwrap();
        assert_eq!(snapshot, original);
        assert_eq!(prepared.parent_context_version_id, None);
        assert_eq!(
            prepared.context_content,
            json!({
                "format":"runtime_snapshot_v1", "variable_pool":{"node-tool":{"text":"exact\0value"}}
            })
        );
        assert_eq!(
            prepared.variable_snapshot,
            json!({RECOVERY_CONTEXT_MARKER:{
                "parent_context_version_id":null, "sequence":sequence
            }})
        );
        assert_eq!(
            prepared.locator_payload,
            json!({
                "node_id":"node-tool", "next_node_index":3, "active_node_ids":["node-tool", "node-answer"]
            })
        );
    }
}

#[tokio::test]
async fn prepared_full_then_history_delta_resumes_exact_snapshot() {
    let service = OrchestrationRuntimeService::for_tests();
    let seeded = service
        .seed_waiting_callback_run("Prepared checkpoint round trip")
        .await;
    let mut snapshot = orchestration_runtime::execution_state::CheckpointSnapshot {
        next_node_index: 3,
        active_node_ids: vec!["node-tool".into(), "node-answer".into()],
        variable_pool: serde_json::Map::from_iter([
            ("obsolete".into(), json!(true)),
            (RECOVERY_CONTEXT_MARKER.into(), Value::Null),
            (
                "node-tool".into(),
                json!({"__llm_tool_callback":{
                    "system":"frozen\0system", "history":[{"role":"user", "content":"query"}],
                    "pending_tool_calls":["call-1"]
                }}),
            ),
        ]),
    };
    let mut parent = None;
    let mut checkpoint = super::checkpoint_record(Value::Null, Value::Null);
    checkpoint.flow_run_id = seeded.flow_run_id;
    for sequence in 0..=1 {
        let original = snapshot.clone();
        let prepared = prepare_recovery_checkpoint(&service.repository, "node-tool", &snapshot)
            .await
            .unwrap();
        assert_eq!(
            snapshot, original,
            "preparation must leave the borrowed snapshot intact"
        );
        assert_eq!(prepared.parent_context_version_id, parent);
        assert_eq!(
            prepared.variable_snapshot[RECOVERY_CONTEXT_MARKER]["sequence"],
            sequence
        );
        if sequence == 0 {
            assert_eq!(prepared.context_content["format"], "runtime_snapshot_v1");
            assert_eq!(serde_json::to_vec(&prepared.context_content).unwrap(),
                br#"{"format":"runtime_snapshot_v1","variable_pool":{"obsolete":true,"node-tool":{"__llm_tool_callback":{"system":"frozen\u0000system","history":[{"role":"user","content":"query"}],"pending_tool_calls":["call-1"]}}}}"#);
            assert!(prepared.context_content["variable_pool"]
                .get(RECOVERY_CONTEXT_MARKER)
                .is_none());
        } else {
            assert_eq!(serde_json::to_vec(&prepared.context_content).unwrap(),
                br#"{"format":"runtime_delta_v1","set":{},"remove":["obsolete"],"llm_callback_appends":[{"node_id":"node-tool","node_set":{},"node_remove":[],"state_set":{"pending_tool_calls":[]},"state_remove":[],"history_append":[{"role":"tool","content":"result\u0000exact"}]}]}"#);
            assert_eq!(prepared.context_content["format"], "runtime_delta_v1");
            assert_eq!(prepared.context_content["remove"], json!(["obsolete"]));
            assert_eq!(
                prepared.context_content["llm_callback_appends"][0]["history_append"],
                json!([{"role":"tool", "content":"result\0exact"}])
            );
            assert!(prepared.context_content["set"]
                .as_object()
                .unwrap()
                .is_empty());
            assert!(!prepared.context_content.to_string().contains("frozen"));
        }
        let content = service
            .repository
            .put_canonical_runtime_content(&PutCanonicalRuntimeContentInput {
                scope_id: Uuid::nil(),
                application_id: seeded.application_id,
                content: prepared.context_content,
            })
            .await
            .unwrap();
        let version = service
            .repository
            .append_context_version(&AppendContextVersionInput {
                scope_id: Uuid::nil(),
                application_id: seeded.application_id,
                flow_run_id: seeded.flow_run_id,
                parent_context_version_id: parent,
                sequence,
                transition_kind: domain::ContextTransitionKind::Callback,
                transition_actor: domain::ContextTransitionActor::Host,
                declared_compaction_provenance: None,
                actual_content_id: content.id,
            })
            .await
            .unwrap();
        parent = Some(version.id);
        checkpoint.locator_payload = prepared.locator_payload;
        checkpoint.locator_payload["context_version_id"] = json!(version.id);
        checkpoint.variable_snapshot = prepared.variable_snapshot;
        let resumed = checkpoint_snapshot_from_record_with_context(
            &service.repository,
            &checkpoint,
            seeded.flow_run_id,
        )
        .await
        .unwrap();
        let mut expected = snapshot.clone();
        expected.variable_pool.insert(
            RECOVERY_CONTEXT_MARKER.into(),
            json!({"context_version_id":version.id, "sequence":sequence}),
        );
        assert_eq!(resumed, expected);
        snapshot = resumed;
        snapshot.variable_pool.remove("obsolete");
        snapshot.variable_pool["node-tool"]["__llm_tool_callback"]["history"]
            .as_array_mut()
            .unwrap()
            .push(json!({"role":"tool", "content":"result\0exact"}));
        snapshot.variable_pool["node-tool"]["__llm_tool_callback"]["pending_tool_calls"] =
            json!([]);
    }
}

#[test]
fn compact_delta_moves_changed_values_in_order_without_recovery_marker() {
    let previous = json!({
        RECOVERY_CONTEXT_MARKER: {"context_version_id":Uuid::nil(), "sequence":1},
        "removed-z": 1,
        "untouched": true,
        "removed-a": 2,
        "changed": {"old":true}
    });
    let snapshot = orchestration_runtime::execution_state::CheckpointSnapshot {
        next_node_index: 1,
        active_node_ids: vec!["node-tool".into()],
        variable_pool: json!({
            RECOVERY_CONTEXT_MARKER: {"context_version_id":Uuid::nil(), "sequence":2},
            "changed": {"z":"exact\0text", "a":-0.0},
            "untouched":true,
            "new":[null, "\\u0000"]
        })
        .as_object()
        .unwrap()
        .clone(),
    };
    let original = serde_json::to_vec(&snapshot.variable_pool).unwrap();
    let compacted = super::super::checkpoint_locator::compact_checkpoint_content(
        &snapshot,
        previous.as_object(),
    )
    .unwrap();
    assert_eq!(serde_json::to_vec(&compacted.content).unwrap(),
        br#"{"format":"runtime_delta_v1","set":{"changed":{"z":"exact\u0000text","a":-0.0},"new":[null,"\\u0000"]},"remove":["removed-z","removed-a"],"llm_callback_appends":[]}"#);
    assert!(compacted.content["set"]
        .get(RECOVERY_CONTEXT_MARKER)
        .is_none());
    assert_eq!(
        serde_json::to_vec(&snapshot.variable_pool).unwrap(),
        original
    );
}
