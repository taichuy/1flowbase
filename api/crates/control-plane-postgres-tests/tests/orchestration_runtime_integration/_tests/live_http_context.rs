use super::*;
use extension_contracts::provider_contract::ProviderInvocationInput;
use orchestration_runtime::{
    compiled_plan::{
        CompiledCodeRuntime, CompiledLlmRuntime, CompiledNode, CompiledPlan, CompiledPluginRuntime,
    },
    execution_engine::{
        start_flow_debug_run_with_runtime_context_and_lifecycle, CapabilityInvocationOutput,
        CapabilityInvoker, CodeInvocationOutput, CodeInvoker, ExecutionLifecycle,
        ExecutionRuntimeContext, ProviderInvocationOutput, ProviderInvoker,
        WorkflowInternalHttpRequest, WorkflowInternalHttpResponse,
    },
};

struct WaitingHttp(tokio::sync::Notify);
#[async_trait::async_trait]
impl ProviderInvoker for WaitingHttp {
    async fn invoke_llm(
        &self,
        _: &CompiledLlmRuntime,
        _: ProviderInvocationInput,
    ) -> anyhow::Result<ProviderInvocationOutput> {
        unreachable!()
    }
    async fn invoke_workflow_internal_http(
        &self,
        _: &str,
        _: WorkflowInternalHttpRequest,
    ) -> anyhow::Result<WorkflowInternalHttpResponse> {
        self.0.notify_one();
        std::future::pending().await
    }
}
#[async_trait::async_trait]
impl CapabilityInvoker for WaitingHttp {
    async fn invoke_capability_node(
        &self,
        _: &CompiledPluginRuntime,
        _: Value,
        _: Value,
    ) -> anyhow::Result<CapabilityInvocationOutput> {
        unreachable!()
    }
}
#[async_trait::async_trait]
impl CodeInvoker for WaitingHttp {
    async fn invoke_code_node(
        &self,
        _: &CompiledCodeRuntime,
        _: Value,
        _: Value,
    ) -> anyhow::Result<CodeInvocationOutput> {
        unreachable!()
    }
}
struct PersistStarted {
    store: PgControlPlaneStore,
    run: Uuid,
}
#[async_trait::async_trait]
impl ExecutionLifecycle for PersistStarted {
    async fn begin_node(&self, node: &CompiledNode, input: &Value) -> anyhow::Result<()> {
        self.store
            .create_node_run(&CreateNodeRunInput {
                flow_run_id: self.run,
                node_id: node.node_id.clone(),
                node_type: node.node_type.clone(),
                node_alias: node.alias.clone(),
                status: NodeRunStatus::Running,
                input_payload: input.clone(),
                debug_payload: json!({}),
                started_at: OffsetDateTime::now_utc(),
            })
            .await?;
        Ok(())
    }
}

#[tokio::test]
async fn workflow_observation_reads_resolved_http_target_while_request_is_still_waiting() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let run = seed_flow_run(&store, &seeded, &compiled, OffsetDateTime::now_utc()).await;
    let plan: CompiledPlan = serde_json::from_value(json!({
        "flow_id":seeded.flow_id,"source_draft_id":"fixture","schema_version":"1flowbase.flow/v2","topological_order":["start","submit"],
        "nodes":{
            "start":{"node_id":"start","node_type":"workflow_start","alias":"Start","dependency_node_ids":[],"downstream_node_ids":["submit"],"bindings":{},"outputs":[],"config":{}},
            "submit":{"node_id":"submit","node_type":"http_request","alias":"Submit cleanup","dependency_node_ids":["start"],"downstream_node_ids":[],"bindings":{
                "headers":{"kind":"named_bindings","raw_value":[{"name":"Authorization","value":{"kind":"constant","value":"Bearer wfcred:fixture:executor"}}],"selector_paths":[]},
                "body":{"kind":"named_bindings","raw_value":[{"name":"job_id","value":{"kind":"constant","value":"cleanup-job"}}],"selector_paths":[]}
            },"outputs":[],"config":{"method":"post","url":"/api/console/applications/{{ sys.target_application_id }}/logs/deletion-jobs","body_type":"json","timeout_ms":30000}}
        }
    })).unwrap();
    let host = WaitingHttp(tokio::sync::Notify::new());
    let lifecycle = PersistStarted {
        store: store.clone(),
        run: run.id,
    };
    let input = json!({"sys":{"target_application_id":seeded.application_id}});
    let execution = start_flow_debug_run_with_runtime_context_and_lifecycle(
        &plan,
        &input,
        ExecutionRuntimeContext::default(),
        &host,
        &lifecycle,
    );
    tokio::pin!(execution);
    tokio::select! {
        _ = host.0.notified() => {},
        result = &mut execution => panic!("request must still be waiting: {result:?}"),
        _ = tokio::time::sleep(std::time::Duration::from_secs(3)) => panic!("request never started"),
    }
    let live = store
        .get_run_node_last_run(seeded.application_id, run.id, "submit")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(live.node_run.status, NodeRunStatus::Running);
    assert!(live.node_run.finished_at.is_none());
    assert_eq!(live.node_run.input_payload["method"], "POST");
    assert_eq!(
        live.node_run.input_payload["url"],
        format!(
            "/api/console/applications/{}/logs/deletion-jobs",
            seeded.application_id
        )
    );
    assert_eq!(live.node_run.input_payload["body"]["job_id"], "cleanup-job");
    // This scope owns only a blocked fixture future, never a network or business deletion task.
}
