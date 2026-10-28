use super::super::callback_resume::semantic_runtime;
use super::*;
use control_plane::orchestration_runtime::{
    CommunicationExecutionBinding, CommunicationExecutionResolver, ResumeFlowRunCommand,
};
use orchestration_runtime::execution_engine::communication::{
    CommunicationExecutionInput, CommunicationExecutionInputs, CommunicationExecutionPort,
    CommunicationExecutionSeal,
};

#[derive(Default)]
struct TurnState {
    messages: Vec<String>,
    consumed: i64,
    checkpoint: Option<Value>,
    replies: Vec<String>,
}
struct RetainedTurn(std::sync::Mutex<TurnState>);
#[async_trait::async_trait]
impl CommunicationExecutionPort for RetainedTurn {
    async fn inspect(&self) -> anyhow::Result<CommunicationExecutionInputs> {
        let state = self.0.lock().unwrap();
        Ok(CommunicationExecutionInputs {
            received_watermark: state.messages.len() as i64,
            consumed_watermark: state.consumed,
            checkpoint: state.checkpoint.clone(),
            inputs: state
                .messages
                .iter()
                .enumerate()
                .filter(|(index, _)| (*index as i64) >= state.consumed)
                .map(|(index, text)| CommunicationExecutionInput {
                    input_id: Uuid::from_u128(index as u128 + 1),
                    input_sequence: index as i64 + 1,
                    messages: vec![json!({"role":"user","content":text})],
                })
                .collect(),
            pending_admission: false,
        })
    }
    async fn checkpoint(&self, consumed: i64, continuation: Value) -> anyhow::Result<()> {
        let mut state = self.0.lock().unwrap();
        state.consumed = consumed;
        state.checkpoint = Some(continuation);
        Ok(())
    }
    async fn check_active(&self) -> anyhow::Result<()> {
        Ok(())
    }
    async fn seal(&self, reply: &str) -> anyhow::Result<CommunicationExecutionSeal> {
        let mut state = self.0.lock().unwrap();
        if state.consumed != state.messages.len() as i64 {
            return Ok(CommunicationExecutionSeal::Pending);
        }
        assert!(state.replies.is_empty());
        state.replies.push(reply.into());
        Ok(CommunicationExecutionSeal::Reserved)
    }
}
struct Resolver(Arc<RetainedTurn>);
#[async_trait::async_trait]
impl CommunicationExecutionResolver for Resolver {
    async fn resolve(
        &self,
        _: &domain::FlowRunRecord,
    ) -> anyhow::Result<Option<CommunicationExecutionBinding>> {
        Ok(Some(CommunicationExecutionBinding {
            port: self.0.clone(),
            initial_prompt_watermark: 1,
        }))
    }
}

#[tokio::test]
async fn communication_human_resume_retains_running_llm_receipt_until_one_final_seal() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let (seeded, run) = queued(&store).await;
    let compiled = store
        .get_compiled_plan(run.compiled_plan_id.unwrap())
        .await
        .unwrap()
        .unwrap();
    let (runtime, provider, package) =
        semantic_runtime::seed_runtime_consumer(&store, &seeded, &compiled, None).await;
    let mut plan: Value = sqlx::query_scalar("SELECT plan FROM flow_compiled_plans WHERE id=$1")
        .bind(compiled.id)
        .fetch_one(store.pool())
        .await
        .unwrap();
    plan["topological_order"] = json!(["node-llm", "human", "answer"]);
    plan["nodes"]["node-llm"]["downstream_node_ids"] = json!(["human"]);
    plan["nodes"]["node-llm"]["bindings"] = json!({"prompt_messages":{"kind":"prompt_messages","raw_value":[{"id":"user","role":"user","content":{"kind":"templated_text","value":"original"}}],"selector_paths":[]}});
    plan["nodes"]["human"] = json!({"node_id":"human","node_type":"human_input","alias":"Approval","dependency_node_ids":["node-llm"],"downstream_node_ids":["answer"],"bindings":{"prompt":{"kind":"templated_text","raw_value":"Approve delivery","selector_paths":[]}},"outputs":[{"key":"input","title":"Approval","value_type":"string","selector":[]}],"config":{}});
    plan["nodes"]["answer"] = json!({"node_id":"answer","node_type":"answer","alias":"Answer","dependency_node_ids":["human"],"downstream_node_ids":[],"bindings":{"answer_template":{"kind":"selector","raw_value":["node-llm","text"],"selector_paths":[["node-llm","text"]]}},"outputs":[{"key":"answer","title":"Answer","value_type":"string","selector":[]}],"config":{}});
    sqlx::query("UPDATE flow_compiled_plans SET plan=$2 WHERE id=$1")
        .bind(compiled.id)
        .bind(plan)
        .execute(store.pool())
        .await
        .unwrap();
    let turn = Arc::new(RetainedTurn(std::sync::Mutex::new(TurnState {
        messages: vec!["original".into()],
        ..TurnState::default()
    })));
    let resolver = Arc::new(Resolver(turn.clone()));
    provider.results.lock().unwrap().extend([
        plugin_framework::provider_contract::ProviderInvocationResult {
            final_content: Some("first candidate".into()),
            finish_reason: Some(plugin_framework::provider_contract::ProviderFinishReason::Stop),
            ..Default::default()
        },
        plugin_framework::provider_contract::ProviderInvocationResult {
            final_content: Some("complete original and supplement".into()),
            finish_reason: Some(plugin_framework::provider_contract::ProviderFinishReason::Stop),
            ..Default::default()
        },
    ]);
    provider.release.add_permits(2);
    let waiting = runtime
        .with_communication_execution_resolver(resolver.clone())
        .dispatch_communication_run(command(&run))
        .await
        .unwrap();
    assert_eq!(waiting.flow_run.status, FlowRunStatus::WaitingHuman);
    let llm = waiting
        .node_runs
        .iter()
        .find(|node| node.node_id == "node-llm")
        .unwrap();
    assert_eq!(
        llm.status,
        domain::NodeRunStatus::Running,
        "response LLM is still owned by the unsealed reply turn"
    );
    let llm_id = llm.id;
    assert_eq!(
        turn.0.lock().unwrap().checkpoint.as_ref().unwrap()["node_receipts"]["node-llm"],
        json!(llm_id)
    );
    turn.0
        .lock()
        .unwrap()
        .messages
        .push("supplement during approval".into());
    let fresh = OrchestrationRuntimeService::new(
        store.clone(),
        provider.clone(),
        Arc::new(runtime_core::runtime_engine::RuntimeEngine::for_tests()),
        "fixture-key",
        provider.transport_store.clone(),
        false,
    )
    .with_node_artifact_context("semantic-fixture", package.0.clone())
    .with_communication_execution_resolver(resolver);
    let completed = fresh
        .resume_flow_run(ResumeFlowRunCommand {
            actor_user_id: seeded.actor_user_id,
            application_id: run.application_id,
            flow_run_id: run.id,
            checkpoint_id: waiting.checkpoints.last().unwrap().id,
            input_payload: json!({"human":{"input":"approved"}}),
        })
        .await
        .unwrap();
    assert_eq!(completed.flow_run.status, FlowRunStatus::Succeeded);
    let llms = completed
        .node_runs
        .iter()
        .filter(|node| node.node_id == "node-llm")
        .collect::<Vec<_>>();
    assert_eq!(llms.len(), 1);
    assert_eq!(llms[0].id, llm_id);
    assert_eq!(llms[0].status, domain::NodeRunStatus::Succeeded);
    assert_eq!(
        completed
            .node_runs
            .iter()
            .filter(|node| node.node_id == "human")
            .count(),
        1
    );
    assert_eq!(
        provider
            .executions
            .load(std::sync::atomic::Ordering::SeqCst),
        2
    );
    let inputs = provider.inputs.lock().unwrap();
    assert!(inputs[1]
        .messages
        .iter()
        .any(|message| message.content == "supplement during approval"));
    assert!(inputs[1]
        .messages
        .iter()
        .any(|message| message.content == "original"));
    assert_eq!(
        turn.0.lock().unwrap().replies,
        vec!["complete original and supplement"]
    );
    assert_eq!(
        completed.flow_run.output_payload["answer"],
        "complete original and supplement"
    );
}
