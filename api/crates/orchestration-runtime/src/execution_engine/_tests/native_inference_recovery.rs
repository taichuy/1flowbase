use super::*;
use crate::compiled_plan::{CompiledEdge, CompiledLlmRuntime, CompiledNode};

fn node(id: &str, kind: &str) -> CompiledNode {
    CompiledNode {
        node_id: id.into(),
        node_type: kind.into(),
        alias: id.into(),
        container_id: None,
        dependency_node_ids: vec![],
        downstream_node_ids: vec![],
        bindings: Default::default(),
        outputs: vec![],
        config: json!({}),
        plugin_runtime: None,
        code_runtime: None,
        llm_runtime: (kind == "llm").then(|| CompiledLlmRuntime {
            provider_instance_id: "provider".into(),
            provider_instance_display_name: "fixture".into(),
            provider_code: "openai".into(),
            protocol: "responses".into(),
            model: "model".into(),
            routing: None,
        }),
    }
}
fn routed_plan() -> CompiledPlan {
    let shapes = [
        ("start", "start"),
        ("router", "if_else"),
        ("llm-a", "llm"),
        ("llm-b", "llm"),
        ("llm-c", "llm"),
        ("llm-d", "llm"),
        ("aggregate", "variable_aggregator"),
        ("answer-a", "answer"),
        ("answer-b", "answer"),
    ];
    let pairs = [
        ("start", "router"),
        ("router", "llm-a"),
        ("router", "llm-b"),
        ("router", "llm-c"),
        ("router", "llm-d"),
        ("llm-a", "aggregate"),
        ("llm-b", "aggregate"),
        ("llm-c", "aggregate"),
        ("llm-d", "aggregate"),
        ("aggregate", "answer-a"),
        ("aggregate", "answer-b"),
    ];
    CompiledPlan {
        flow_id: uuid::Uuid::nil(),
        source_draft_id: "fixture".into(),
        schema_version: "1".into(),
        topological_order: shapes.iter().map(|(id, _)| id.to_string()).collect(),
        nodes: shapes
            .iter()
            .map(|(id, kind)| (id.to_string(), node(id, kind)))
            .collect(),
        compile_issues: vec![],
        edges: pairs
            .iter()
            .enumerate()
            .map(|(i, (source, target))| CompiledEdge {
                edge_id: i.to_string(),
                source: source.to_string(),
                target: target.to_string(),
                source_handle: None,
                target_handle: None,
            })
            .collect(),
    }
}
#[test]
fn actual_four_llm_shape_admits_each_selected_llm_without_other_routes() {
    let plan = routed_plan();
    for selected in ["llm-a", "llm-b", "llm-c", "llm-d"] {
        let scope = recovery_scope(&plan, selected).unwrap();
        assert_eq!(
            scope,
            BTreeSet::from([
                selected.into(),
                "aggregate".into(),
                "answer-a".into(),
                "answer-b".into()
            ])
        );
    }
    assert!(validate_native_inference_recovery_scope(&plan, "router").is_err());
}
#[test]
fn downstream_side_effect_second_llm_and_backward_edge_are_rejected() {
    for kind in [
        "llm",
        "sql",
        "http_request",
        "code",
        "plugin_node",
        "variable_assigner",
    ] {
        let mut plan = routed_plan();
        plan.nodes
            .insert("aggregate".into(), node("aggregate", kind));
        assert!(
            validate_native_inference_recovery_scope(&plan, "llm-b").is_err(),
            "{kind}"
        );
    }
    let mut plan = routed_plan();
    plan.edges.push(CompiledEdge {
        edge_id: "cycle".into(),
        source: "aggregate".into(),
        target: "llm-b".into(),
        source_handle: None,
        target_handle: None,
    });
    assert!(validate_native_inference_recovery_scope(&plan, "llm-b").is_err());
}

#[derive(Default)]
struct CapturingInference {
    calls: std::sync::Mutex<Vec<ProviderInvocationInput>>,
}
#[async_trait::async_trait]
impl ProviderInvoker for CapturingInference {
    async fn invoke_llm(
        &self,
        _runtime: &CompiledLlmRuntime,
        input: ProviderInvocationInput,
    ) -> Result<ProviderInvocationOutput> {
        self.calls.lock().unwrap().push(input);
        Ok(ProviderInvocationOutput {
            events: vec![
                ProviderStreamEvent::TextDelta {
                    delta: "recovered answer".into(),
                },
                ProviderStreamEvent::Finish {
                    reason: ProviderFinishReason::Stop,
                },
            ],
            result: ProviderInvocationResult {
                final_content: Some("recovered answer".into()),
                finish_reason: Some(ProviderFinishReason::Stop),
                ..Default::default()
            },
            first_token_at: None,
            time_to_first_token_ms: None,
        })
    }
}
#[async_trait::async_trait]
impl CapabilityInvoker for CapturingInference {
    async fn invoke_capability_node(
        &self,
        _runtime: &crate::compiled_plan::CompiledPluginRuntime,
        _config: Value,
        _input: Value,
    ) -> Result<CapabilityInvocationOutput> {
        panic!("inference recovery must not run capability side effects")
    }
}
#[async_trait::async_trait]
impl CodeInvoker for CapturingInference {
    async fn invoke_code_node(
        &self,
        _runtime: &crate::compiled_plan::CompiledCodeRuntime,
        _config: Value,
        _input: Value,
    ) -> Result<CodeInvocationOutput> {
        panic!("inference recovery must not run code or tool side effects")
    }
}

fn executable_routed_plan() -> CompiledPlan {
    use crate::compiled_plan::{CompiledBinding, CompiledOutput};
    let mut plan = routed_plan();
    let output = |key: &str| CompiledOutput {
        key: key.into(),
        title: key.into(),
        value_type: "string".into(),
        selector: vec![],
        json_schema: None,
    };
    for id in ["llm-a", "llm-b", "llm-c", "llm-d"] {
        let node = plan.nodes.get_mut(id).unwrap();
        node.outputs = vec![output("text")];
        node.bindings.insert("prompt_messages".into(),CompiledBinding {i18n_text_ref:None,
            kind:"prompt_messages".into(),selector_paths:vec![vec!["start".into(),"query".into()]],
            raw_value:json!([{"id":"user-1","role":"user","content":{"kind":"templated_text","value":"{{start.query}}"}}])});
    }
    let aggregate = plan.nodes.get_mut("aggregate").unwrap();
    aggregate.outputs = vec![output("text")];
    aggregate.bindings.insert("groups".into(),CompiledBinding {i18n_text_ref:None,
        kind:"variable_groups".into(),selector_paths:["llm-a","llm-b","llm-c","llm-d"].iter()
            .map(|id| vec![id.to_string(),"text".into()]).collect(),
        raw_value:json!([{"key":"text","valueType":"string","candidates":[["llm-a","text"],["llm-b","text"],["llm-c","text"],["llm-d","text"]]}])});
    for id in ["answer-a", "answer-b"] {
        let answer = plan.nodes.get_mut(id).unwrap();
        answer.outputs = vec![output("answer")];
        answer.bindings.insert(
            "answer_template".into(),
            CompiledBinding {
                i18n_text_ref: None,
                kind: "selector".into(),
                selector_paths: vec![vec!["aggregate".into(), "text".into()]],
                raw_value: json!(["aggregate", "text"]),
            },
        );
    }
    plan
}

#[tokio::test]
async fn real_engine_recovers_one_of_four_llms_and_inherits_budget_without_reconsuming_tools() {
    let plan = executable_routed_plan();
    let deadline =
        (time::OffsetDateTime::now_utc().unix_timestamp_nanos() / 1_000_000) as i64 + 60_000;
    let checkpoint = CheckpointSnapshot {
        next_node_index: 3,
        active_node_ids: vec!["start".into(), "router".into(), "llm-b".into()],
        variable_pool: serde_json::from_value(json!({"start":{"query":"frozen original question"},
            "env":{"must_remain":"original"},
            "llm-b":{"__llm_tool_callback":{"response_id":"old-cursor-must-not-replay",
                "history":[{"role":"tool","content":"already-consumed-result"}],
                "pending_tool_calls":[{"id":"consumed-call"}]}},
            "sys":{"native_inference_recovery":{"node_id":"llm-b","remaining_attempts":1,
                "absolute_deadline_unix_ms":deadline}}}))
        .unwrap(),
    };
    let before = checkpoint.clone();
    let invoker = CapturingInference::default();
    let result = recover_native_inference_with_runtime_context_and_lifecycle(
        &plan,
        &checkpoint,
        "llm-b",
        ExecutionRuntimeContext::default(),
        &invoker,
        &NoopExecutionLifecycle,
    )
    .await
    .unwrap();
    assert_eq!(result.stop_reason, ExecutionStopReason::Completed);
    assert_eq!(
        result
            .node_traces
            .iter()
            .map(|trace| trace.node_id.as_str())
            .collect::<Vec<_>>(),
        vec!["llm-b", "aggregate", "answer-a", "answer-b"]
    );
    assert_eq!(
        result.variable_pool["answer-a"]["answer"],
        "recovered answer"
    );
    assert_eq!(
        result.variable_pool["env"],
        json!({"must_remain":"original"})
    );
    assert_eq!(checkpoint, before, "source callback snapshot is immutable");
    let calls = invoker.calls.lock().unwrap();
    assert_eq!(
        calls.len(),
        1,
        "other LLM branches and consumed tools must not be invoked"
    );
    assert_eq!(calls[0].trace_context["node_id"], "llm-b");
    assert!(calls[0].previous_response_id.is_none());
    assert!(!serde_json::to_string(&calls[0].messages)
        .unwrap()
        .contains("already-consumed-result"));
    let directive: extension_contracts::provider_contract::ProviderRecoveryDirective =
        serde_json::from_value(
            calls[0].run_context
                [extension_contracts::provider_contract::PROVIDER_RECOVERY_DIRECTIVE_CONTEXT_KEY]
                .clone(),
        )
        .unwrap();
    assert_eq!(
        directive.policy.budget().absolute_deadline_unix_ms,
        deadline
    );
    assert_eq!(directive.policy.budget().max_inner_attempts, 1);
    drop(calls);
    let mut parallel = checkpoint;
    parallel.active_node_ids.push("llm-c".into());
    assert!(recover_native_inference_with_runtime_context_and_lifecycle(
        &plan,
        &parallel,
        "llm-b",
        ExecutionRuntimeContext::default(),
        &invoker,
        &NoopExecutionLifecycle
    )
    .await
    .is_err());
    assert_eq!(
        invoker.calls.lock().unwrap().len(),
        1,
        "invalid parallel scope rejected before inference"
    );
}
