use super::*;
use crate::orchestration_runtime::test_support::{
    InMemoryOrchestrationRuntimeRepository as Repository, InMemoryProviderRuntime as Runtime,
};
use crate::orchestration_runtime::{RuntimeDataModelExecutionContext, RuntimeFlowExecutionContext};
use control_plane_contracts::ports::{CreateFlowRunInput, InternalCompactionHead};
use orchestration_runtime::compiled_plan::{CompiledLlmRuntime, CompiledNode, CompiledPlan};
use orchestration_runtime::execution_engine::{
    execute_llm_node, ExecutionLifecycle, ExecutionRuntimeContext, ResolvedProviderRoute,
    RuntimeInternalToolInvoker, RuntimeInternalToolOutput, RuntimeInternalToolRegistration,
};
use plugin_framework::provider_contract::{
    NativePromptBlock, ProviderCountTokensResult, ProviderFinishReason, ProviderInvocationResult,
    ProviderMessage, ProviderMessageRole, ProviderToolCall,
};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

const WINDOW: u64 = 10_000;
const RAW_USER: &str =
    "Use the existing record only. My exact latest question must remain visible.";

struct Fixture {
    repository: Repository,
    invoker: RuntimeProviderInvoker<Repository, Runtime>,
    compiled: CompiledLlmRuntime,
    route: ResolvedProviderRoute,
    input: ProviderInvocationInput,
    counts: Arc<Mutex<Vec<ProviderInvocationInput>>>,
    generated: Arc<Mutex<Vec<ProviderInvocationInput>>>,
    run_id: Uuid,
}

fn message(role: ProviderMessageRole, content: &str) -> ProviderMessage {
    ProviderMessage {
        role,
        content: content.into(),
        name: None,
        tool_call_id: None,
        is_error: None,
        tool_calls: None,
        content_blocks: None,
    }
}

fn tool_group(id: &str) -> [ProviderMessage; 2] {
    let mut call = message(ProviderMessageRole::Assistant, "");
    call.tool_calls = Some(
        json!([{"id":id,"type":"function","function":{"name":"update_record","arguments":"{}"}}]),
    );
    let mut result = message(ProviderMessageRole::Tool, &format!("completed-result:{id}"));
    result.tool_call_id = Some(id.into());
    [call, result]
}

fn closed_history() -> Vec<ProviderMessage> {
    let mut messages = vec![message(ProviderMessageRole::User, RAW_USER)];
    messages.extend(tool_group("old-write"));
    messages.push(message(ProviderMessageRole::Assistant, "old closed action"));
    messages.extend(tool_group("recent-read-one"));
    messages.extend(tool_group("recent-read-two"));
    messages
}

fn result(text: &str) -> ProviderInvocationResult {
    ProviderInvocationResult {
        final_content: Some(text.into()),
        finish_reason: Some(ProviderFinishReason::Stop),
        ..Default::default()
    }
}

impl Fixture {
    async fn new(runtime: Runtime, tokens: &[u64]) -> Self {
        let repository = Repository::with_permissions(vec![]);
        repository.enable_internal_compaction_fixture();
        let (provider_id, _) = repository.seed_included_provider_instances();
        let (runtime, generated) = runtime.capturing_invocations();
        let (runtime, counts) = runtime.with_count_tokens_results(
            tokens
                .iter()
                .map(|input_tokens| ProviderCountTokensResult {
                    input_tokens: *input_tokens,
                    ..Default::default()
                })
                .collect(),
        );
        let application_id = Uuid::now_v7();
        let run = repository
            .create_flow_run(&CreateFlowRunInput {
                application_run_log_context: None,
                actor_user_id: Uuid::nil(),
                application_id,
                flow_id: Uuid::now_v7(),
                flow_draft_id: Uuid::now_v7(),
                compiled_plan_id: Uuid::now_v7(),
                debug_session_id: "compaction-controller".into(),
                flow_schema_version: "1".into(),
                document_hash: "fixture".into(),
                run_mode: domain::FlowRunMode::DebugFlowRun,
                target_node_id: None,
                title: "Existing task".into(),
                status: domain::FlowRunStatus::Running,
                input_payload: json!({}),
                started_at: OffsetDateTime::now_utc(),
                api_key_id: None,
                publication_version_id: None,
                assistant_conversation_id: None,
                external_user: None,
                external_conversation_id: None,
                external_trace_id: None,
                compatibility_mode: None,
                idempotency_key: None,
            })
            .await
            .unwrap();
        let compiled = CompiledLlmRuntime {
            provider_instance_id: provider_id.to_string(),
            provider_instance_display_name: "Selected Alpha".into(),
            provider_code: "fixture_provider".into(),
            protocol: "openai_compatible".into(),
            model: "gpt-5.4-mini".into(),
            routing: None,
        };
        let invoker = RuntimeProviderInvoker {
            repository: repository.clone(),
            runtime,
            workspace_id: Uuid::nil(),
            provider_secret_master_key: "test-master-key".into(),
            live_provider_events: None,
            runtime_event_stream: None,
            flow_run_id: Some(run.id),
            provider_session_scope: Some("existing-conversation".into()),
            active_node_id: None,
            active_node_run_id: None,
            api_node_id: Some("local:test".into()),
            provider_install_root: Some(std::env::temp_dir()),
            flow_execution_context: Some(Arc::new(RuntimeFlowExecutionContext {
                notification_sender: None,
                user_account: None,
                require_provider_usage_for_billing: false,
                active_node: Mutex::new(None),
                tool_delivery_events: Mutex::new(Vec::new()),
                data_model: RuntimeDataModelExecutionContext {
                    actor: domain::ActorContext::root(Uuid::nil(), Uuid::nil(), "root"),
                    application_id,
                    draft_id: Uuid::nil(),
                    flow_run_id: run.id,
                    runtime_engine: Arc::new(
                        runtime_core::runtime_engine::RuntimeEngine::for_tests(),
                    ),
                },
            })),
            answer_presentation: None,
            transport_connection_scope_override: None,
            observation_context: None,
            provider_transport_payload: None,
            provider_transport_store: None,
            provider_continuation: None,
            response_round_id: None,
            native_user_messages_digest: None,
            native_history: None,
            model_pricing_cache_store: None,
        };
        let route = invoker.resolve_llm_route(&compiled).await.unwrap();
        let mut input = ProviderInvocationInput {
            provider_instance_id: compiled.provider_instance_id.clone(),
            provider_code: compiled.provider_code.clone(),
            protocol: compiled.protocol.clone(),
            model: compiled.model.clone(),
            system: vec![NativePromptBlock::text("Keep the user's constraints.")],
            messages: closed_history(),
            tools: vec![
                json!({"type":"function","function":{"name":"update_record","description":"Update the selected record","parameters":{"type":"object","properties":{}}}}),
            ],
            ..Default::default()
        };
        input
            .run_context
            .insert(POLICY_KEY.into(), json!({"enabled":true}));
        input
            .trace_context
            .insert("node_id".into(), "existing-llm".into());
        input
            .model_parameters
            .insert("max_tokens".into(), json!(512));
        input
            .model_parameters
            .insert("requested_context_window".into(), json!(WINDOW));
        Self {
            repository,
            invoker,
            compiled,
            route,
            input,
            counts,
            generated,
            run_id: run.id,
        }
    }

    async fn apply(&self, input: &mut ProviderInvocationInput) -> Result<Option<Value>> {
        self.invoker
            .apply_internal_compaction(&self.compiled, &self.route, input, Some(WINDOW))
            .await
    }

    fn head(&self) -> InternalCompactionHead {
        let heads = self.repository.internal_compaction_heads();
        assert_eq!(heads.len(), 1);
        heads[0].clone()
    }

    async fn seed_previous_checkpoint(&self) -> InternalCompactionHead {
        let mut source = self.input.clone();
        self.apply(&mut source).await.unwrap();
        let observations = self.repository.internal_compaction_observations();
        let observed = observations.last().unwrap();
        let checkpoint = WorkingCheckpoint {
            schema_version: 1,
            prefix_hash: observed.prefix_hash.clone(),
            covered_source_hashes: observed.source_hashes[..3].to_vec(),
            retained_item_refs: vec![0],
            summary: "Previous accepted checkpoint".into(),
        };
        self.repository
            .commit_internal_compaction(&CommitInternalCompactionInput {
                scope_id: observed.scope_id,
                application_id: observed.application_id,
                flow_run_id: Some(self.run_id),
                session_key: observed.session_key.clone(),
                expected_checkpoint_id: None,
                prefix_hash: observed.prefix_hash.clone(),
                covered_source_hashes: checkpoint.covered_source_hashes.clone(),
                checkpoint: serde_json::to_value(checkpoint).unwrap(),
            })
            .await
            .unwrap()
            .unwrap();
        self.head()
    }
}

#[tokio::test]
async fn enabled_host_counts_complete_input_on_selected_runtime_and_disabled_skips_it() {
    let fixture = Fixture::new(Runtime::default(), &[4_000]).await;
    let mut disabled = fixture.input.clone();
    disabled
        .run_context
        .insert(POLICY_KEY.into(), json!({"enabled":false}));
    assert_eq!(fixture.apply(&mut disabled).await.unwrap(), None);
    assert!(fixture.counts.lock().unwrap().is_empty());
    assert!(fixture
        .repository
        .internal_compaction_observations()
        .is_empty());

    let mut enabled = fixture.input.clone();
    enabled.mcp_bindings = vec![json!({"name":"bound-mcp","tool_schema":{"type":"object"}})];
    enabled.response_format = Some(json!({"type":"json_object"}));
    enabled.request_context.end_user_reference = Some("actor-reference".into());
    let receipt = fixture.apply(&mut enabled).await.unwrap().unwrap();
    assert_eq!(receipt["status"], "below_threshold");
    assert_eq!(receipt["output_reserve"], 512);
    assert_eq!(receipt["count"]["method"], "upstream_api");
    let counted = fixture.counts.lock().unwrap();
    assert_eq!(counted.len(), 1);
    let counted = &counted[0];
    assert_eq!(
        counted.provider_instance_id,
        fixture.compiled.provider_instance_id
    );
    assert_eq!(counted.model, fixture.compiled.model);
    assert!(counted
        .provider_config
        .to_string()
        .contains("alpha.example.com"));
    assert_eq!(counted.messages, enabled.messages);
    assert_eq!(counted.system, enabled.system);
    assert_eq!(counted.tools, enabled.tools);
    assert_eq!(counted.mcp_bindings, enabled.mcp_bindings);
    assert_eq!(counted.response_format, enabled.response_format);
    assert_eq!(counted.request_context, enabled.request_context);
    assert_eq!(counted.model_parameters["max_output_tokens"], 512);
    let observed = fixture.repository.internal_compaction_observations();
    assert_eq!(
        observed[0].source,
        serde_json::to_value(&fixture.input.messages).unwrap()
    );
    assert!(fixture.generated.lock().unwrap().is_empty());
    assert_eq!(fixture.head(), InternalCompactionHead::default());
}

#[tokio::test]
async fn resolved_host_uses_output_reserve_then_reuses_checkpoint_for_tool_continuation() {
    let runtime = Runtime::with_provider_results(vec![
        result("Saved actions are complete; retain the constraint. <checkpoint-complete>"),
        result("normal response"),
        result("continuation response"),
    ]);
    let fixture = Fixture::new(runtime, &[8_300, 1_000, 1_000, 4_000, 4_000]).await;
    let mut input = fixture.input.clone();
    input
        .model_parameters
        .insert("max_output_tokens".into(), json!(2_000));
    let output = fixture
        .invoker
        .invoke_resolved_llm(&fixture.compiled, fixture.route.clone(), input.clone())
        .await
        .unwrap();
    let receipt = &output.result.provider_metadata["_1flowbase_upstream_provider_metadata"]
        ["_1flowbase_internal_compaction"];
    assert_eq!(receipt["status"], "activated");
    assert_eq!(receipt["input_tokens_before"], 8_300);
    assert_eq!(receipt["threshold"], 8_500);
    assert_eq!(receipt["output_reserve"], 2_000);
    assert_eq!(receipt["input_tokens_after"], 4_000);
    let active = fixture.head();
    assert!(active.checkpoint_id.is_some());
    let checkpoint: WorkingCheckpoint =
        serde_json::from_value(active.checkpoint.clone().unwrap()).unwrap();
    assert_eq!(checkpoint.covered_source_hashes.len(), 4);
    assert_eq!(checkpoint.retained_item_refs, vec![0]);

    input.messages.extend(tool_group("new-continuation-result"));
    let output = fixture
        .invoker
        .invoke_resolved_llm(&fixture.compiled, fixture.route.clone(), input)
        .await
        .unwrap();
    assert_eq!(
        output.result.provider_metadata["_1flowbase_upstream_provider_metadata"]
            ["_1flowbase_internal_compaction"]["status"],
        "reused"
    );
    assert_eq!(fixture.head(), active);
    assert_eq!(fixture.repository.internal_compaction_candidates().len(), 1);
    assert_eq!(
        fixture.repository.internal_compaction_run_count(),
        1,
        "summary must remain inside the existing task"
    );
    let generated = fixture.generated.lock().unwrap();
    assert_eq!(
        generated.len(),
        3,
        "one summary and two ordinary invocations, without recursive summaries"
    );
    let summary = &generated[0];
    assert_eq!(summary.operation, ProviderWireOperation::Generate);
    assert_eq!(summary.run_context[SUMMARY_KEY], true);
    assert!(summary.tools.is_empty() && summary.mcp_bindings.is_empty());
    assert_eq!(summary.model_parameters["max_output_tokens"], 4_096);
    assert_eq!(
        summary.provider_instance_id,
        fixture.compiled.provider_instance_id
    );
    for normal in &generated[1..] {
        assert_eq!(normal.model_parameters["max_output_tokens"], 2_000);
        assert_eq!(
            normal
                .messages
                .iter()
                .filter(|m| m.role == ProviderMessageRole::User && m.content == RAW_USER)
                .count(),
            1
        );
        let messages = serde_json::to_string(&normal.messages).unwrap();
        assert!(!messages.contains("old-write"));
        assert!(!messages.contains("old closed action"));
        assert!(messages.contains("recent-read-one") && messages.contains("recent-read-two"));
        assert_eq!(normal.tools, fixture.input.tools);
    }
    assert!(serde_json::to_string(&generated[2].messages)
        .unwrap()
        .contains("new-continuation-result"));
    assert_eq!(fixture.counts.lock().unwrap().len(), 5);
    let observations = fixture.repository.internal_compaction_observations();
    assert_eq!(observations.len(), 2);
    assert!(
        observations[1].source.to_string().contains("old-write"),
        "the durable original keeps the old completed action"
    );
}

#[tokio::test]
async fn failed_summary_attempts_preserve_existing_checkpoint_and_model_input() {
    for failure in [
        "empty",
        "truncated",
        "length",
        "model_error",
        "no_shrink",
        "cancel",
    ] {
        let mut summary = result("New checkpoint <checkpoint-complete>");
        match failure {
            "empty" => summary.final_content = Some(" <checkpoint-complete>".into()),
            "truncated" => summary.final_content = Some("partial checkpoint".into()),
            "length" => summary.finish_reason = Some(ProviderFinishReason::Length),
            _ => {}
        }
        let runtime = if failure == "model_error" {
            Runtime::with_fail_before_token_models(vec!["gpt-5.4-mini"])
        } else {
            Runtime::with_provider_result(summary)
        };
        let mut counts = vec![4_000, 8_600, 1_000, 1_000];
        if failure == "no_shrink" {
            counts.push(8_600);
        }
        let mut fixture = Fixture::new(runtime, &counts).await;
        let previous = fixture.seed_previous_checkpoint().await;
        let checkpoint: WorkingCheckpoint =
            serde_json::from_value(previous.checkpoint.clone().unwrap()).unwrap();
        let expected_view = checkpoint
            .view(&fixture.input.messages, &checkpoint.prefix_hash)
            .unwrap();
        if failure == "cancel" {
            let repository = fixture.repository.clone();
            let run_id = fixture.run_id;
            fixture.invoker.runtime =
                fixture
                    .invoker
                    .runtime
                    .clone()
                    .before_invocation_return(move || {
                        repository.force_flow_run_status(run_id, domain::FlowRunStatus::Cancelled);
                    });
        }
        let mut input = fixture.input.clone();
        let outcome = fixture.apply(&mut input).await;
        if failure == "cancel" {
            assert!(outcome
                .unwrap_err()
                .to_string()
                .contains("compaction run is no longer active"));
        } else {
            let receipt = outcome.unwrap().unwrap();
            assert_eq!(receipt["status"], "failed_preserved", "{failure}");
            let expected = match failure {
                "empty" => "empty summary",
                "truncated" => "summary is empty or truncated",
                "length" => "summary did not complete normally",
                "model_error" => "provider unavailable before first token",
                "no_shrink" => "summary did not release enough context growth space",
                _ => unreachable!(),
            };
            assert!(
                receipt["failure"]
                    .as_str()
                    .is_some_and(|text| text.contains(expected)),
                "{failure}: {receipt}"
            );
        }
        assert_eq!(fixture.head(), previous, "{failure}");
        assert_eq!(input.messages, expected_view, "{failure}");
        assert_eq!(
            fixture.repository.internal_compaction_candidates().len(),
            1,
            "invalid summaries must not reach publication: {failure}"
        );
        assert_eq!(
            fixture.generated.lock().unwrap().len(),
            1,
            "one attempt only: {failure}"
        );
        assert_eq!(
            fixture.generated.lock().unwrap()[0].run_context[SUMMARY_KEY],
            true
        );
        assert_eq!(
            fixture.counts.lock().unwrap().len(),
            if failure == "no_shrink" { 5 } else { 4 }
        );
        assert_eq!(fixture.repository.internal_compaction_run_count(), 1);
    }
}

#[tokio::test]
async fn unsafe_failed_summary_blocks_actual_normal_generation_and_preserves_checkpoint() {
    let fixture = Fixture::new(
        Runtime::with_provider_result(result("truncated summary")),
        &[4_000, 8_300, 1_000, 1_000],
    )
    .await;
    let previous = fixture.seed_previous_checkpoint().await;
    let mut input = fixture.input.clone();
    input
        .model_parameters
        .insert("max_output_tokens".into(), json!(2_000));
    let error = fixture
        .invoker
        .invoke_resolved_llm(&fixture.compiled, fixture.route.clone(), input)
        .await
        .unwrap_err();
    assert!(
        error.to_string().contains("existing model input is unsafe"),
        "{error}"
    );
    assert_eq!(fixture.head(), previous);
    assert_eq!(
        fixture.generated.lock().unwrap().len(),
        1,
        "unsafe main Generate must never reach the runtime"
    );
    assert_eq!(
        fixture.generated.lock().unwrap()[0].run_context[SUMMARY_KEY],
        true
    );
    assert_eq!(fixture.repository.internal_compaction_candidates().len(), 1);
}

#[tokio::test]
async fn edited_source_or_prefix_rebuilds_raw_view_without_erasing_previous_checkpoint() {
    for edit_prefix in [false, true] {
        let fixture = Fixture::new(Runtime::default(), &[4_000, 2_000]).await;
        let previous = fixture.seed_previous_checkpoint().await;
        let mut changed = fixture.input.clone();
        if edit_prefix {
            changed
                .system
                .push(NativePromptBlock::text("New authorized system constraint"));
        } else {
            changed.messages[2].content = "corrected original tool result".into();
        }
        let raw = changed.messages.clone();
        let receipt = fixture.apply(&mut changed).await.unwrap().unwrap();
        assert_eq!(receipt["status"], "below_threshold");
        assert_eq!(changed.messages, raw);
        assert_eq!(fixture.head(), previous);
        let observations = fixture.repository.internal_compaction_observations();
        if edit_prefix {
            assert_ne!(observations[0].prefix_hash, observations[1].prefix_hash);
        } else {
            assert_ne!(
                observations[0].source_hashes[2],
                observations[1].source_hashes[2]
            );
        }
        assert_eq!(observations[1].source, serde_json::to_value(raw).unwrap());
        assert!(fixture.generated.lock().unwrap().is_empty());
    }
}

#[derive(Default)]
struct CountingInternalTool {
    effects: AtomicUsize,
}

#[async_trait]
impl RuntimeInternalToolInvoker for CountingInternalTool {
    fn registrations_for_node(&self, _node: &CompiledNode) -> Vec<RuntimeInternalToolRegistration> {
        vec![RuntimeInternalToolRegistration {
            registration_id: "fixture|update_record".into(),
            provider_name: "update_record".into(),
            provider_tool: json!({"type":"function","function":{"name":"update_record","description":"Update one record","parameters":{"type":"object","properties":{"record_id":{"type":"integer"}}}}}),
            owner: json!({"kind":"mcp_instance","instance_id":"fixture","operation":"call","source":{"kind":"run","key":"0"}}),
        }]
    }

    async fn invoke_runtime_internal_tool(
        &self,
        _node: &CompiledNode,
        _registration: &RuntimeInternalToolRegistration,
        arguments: Value,
    ) -> Result<RuntimeInternalToolOutput> {
        assert_eq!(
            arguments,
            json!({"record_id":42}),
            "historical writes must never replay"
        );
        self.effects.fetch_add(1, Ordering::SeqCst);
        Ok(RuntimeInternalToolOutput {
            content: json!("new-write-committed-receipt"),
            is_error: false,
            event: json!({"event_type":"fixture_write_completed"}),
        })
    }
}

#[derive(Default)]
struct ToolReceipts {
    started: Mutex<Vec<Value>>,
    finished: Mutex<Vec<Value>>,
}

#[async_trait]
impl ExecutionLifecycle for ToolReceipts {
    async fn begin_node(&self, _node: &CompiledNode, _input: &Value) -> Result<()> {
        Ok(())
    }

    async fn runtime_internal_tool_started(
        &self,
        _node: &CompiledNode,
        call: &Value,
        _usage: &Value,
    ) -> Result<()> {
        self.started.lock().unwrap().push(call.clone());
        Ok(())
    }

    async fn runtime_internal_tool_finished(
        &self,
        _node: &CompiledNode,
        _call: &Value,
        _usage: &Value,
        receipt: &Value,
        _duration_ms: u64,
    ) -> Result<()> {
        self.finished.lock().unwrap().push(receipt.clone());
        Ok(())
    }
}

#[tokio::test]
async fn real_engine_compacts_then_executes_one_new_tool_and_retains_its_receipt_on_continuation() {
    let mut tool_output = result("");
    tool_output.tool_calls.push(ProviderToolCall {
        id: "new-write".into(),
        name: "update_record".into(),
        arguments: json!({"record_id":42}),
        provider_metadata: json!({}),
    });
    tool_output.finish_reason = Some(ProviderFinishReason::ToolCall);
    let fixture = Fixture::new(
        Runtime::with_provider_results(vec![
            result("Historical writes are complete. <checkpoint-complete>"),
            tool_output,
            result("finished after one new write"),
        ]),
        &[8_600, 1_000, 1_000, 4_000, 4_000],
    )
    .await;
    let node: CompiledNode = serde_json::from_value(json!({
        "node_id":"existing-llm","node_type":"llm","alias":"Existing LLM","container_id":null,
        "dependency_node_ids":[],"downstream_node_ids":[],"bindings":{},"outputs":[],
        "llm_runtime":fixture.compiled,
        "config":{"context_policy":{"integration_context":"enabled","auto_compact":true},
            "llm_parameters":{"items":{
                "max_output_tokens":{"enabled":true,"value":512},
                "requested_context_window":{"enabled":true,"value":WINDOW}
            }}}
    }))
    .unwrap();
    let plan = CompiledPlan {
        flow_id: Uuid::now_v7(),
        source_draft_id: "existing-draft".into(),
        schema_version: "1".into(),
        topological_order: vec![node.node_id.clone()],
        edges: vec![],
        nodes: std::collections::BTreeMap::from([(node.node_id.clone(), node.clone())]),
        compile_issues: vec![],
    };
    let resolved = json!({"history":fixture.input.messages,
        "prompt_messages":[{"role":"system","content":"Keep the user's constraints."}]})
    .as_object()
    .unwrap()
    .clone();
    let mut pool = serde_json::Map::new();
    let tool = Arc::new(CountingInternalTool::default());
    let lifecycle = ToolReceipts::default();
    let context = ExecutionRuntimeContext::from_plan_input(&plan, &pool)
        .unwrap()
        .with_runtime_internal_tool_invoker(tool.clone());
    let execution = execute_llm_node(
        &plan,
        &node,
        &resolved,
        &serde_json::Map::new(),
        &mut pool,
        &context,
        &fixture.invoker,
        &lifecycle,
    )
    .await
    .unwrap();
    assert_eq!(execution.error_payload, None);
    assert_eq!(
        execution.output_payload["text"],
        "finished after one new write"
    );
    assert!(execution.pending_callback.is_none());
    assert_eq!(tool.effects.load(Ordering::SeqCst), 1);
    assert_eq!(execution.metrics_payload["internal_tool_call_count"], 1);
    assert_eq!(lifecycle.started.lock().unwrap().len(), 1);
    assert_eq!(lifecycle.started.lock().unwrap()[0]["id"], "new-write");
    {
        let receipts = lifecycle.finished.lock().unwrap();
        assert_eq!(receipts.len(), 1);
        assert_eq!(receipts[0]["tool_call_id"], "new-write");
        assert_eq!(receipts[0]["execution_status"], "succeeded");
        assert_eq!(receipts[0]["content"], "new-write-committed-receipt");
    }
    assert_eq!(
        execution.debug_payload["runtime_internal_tool_events"][0]["tool_call_id"],
        "new-write"
    );
    {
        let generated = fixture.generated.lock().unwrap();
        assert_eq!(
            generated.len(),
            3,
            "one summary and two actual inference rounds"
        );
        assert!(generated[0].tools.is_empty());
        for invocation in &generated[1..] {
            assert_eq!(
                invocation
                    .messages
                    .iter()
                    .filter(|m| m.role == ProviderMessageRole::User && m.content == RAW_USER)
                    .count(),
                1
            );
            assert!(!serde_json::to_string(&invocation.messages)
                .unwrap()
                .contains("old-write"));
        }
        let resumed = &generated[2].messages;
        assert!(resumed.iter().any(|message| message
            .tool_calls
            .as_ref()
            .is_some_and(|calls| calls.to_string().contains("new-write"))));
        assert!(resumed.iter().any(
            |message| message.tool_call_id.as_deref() == Some("new-write")
                && message.content.contains("new-write-committed-receipt")
        ));
    }
    let events = fixture
        .repository
        .list_runtime_events(fixture.run_id, 0)
        .await
        .unwrap();
    let statuses = events
        .iter()
        .filter(|event| event.event_type == "internal_compaction")
        .map(|event| event.payload["status"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(statuses, vec!["activated", "reused"]);
    assert_eq!(fixture.counts.lock().unwrap().len(), 5);
    assert_eq!(fixture.repository.internal_compaction_candidates().len(), 1);
    assert_eq!(fixture.repository.internal_compaction_run_count(), 1);
}

#[tokio::test]
async fn real_host_fallback_count_accepts_plaintext_reasoning_and_reuses_existing_checkpoint() {
    let fixture = Fixture::new(
        Runtime::with_provider_result(result("normal reasoning follow-up")),
        &[4_000],
    )
    .await;
    let previous = fixture.seed_previous_checkpoint().await;
    let mut input = fixture.input.clone();
    let mut historical_reasoning = message(ProviderMessageRole::Assistant, "A prior answer");
    historical_reasoning.content_blocks = Some(json!([
        {"type":"reasoning","text":"The old result was already committed."},
        {"type":"text","text":"A prior answer"}
    ]));
    input.messages.push(historical_reasoning.clone());
    // The only scripted count was consumed while seeding the old head. The real
    // host must now use its estimator after the runtime reports CountTokens unavailable.
    let output = fixture
        .invoker
        .invoke_resolved_llm(&fixture.compiled, fixture.route.clone(), input)
        .await
        .unwrap();
    let receipt = &output.result.provider_metadata["_1flowbase_upstream_provider_metadata"]
        ["_1flowbase_internal_compaction"];
    assert_eq!(receipt["status"], "reused");
    assert_eq!(receipt["count"]["method"], "generic_estimate");
    assert_eq!(receipt["count"]["coverage"], "complete");
    assert_eq!(receipt["count"]["unknown_block_count"], 0);
    assert_eq!(receipt["count"]["fallback_reason"], "plugin_unavailable");
    assert!(receipt["input_tokens_after"]
        .as_u64()
        .is_some_and(|count| count > 0));
    assert_eq!(fixture.head(), previous);
    assert_eq!(fixture.counts.lock().unwrap().len(), 2);
    let generated = fixture.generated.lock().unwrap();
    assert_eq!(
        generated.len(),
        1,
        "checkpoint reuse must not generate another summary"
    );
    assert_eq!(generated[0].messages.last(), Some(&historical_reasoning));
}

#[path = "internal_compaction/strict.rs"]
mod strict;
