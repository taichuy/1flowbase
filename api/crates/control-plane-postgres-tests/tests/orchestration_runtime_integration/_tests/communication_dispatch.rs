use super::*;
use control_plane::{
    capability_plugin_runtime::*,
    orchestration_runtime::{OrchestrationRuntimeService, StartPublishedFlowRunCommand},
    ports::{ProviderRuntimeInvocationOutput, ProviderRuntimePort},
};
use control_plane_contracts::ports::AuthRepository;
use plugin_framework::provider_contract::{ProviderInvocationInput, ProviderModelDescriptor};

// The real service and PostgreSQL adapter execute a start->answer graph.
// Provider calls are forbidden in this focused initial-ownership fixture.
#[derive(Clone)]
struct NoProvider;
#[async_trait::async_trait]
impl ProviderRuntimePort for NoProvider {
    async fn reconcile_provider_worker_demand(
        &self,
        _: &domain::PluginInstallationRecord,
        _: u64,
        _: Option<bool>,
    ) -> anyhow::Result<()> {
        Ok(())
    }
    async fn ensure_loaded(&self, _: &domain::LocalPluginInstallationRecord) -> anyhow::Result<()> {
        anyhow::bail!("unexpected provider")
    }
    async fn validate_provider(
        &self,
        _: &domain::LocalPluginInstallationRecord,
        _: Value,
    ) -> anyhow::Result<Value> {
        anyhow::bail!("unexpected provider")
    }
    async fn list_models(
        &self,
        _: &domain::LocalPluginInstallationRecord,
        _: Value,
    ) -> anyhow::Result<Vec<ProviderModelDescriptor>> {
        anyhow::bail!("unexpected provider")
    }
    async fn invoke_stream(
        &self,
        _: &domain::LocalPluginInstallationRecord,
        _: ProviderInvocationInput,
    ) -> anyhow::Result<ProviderRuntimeInvocationOutput> {
        anyhow::bail!("unexpected provider")
    }
}
#[async_trait::async_trait]
impl CapabilityPluginRuntimePort for NoProvider {
    async fn validate_config(&self, _: ValidateCapabilityConfigInput) -> anyhow::Result<Value> {
        anyhow::bail!("unexpected capability")
    }
    async fn resolve_dynamic_options(
        &self,
        _: ResolveCapabilityOptionsInput,
    ) -> anyhow::Result<Value> {
        anyhow::bail!("unexpected capability")
    }
    async fn resolve_output_schema(
        &self,
        _: ResolveCapabilityOutputSchemaInput,
    ) -> anyhow::Result<Value> {
        anyhow::bail!("unexpected capability")
    }
    async fn execute_node(
        &self,
        _: ExecuteCapabilityNodeInput,
    ) -> anyhow::Result<CapabilityExecutionOutput> {
        anyhow::bail!("unexpected capability")
    }
}
fn service(
    store: PgControlPlaneStore,
) -> OrchestrationRuntimeService<PgControlPlaneStore, NoProvider> {
    OrchestrationRuntimeService::new(
        store,
        NoProvider,
        Arc::new(runtime_core::runtime_engine::RuntimeEngine::for_tests()),
        "fixture-key",
        Arc::new(storage_ephemeral::MemoryProviderTransportStore::new(
            Duration::minutes(5),
        )),
        false,
    )
}
fn command(run: &domain::FlowRunRecord) -> StartPublishedFlowRunCommand {
    StartPublishedFlowRunCommand {
        application_id: run.application_id,
        flow_run_id: run.id,
        provider_transport_slot: None,
        transport_connection_scope: None,
        observation_context: None,
    }
}
async fn queued(store: &PgControlPlaneStore) -> (RuntimeSeedState, domain::FlowRunRecord) {
    let seeded = seed_runtime_base(store).await;
    let plan = json!({"flow_id":seeded.flow_id,"source_draft_id":seeded.draft_id.to_string(),"schema_version":"1flowbase.flow/v2",
        "topological_order":["node-start","answer"],"edges":[],"compile_issues":[],"nodes":{
        "node-start":{"node_id":"node-start","node_type":"start","alias":"Start","dependency_node_ids":[],"downstream_node_ids":["answer"],"bindings":{},"outputs":[{"key":"query","title":"Query","value_type":"string","selector":[]}],"config":{}},
        "answer":{"node_id":"answer","node_type":"answer","alias":"Answer","dependency_node_ids":["node-start"],"downstream_node_ids":[],"bindings":{"answer_template":{"kind":"templated_text","raw_value":"dispatched in bound workspace","selector_paths":[]}},"outputs":[{"key":"answer","title":"Answer","value_type":"string","selector":[]}],"config":{}}
    }});
    let compiled = store
        .upsert_compiled_plan(&UpsertCompiledPlanInput {
            actor_user_id: seeded.actor_user_id,
            flow_id: seeded.flow_id,
            flow_draft_id: seeded.draft_id,
            schema_version: "1flowbase.flow/v2".into(),
            document_hash: Uuid::now_v7().to_string(),
            document_updated_at: seeded.draft_updated_at,
            plan,
        })
        .await
        .unwrap();
    let run = store
        .create_flow_run(&CreateFlowRunInput {
            application_run_log_context: None,
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            flow_id: seeded.flow_id,
            flow_draft_id: seeded.draft_id,
            compiled_plan_id: compiled.id,
            debug_session_id: String::new(),
            flow_schema_version: compiled.schema_version,
            document_hash: compiled.document_hash,
            run_mode: FlowRunMode::AssistantExecution,
            target_node_id: None,
            title: "Communication dispatch".into(),
            status: FlowRunStatus::Queued,
            input_payload: json!({"node-start":{"query":"current input"}}),
            started_at: OffsetDateTime::now_utc(),
            api_key_id: None,
            publication_version_id: None,
            assistant_conversation_id: None,
            external_user: None,
            external_conversation_id: None,
            external_trace_id: None,
            compatibility_mode: Some("communication".into()),
            idempotency_key: None,
        })
        .await
        .unwrap();
    (seeded, run)
}
async fn started_events(store: &PgControlPlaneStore, id: Uuid) -> i64 {
    sqlx::query_scalar("select count(*) from flow_run_events where flow_run_id=$1 and event_type='assistant_execution_started'").bind(id).fetch_one(store.pool()).await.unwrap()
}
async fn mark_running(store: &PgControlPlaneStore, run: &domain::FlowRunRecord) {
    assert!(store
        .update_flow_run_if_status(
            &UpdateFlowRunInput {
                flow_run_id: run.id,
                status: FlowRunStatus::Running,
                output_payload: json!({}),
                error_payload: None,
                finished_at: None
            },
            FlowRunStatus::Queued
        )
        .await
        .unwrap()
        .is_some());
}

#[tokio::test]
async fn communication_queued_crash_recovery_and_concurrent_dispatch_have_one_engine_owner() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let (_, run) = queued(&store).await;
    // The process disappeared after durable reservation, without spawning any task.
    let a = service(store.clone());
    let b = service(store.clone());
    let (first, second) = tokio::join!(
        a.dispatch_communication_run(command(&run)),
        b.dispatch_communication_run(command(&run))
    );
    first.unwrap();
    second.unwrap();
    let detail = store
        .get_application_run_detail(run.application_id, run.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(detail.flow_run.status, FlowRunStatus::Succeeded);
    assert_eq!(
        detail.flow_run.output_payload["answer"],
        "dispatched in bound workspace"
    );
    assert_eq!(started_events(&store, run.id).await, 1);
    assert_eq!(detail.node_runs.len(), 2);
    service(store.clone())
        .dispatch_communication_run(command(&run))
        .await
        .unwrap();
    assert_eq!(started_events(&store, run.id).await, 1);
}

#[tokio::test]
async fn communication_active_dispatch_is_not_failed_and_post_cas_crash_is_not_replayed() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let (_, run) = queued(&store).await;
    let lease = store
        .try_acquire_flow_run_dispatch_lease(run.application_id, run.id)
        .await
        .unwrap()
        .unwrap();
    mark_running(&store, &run).await;
    assert_eq!(
        service(store.clone())
            .dispatch_communication_run(command(&run))
            .await
            .unwrap()
            .flow_run
            .status,
        FlowRunStatus::Running
    );
    // Drop closes the session (also the crash-after-CAS/before-engine boundary), without
    // returning a locked connection to the pool. A successor must fail, never replay AI.
    drop(lease);
    let recovered = tokio::time::timeout(std::time::Duration::from_secs(10), async {
        loop {
            let detail = service(store.clone())
                .dispatch_communication_run(command(&run))
                .await
                .unwrap();
            if detail.flow_run.status == FlowRunStatus::Failed {
                break detail;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert_eq!(
        recovered.flow_run.error_payload.unwrap()["code"],
        "communication_dispatch_interrupted"
    );
    assert!(recovered.node_runs.is_empty());
    assert_eq!(started_events(&store, run.id).await, 0);
}

#[tokio::test]
async fn communication_prestart_failure_settles_instead_of_observing_forever() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let (_, run) = queued(&store).await;
    sqlx::query("update flow_compiled_plans set plan=$2 where id=$1")
        .bind(run.compiled_plan_id)
        .bind(json!({"invalid_plan":true}))
        .execute(store.pool())
        .await
        .unwrap();
    let detail = service(store.clone())
        .dispatch_communication_run(command(&run))
        .await
        .unwrap();
    assert_eq!(detail.flow_run.status, FlowRunStatus::Failed);
    assert_eq!(
        detail.flow_run.error_payload.unwrap()["code"],
        "communication_dispatch_failed"
    );
    assert!(detail.node_runs.is_empty());
}

#[tokio::test]
async fn communication_execution_and_actor_reload_preserve_nondefault_workspace() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let (seeded, run) = queued(&store).await;
    let earlier = seed_workspace(&store, "Earlier membership").await;
    sqlx::query("insert into workspace_memberships (id,workspace_id,user_id,introduction,created_at) values ($1,$2,$3,'',now()-interval '1 day')")
        .bind(Uuid::now_v7()).bind(earlier).bind(seeded.actor_user_id).execute(store.pool()).await.unwrap();
    assert_eq!(
        ApplicationRepository::load_actor_context_for_user(&store, seeded.actor_user_id)
            .await
            .unwrap()
            .current_workspace_id,
        earlier
    );
    let actor = AuthRepository::load_actor_context(
        &store,
        seeded.actor_user_id,
        root_tenant_id(&store).await,
        seeded.workspace_id,
        None,
    )
    .await
    .unwrap();
    let scoped = store.for_actor(actor);
    assert_eq!(
        ApplicationRepository::load_actor_context_for_user(&scoped, seeded.actor_user_id)
            .await
            .unwrap()
            .current_workspace_id,
        seeded.workspace_id
    );
    assert_eq!(
        AuthRepository::load_actor_context_for_user(&scoped, seeded.actor_user_id)
            .await
            .unwrap()
            .current_workspace_id,
        seeded.workspace_id
    );
    assert!(
        ApplicationRepository::load_actor_context_for_user(&scoped, Uuid::now_v7())
            .await
            .is_err()
    );
    let detail = service(scoped)
        .dispatch_communication_run(command(&run))
        .await
        .unwrap();
    assert_eq!(detail.flow_run.status, FlowRunStatus::Succeeded);
    assert_eq!(
        detail.flow_run.output_payload["answer"],
        "dispatched in bound workspace"
    );
    assert_eq!(started_events(&store, run.id).await, 1);
}

#[tokio::test]
async fn communication_dispatch_leaves_expired_resume_claims_to_resume_recovery() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let (seeded, run) = queued(&store).await;
    sqlx::query("update flow_runs set status='waiting_human' where id=$1")
        .bind(run.id)
        .execute(store.pool())
        .await
        .unwrap();
    let checkpoint = store
        .create_checkpoint(&CreateCheckpointInput {
            flow_run_id: run.id,
            node_run_id: None,
            status: "waiting_human".into(),
            reason: "resume owner".into(),
            locator_payload: json!({}),
            variable_snapshot: json!({}),
            external_ref_payload: None,
        })
        .await
        .unwrap();
    let claim = store
        .acquire_resume_claim(&AcquireResumeClaimInput {
            scope_id: seeded.workspace_id,
            application_id: run.application_id,
            flow_run_id: run.id,
            checkpoint_id: checkpoint.id,
            callback_task_id: None,
            kind: ResumeClaimKind::Human,
            request_payload: json!({"answer":"approved"}),
        })
        .await
        .unwrap()
        .claim;
    sqlx::query(
        "update flow_run_resume_claims set lease_expires_at=now()-interval '1 second' where id=$1",
    )
    .bind(claim.id)
    .execute(store.pool())
    .await
    .unwrap();
    sqlx::query("update flow_runs set status='running' where id=$1")
        .bind(run.id)
        .execute(store.pool())
        .await
        .unwrap();
    let runtime = service(store.clone());
    assert_eq!(
        runtime
            .dispatch_communication_run(command(&run))
            .await
            .unwrap()
            .flow_run
            .status,
        FlowRunStatus::Running
    );
    runtime
        .reject_communication_dispatch(run.application_id, run.id)
        .await
        .unwrap();
    let detail = store
        .get_application_run_detail(run.application_id, run.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(detail.flow_run.status, FlowRunStatus::Running);
    assert!(detail.node_runs.is_empty());
    assert_eq!(started_events(&store, run.id).await, 0);
}
