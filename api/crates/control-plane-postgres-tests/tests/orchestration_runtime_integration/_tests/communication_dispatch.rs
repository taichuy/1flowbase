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
    // The execution engine already owns this terminal failure. Dispatch must retain
    // its diagnostic rather than replacing it with an initial-dispatch fallback code.
    let error = detail.flow_run.error_payload.clone().unwrap();
    assert!(error["message"]
        .as_str()
        .is_some_and(|message| !message.is_empty()));
    assert!(detail.node_runs.is_empty());
    let initial_starts = started_events(&store, run.id).await;
    let observed = service(store.clone())
        .dispatch_communication_run(command(&run))
        .await
        .unwrap();
    assert_eq!(observed.flow_run.status, FlowRunStatus::Failed);
    assert_eq!(observed.flow_run.error_payload, Some(error));
    assert!(observed.node_runs.is_empty());
    assert_eq!(started_events(&store, run.id).await, initial_starts);
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

#[tokio::test]
async fn communication_dispatch_completes_with_one_business_pool_connection() {
    // Hold the schema owner separately because this custom pool does not install
    // PostgresTestSchema::connect's automatic ownership hook.
    let database = isolated_database().await;
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(std::time::Duration::from_secs(2))
        .connect(database.database_url())
        .await
        .unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let (_, run) = queued(&store).await;
    let detail = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        service(store.clone()).dispatch_communication_run(command(&run)),
    )
    .await
    .expect("dispatch must not retain the only business-pool permit")
    .unwrap();
    assert_eq!(detail.flow_run.status, FlowRunStatus::Succeeded);
    assert_eq!(
        detail.flow_run.output_payload["answer"],
        "dispatched in bound workspace"
    );
    assert_eq!(started_events(&store, run.id).await, 1);
    assert_eq!(store.pool().size(), 1);
    store.pool().close().await;
}

#[tokio::test]
async fn communication_management_links_original_run_application_after_binding_changes() {
    use control_plane::ports::{
        CommunicationRepository, NotificationRepository, SaveNotificationInstanceInput,
    };
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let (seeded, run) = queued(&store).await;
    let actor = domain::ActorContext::root(seeded.actor_user_id, seeded.workspace_id, "root");
    let notification_service =
        control_plane::notifications::NotificationService::new(store.clone(), "fixture-key".into());
    let channel = Uuid::now_v7();
    let now = OffsetDateTime::now_utc();
    store
        .save_notification_instance(SaveNotificationInstanceInput {
            record: domain::NotificationInstanceRecord {
                id: channel,
                workspace_id: seeded.workspace_id,
                type_code: "fixture.communication".into(),
                installation_id: None,
                display_name: "Original channel".into(),
                status: domain::NotificationInstanceStatus::Enabled,
                config_json: json!({}),
                configured_secret_fields: vec![],
                created_at: now,
                updated_at: now,
            },
            secret_config_json: json!({}),
            master_key: "fixture-key".into(),
            create: true,
            initial_authorization: None,
        })
        .await
        .unwrap();
    let config = domain::CommunicationBindingInput {
        application_id: run.application_id,
        model: None,
        reasoning_effort: None,
        file_table_id: None,
        mcp_instance_ids: vec![],
        system_prompt: "Fixture".into(),
    };
    notification_service
        .save_binding(&actor, channel, config.clone(), "fixture-tools".into())
        .await
        .unwrap();
    let conversation = Uuid::now_v7();
    let message = Uuid::now_v7();
    sqlx::query("INSERT INTO communication_conversations(id,scope_id,instance_id,external_identity,external_conversation_key) VALUES($1,$2,$3,'alice','chat')")
        .bind(conversation).bind(seeded.workspace_id).bind(channel).execute(store.pool()).await.unwrap();
    sqlx::query("INSERT INTO communication_messages(id,scope_id,instance_id,conversation_id,external_identity,external_message_key,received_at_ms,run_id) VALUES($1,$2,$3,$4,'alice','message',42,$5)")
        .bind(message).bind(seeded.workspace_id).bind(channel).bind(conversation).bind(run.id).execute(store.pool()).await.unwrap();
    let replacement = Uuid::now_v7();
    notification_service
        .save_binding(
            &actor,
            channel,
            domain::CommunicationBindingInput {
                application_id: replacement,
                ..config
            },
            "fixture-tools".into(),
        )
        .await
        .unwrap();
    let detail = notification_service
        .management_message(&actor, message)
        .await
        .unwrap();
    assert_eq!(detail.application_id, Some(run.application_id));
    assert_eq!(detail.message.run_id, Some(run.id));
    assert_ne!(detail.application_id, Some(replacement));
    let list = notification_service
        .management_messages(&actor, &Default::default())
        .await
        .unwrap();
    assert_eq!(list.items[0].application_id, Some(run.application_id));
    let conversations = notification_service
        .management_conversations(&actor, &Default::default())
        .await
        .unwrap();
    assert_eq!(
        conversations.items[0]
            .latest_message
            .as_ref()
            .unwrap()
            .application_id,
        Some(run.application_id)
    );
    assert_eq!(
        store
            .communication_run_application_id(seeded.workspace_id, run.id)
            .await
            .unwrap(),
        Some(run.application_id)
    );
}
