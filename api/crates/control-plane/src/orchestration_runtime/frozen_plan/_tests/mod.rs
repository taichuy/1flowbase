use std::{collections::HashMap, sync::Mutex};

use async_trait::async_trait;
use serde_json::json;

use super::*;
use crate::orchestration_runtime::{
    CompleteCallbackTaskCommand, ContinueFlowDebugRunCommand, ResumeFlowRunCommand,
    StartFlowDebugRunCommand,
};

// Behavior fixture: cached records remain immutable; misses invoke the actual
// repository loader. Assertions below check plan content and service effects.
#[derive(Default)]
struct PlanCacheFixture {
    plans: Mutex<HashMap<Uuid, Arc<domain::CompiledPlanRecord>>>,
}

#[async_trait]
impl PublishedPlanCache for PlanCacheFixture {
    async fn get_or_load(
        &self,
        id: Uuid,
        loader: PublishedPlanLoader,
    ) -> anyhow::Result<Option<Arc<domain::CompiledPlanRecord>>> {
        if let Some(plan) = self
            .plans
            .lock()
            .expect("cache fixture lock")
            .get(&id)
            .cloned()
        {
            return Ok(Some(plan));
        }
        let plan = loader().await?.map(Arc::new);
        if let Some(plan) = &plan {
            self.plans
                .lock()
                .expect("cache fixture lock")
                .insert(id, plan.clone());
        }
        Ok(plan)
    }
}

fn fail_loader() -> PublishedPlanLoader {
    Box::new(|| Box::pin(async { Err(anyhow::anyhow!("durable plan loader failed")) }))
}

#[tokio::test]
async fn frozen_plan_helper_propagates_loader_error_and_does_not_cache_missing() {
    let cache: Arc<dyn PublishedPlanCache> = Arc::new(PlanCacheFixture::default());
    let id = Uuid::now_v7();
    for cache in [None, Some(&cache)] {
        assert!(
            load_frozen_plan(cache, id, Box::new(|| Box::pin(async { Ok(None) })))
                .await
                .expect("missing row is not an error")
                .is_none()
        );
        let error = load_frozen_plan(cache, id, fail_loader())
            .await
            .expect_err("loader fails");
        assert_eq!(error.to_string(), "durable plan loader failed");
    }
}

#[tokio::test]
async fn frozen_plan_service_loader_preserves_database_behavior_without_cache() {
    let service = OrchestrationRuntimeService::for_tests();
    assert!(service
        .load_frozen_compiled_plan(Uuid::now_v7())
        .await
        .unwrap()
        .is_none());
    let seeded = service.seed_waiting_callback_run("Uncached callback").await;
    let run = service
        .repository
        .get_flow_run(seeded.application_id, seeded.flow_run_id)
        .await
        .unwrap()
        .unwrap();
    let id = run.compiled_plan_id.unwrap();
    let first = service
        .load_frozen_compiled_plan(id)
        .await
        .unwrap()
        .unwrap();
    let second = service
        .load_frozen_compiled_plan(id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(first.plan, second.plan);
    assert_eq!(first.id, id);
    assert!(!Arc::ptr_eq(&first, &second));
}

#[tokio::test]
async fn callback_cache_hit_and_miss_execute_frozen_old_plan() {
    for warm in [false, true] {
        let service = OrchestrationRuntimeService::for_tests();
        let seeded = service.seed_waiting_callback_run("Frozen callback").await;
        let run = service
            .repository
            .get_flow_run(seeded.application_id, seeded.flow_run_id)
            .await
            .unwrap()
            .unwrap();
        let id = run.compiled_plan_id.unwrap();
        let frozen = Arc::new(
            service
                .repository
                .get_compiled_plan(id)
                .await
                .unwrap()
                .unwrap(),
        );
        // A new immutable ID for this same flow must never replace the run's ID.
        let latest = service
            .repository
            .upsert_compiled_plan(&crate::ports::UpsertCompiledPlanInput {
                actor_user_id: seeded.actor_user_id,
                flow_id: frozen.flow_id,
                flow_draft_id: frozen.draft_id,
                schema_version: frozen.schema_version.clone(),
                document_hash: "new-publication".into(),
                document_updated_at: frozen.document_updated_at,
                plan: json!({"new_publication": true}),
            })
            .await
            .unwrap();
        let cache = Arc::new(PlanCacheFixture::default());
        cache
            .plans
            .lock()
            .unwrap()
            .insert(latest.id, Arc::new(latest));
        if warm {
            cache.plans.lock().unwrap().insert(id, frozen.clone());
        }
        let service = service.with_published_plan_cache(cache.clone());
        let completed = service
            .complete_callback_task(CompleteCallbackTaskCommand {
                responses_continuation: None,
                transport_connection_scope: None,
                observation_context: None,
                native_transport: None,
                actor_user_id: seeded.actor_user_id,
                application_id: seeded.application_id,
                callback_task_id: seeded.callback_task_id,
                response_payload: json!({"result": {"status": "ok"}}),
            })
            .await
            .expect("frozen callback plan remains executable");
        assert_eq!(completed.flow_run.status, domain::FlowRunStatus::Succeeded);
        assert_eq!(
            completed.callback_tasks[0].status,
            domain::CallbackTaskStatus::Completed
        );
        assert_eq!(completed.flow_run.compiled_plan_id, Some(id));
        assert!(completed
            .node_runs
            .iter()
            .any(|node| node.node_id == "node-answer"));
        let loaded = service
            .load_frozen_compiled_plan(id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(loaded.plan, frozen.plan);
        assert!(Arc::ptr_eq(&loaded, &cache.plans.lock().unwrap()[&id]));
        if warm {
            assert!(Arc::ptr_eq(&loaded, &frozen));
        }
        let duplicate = service
            .complete_callback_task(CompleteCallbackTaskCommand {
                responses_continuation: None,
                transport_connection_scope: None,
                observation_context: None,
                native_transport: None,
                actor_user_id: seeded.actor_user_id,
                application_id: seeded.application_id,
                callback_task_id: seeded.callback_task_id,
                response_payload: json!({"result": {"status": "ok"}}),
            })
            .await
            .expect_err("warm cache cannot admit a completed callback again");
        assert!(matches!(
            duplicate.downcast_ref::<crate::errors::ControlPlaneError>(),
            Some(crate::errors::ControlPlaneError::Conflict(
                "callback_task_not_pending"
            ))
        ));
        let durable = service
            .repository
            .get_application_run_detail(seeded.application_id, seeded.flow_run_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(durable.node_runs.len(), completed.node_runs.len());
        assert_eq!(
            durable.flow_run.output_payload,
            completed.flow_run.output_payload
        );
        let cache_port: Arc<dyn PublishedPlanCache> = cache;
        let hit = load_frozen_plan(Some(&cache_port), id, fail_loader())
            .await
            .expect("warm immutable plan does not need a durable loader")
            .unwrap();
        assert!(Arc::ptr_eq(&hit, &loaded));
    }
}

#[tokio::test]
async fn continue_and_human_resume_share_frozen_plan_cache() {
    let cache = Arc::new(PlanCacheFixture::default());
    let service = OrchestrationRuntimeService::for_tests().with_published_plan_cache(cache.clone());
    let seeded = service
        .seed_application_with_human_input_flow("Cached human resume")
        .await;
    let started = service
        .start_flow_debug_run(StartFlowDebugRunCommand {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            input_payload: json!({"node-start": {"query": "approve"}}),
            document_snapshot: None,
            debug_session_id: None,
        })
        .await
        .unwrap();
    let id = started.flow_run.compiled_plan_id.unwrap();
    let waiting = service
        .continue_flow_debug_run(ContinueFlowDebugRunCommand {
            application_id: seeded.application_id,
            flow_run_id: started.flow_run.id,
            workspace_id: Uuid::nil(),
        })
        .await
        .unwrap();
    assert_eq!(waiting.flow_run.status, domain::FlowRunStatus::WaitingHuman);
    let cached = cache.plans.lock().unwrap()[&id].clone();
    let completed = service
        .resume_flow_run(ResumeFlowRunCommand {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            flow_run_id: started.flow_run.id,
            checkpoint_id: waiting.checkpoints.last().unwrap().id,
            input_payload: json!({"node-human": {"input": "approved"}}),
        })
        .await
        .unwrap();
    assert_eq!(completed.flow_run.status, domain::FlowRunStatus::Succeeded);
    assert_eq!(
        completed.flow_run.output_payload["answer"],
        json!("approved")
    );
    assert!(Arc::ptr_eq(
        &cached,
        &service
            .load_frozen_compiled_plan(id)
            .await
            .unwrap()
            .unwrap()
    ));
}

#[tokio::test]
async fn warm_callback_cache_does_not_bypass_permission() {
    let service = OrchestrationRuntimeService::for_tests();
    let seeded = service.seed_waiting_callback_run("Authorized cache").await;
    let run = service
        .repository
        .get_flow_run(seeded.application_id, seeded.flow_run_id)
        .await
        .unwrap()
        .unwrap();
    let id = run.compiled_plan_id.unwrap();
    let cache = Arc::new(PlanCacheFixture::default());
    let frozen = service
        .repository
        .get_compiled_plan(id)
        .await
        .unwrap()
        .unwrap();
    cache.plans.lock().unwrap().insert(id, Arc::new(frozen));
    let service = service.with_published_plan_cache(cache);
    let command = CompleteCallbackTaskCommand {
        responses_continuation: None,
        transport_connection_scope: None,
        observation_context: None,
        native_transport: None,
        actor_user_id: seeded.actor_user_id,
        application_id: seeded.application_id,
        callback_task_id: seeded.callback_task_id,
        response_payload: json!({"result": {"status": "ok"}}),
    };
    service.replace_application_console_policies_for_tests(Vec::new());
    let error = service
        .complete_callback_task(command)
        .await
        .expect_err("permission is still required");
    assert!(matches!(
        error.downcast_ref::<crate::errors::ControlPlaneError>(),
        Some(crate::errors::ControlPlaneError::PermissionDenied(_))
    ));
    let task = service
        .callback_task_for_tests(seeded.callback_task_id)
        .await;
    assert_eq!(task.status, domain::CallbackTaskStatus::Pending);
    let current = service
        .repository
        .get_flow_run(seeded.application_id, seeded.flow_run_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(current.status, domain::FlowRunStatus::WaitingCallback);
}

struct FailingCache;

#[async_trait]
impl PublishedPlanCache for FailingCache {
    async fn get_or_load(
        &self,
        _id: Uuid,
        _loader: PublishedPlanLoader,
    ) -> anyhow::Result<Option<Arc<domain::CompiledPlanRecord>>> {
        Err(anyhow::anyhow!("frozen plan read failed"))
    }
}

#[tokio::test]
async fn callback_cache_error_propagates_before_completing_pending_task() {
    let service = OrchestrationRuntimeService::for_tests();
    let seeded = service.seed_waiting_callback_run("Failed plan read").await;
    let service = service.with_published_plan_cache(Arc::new(FailingCache));
    let error = service
        .complete_callback_task(CompleteCallbackTaskCommand {
            responses_continuation: None,
            transport_connection_scope: None,
            observation_context: None,
            native_transport: None,
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            callback_task_id: seeded.callback_task_id,
            response_payload: json!({"result": {"status": "ok"}}),
        })
        .await
        .expect_err("cache error is not replaced by latest plan or recompilation");
    assert_eq!(error.to_string(), "frozen plan read failed");
    assert_eq!(
        service
            .callback_task_for_tests(seeded.callback_task_id)
            .await
            .status,
        domain::CallbackTaskStatus::Pending
    );
    assert_eq!(
        service
            .repository
            .get_flow_run(seeded.application_id, seeded.flow_run_id)
            .await
            .unwrap()
            .unwrap()
            .status,
        domain::FlowRunStatus::WaitingCallback
    );
}

#[tokio::test]
async fn continue_and_human_resume_propagate_plan_read_error() {
    let service = OrchestrationRuntimeService::for_tests();
    let seeded = service
        .seed_application_with_human_input_flow("Continue read failure")
        .await;
    let started = service
        .start_flow_debug_run(StartFlowDebugRunCommand {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            input_payload: json!({"node-start": {"query": "approve"}}),
            document_snapshot: None,
            debug_session_id: None,
        })
        .await
        .unwrap();
    let service = service.with_published_plan_cache(Arc::new(FailingCache));
    let failed = service
        .continue_flow_debug_run(ContinueFlowDebugRunCommand {
            application_id: seeded.application_id,
            flow_run_id: started.flow_run.id,
            workspace_id: Uuid::nil(),
        })
        .await
        .expect("continue settles its failure through the existing terminal owner");
    assert_eq!(failed.flow_run.status, domain::FlowRunStatus::Failed);
    assert_eq!(
        failed.flow_run.error_payload.as_ref().unwrap()["message"],
        "frozen plan read failed"
    );
    assert_eq!(
        service
            .repository
            .get_flow_run(seeded.application_id, started.flow_run.id)
            .await
            .unwrap()
            .unwrap()
            .status,
        domain::FlowRunStatus::Failed
    );

    let service = OrchestrationRuntimeService::for_tests();
    let seeded = service.seed_waiting_human_run("Resume read failure").await;
    let service = service.with_published_plan_cache(Arc::new(FailingCache));
    let error = service
        .resume_flow_run(ResumeFlowRunCommand {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            flow_run_id: seeded.flow_run_id,
            checkpoint_id: seeded.checkpoint_id,
            input_payload: json!({"node-human": {"input": "approved"}}),
        })
        .await
        .expect_err("resume must propagate frozen plan error");
    assert_eq!(error.to_string(), "frozen plan read failed");
    assert_eq!(
        service
            .repository
            .get_flow_run(seeded.application_id, seeded.flow_run_id)
            .await
            .unwrap()
            .unwrap()
            .status,
        domain::FlowRunStatus::WaitingHuman
    );
}
