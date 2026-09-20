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
    assert!(
        same_run.variable_pool.contains_key("node-tool"),
        "base callback state must survive the delta"
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
