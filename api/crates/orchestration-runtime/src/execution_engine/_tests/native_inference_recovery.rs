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
            scope.node_ids,
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

#[test]
fn indexed_recovery_checks_full_large_tail_and_edgeless_fallback() {
    let count = 1400;
    let mut plan = routed_plan();
    plan.nodes = (0..count)
        .map(|i| {
            let id = format!("tail-{i:04}");
            let mut node = node(&id, if i == 0 { "llm" } else { "answer" });
            if i + 1 < count {
                node.downstream_node_ids.push(format!("tail-{:04}", i + 1));
            }
            (id, node)
        })
        .collect();
    plan.topological_order = (0..count).map(|i| format!("tail-{i:04}")).collect();
    plan.edges = (1..count)
        .map(|i| CompiledEdge {
            edge_id: i.to_string(),
            source: format!("tail-{:04}", i - 1),
            target: format!("tail-{i:04}"),
            source_handle: Some("error".into()),
            target_handle: None,
        })
        .collect();
    let expected = plan.topological_order.iter().cloned().collect();
    {
        let scope = recovery_scope(&plan, "tail-0000").unwrap();
        assert_eq!(scope.node_ids, expected);
        for (index, id) in plan.topological_order.iter().enumerate() {
            assert_eq!(scope.positions.get(id.as_str()), Some(&index));
        }
        assert_eq!(scope.positions.get("missing"), None);
    }
    plan.edges.clear();
    assert_eq!(
        recovery_scope(&plan, "tail-0000").unwrap().node_ids,
        expected
    );
    // The edge-less fallback still inspects an inadmissible late side effect.
    plan.nodes.get_mut("tail-1399").unwrap().node_type = "sql".into();
    assert_eq!(
        recovery_scope(&plan, "tail-0000")
            .err()
            .unwrap()
            .to_string(),
        "native_recovery_scope_unsupported"
    );
}

#[test]
fn recovery_keeps_error_edges_and_rejects_ambiguous_positions() {
    let mut plan = routed_plan();
    plan.nodes
        .insert("side-effect".into(), node("side-effect", "sql"));
    plan.topological_order.push("side-effect".into());
    plan.edges.push(CompiledEdge {
        edge_id: "error-side-effect".into(),
        source: "llm-b".into(),
        target: "side-effect".into(),
        source_handle: Some("error".into()),
        target_handle: None,
    });
    assert_eq!(
        recovery_scope(&plan, "llm-b").err().unwrap().to_string(),
        "native_recovery_scope_unsupported"
    );
    plan.topological_order.push("llm-b".into());
    assert_eq!(
        recovery_scope(&plan, "llm-b").err().unwrap().to_string(),
        "native_recovery_scope_invalid"
    );
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
        container_state: None,
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
    {
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
    }
    let mut missing = checkpoint.clone();
    missing.active_node_ids.push("unknown-active-node".into());
    let missing_error = recover_native_inference_with_runtime_context_and_lifecycle(
        &plan,
        &missing,
        "llm-b",
        ExecutionRuntimeContext::default(),
        &invoker,
        &NoopExecutionLifecycle,
    )
    .await
    .unwrap_err();
    assert_eq!(
        missing_error.to_string(),
        "native_recovery_parallel_scope_unsupported"
    );
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

struct ExactReplayInference {
    native: bool,
    authorize: bool,
    typed_receipt: bool,
    error_return: bool,
    resolutions: std::sync::atomic::AtomicUsize,
    pipeline_calls: std::sync::atomic::AtomicUsize,
    pipeline_counts_at_send: std::sync::Mutex<Vec<usize>>,
    calls: std::sync::Mutex<Vec<(CompiledLlmRuntime, usize, ProviderInvocationInput)>>,
}

#[async_trait::async_trait]
impl ProviderInvoker for ExactReplayInference {
    async fn pipeline_provider_input(
        &self,
        mut input: ProviderInvocationInput,
    ) -> std::result::Result<
        crate::provider_input_pipeline::ProviderInputPipelineOutput,
        crate::provider_input_pipeline::ProviderInputPipelineError,
    > {
        let generation = self
            .pipeline_calls
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        // A legal host pipeline can depend on live state. These extensions deliberately change
        // on every build, including the routing probe, so replaying by rebuilding cannot pass.
        input.client_protocol_envelope = Some(
            serde_json::from_value(json!({
                "source_protocol":"openai_responses",
                "body":{"future_extension":{"generation":generation}},
                "headers":{"conversation":[format!("header-{generation}")]},
                "query":{"conversation":[format!("query-{generation}")]}
            }))
            .unwrap(),
        );
        input.request_context.end_user_reference = Some(format!("end-user-{generation}"));
        input
            .model_parameters
            .insert("fixture_control".into(), json!(generation));
        input
            .run_context
            .insert("fixture_business_snapshot".into(), json!(generation));
        input.messages.push(ProviderMessage {
            role: ProviderMessageRole::User,
            content: format!("pipeline context {generation}"),
            name: None,
            tool_call_id: None,
            is_error: None,
            tool_calls: None,
            content_blocks: None,
        });
        Ok(crate::provider_input_pipeline::ProviderInputPipelineOutput::unchanged(input))
    }

    async fn resolve_main_llm_routing(
        &self,
        runtime: &CompiledLlmRuntime,
        _node_id: &str,
    ) -> Result<Option<ResolvedMainLlmRouting>> {
        let index = self
            .resolutions
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let mut selected = runtime.clone();
        selected.provider_instance_id = format!("provider-selection-{index}");
        selected.model = format!("model-selection-{index}");
        Ok(Some(ResolvedMainLlmRouting {
            candidates: vec![ResolvedMainLlmRouteCandidate {
                runtime: selected,
                route: ResolvedProviderRoute::new(BTreeSet::new(), index),
            }],
            distribution_rule: crate::compiled_plan::LlmDistributionRule::None,
            distribution_key: None,
        }))
    }

    fn prepare_selected_llm_input(
        &self,
        _runtime: &CompiledLlmRuntime,
        _route: &ResolvedProviderRoute,
        input: &mut ProviderInvocationInput,
    ) -> Result<ai_native_recovery::RecoveryInputMode> {
        if self.native {
            input.native_transport = Some(
                extension_contracts::provider_contract::ProviderNativeTransport {
                    protocol: "openai_responses".into(),
                    wire_body: json!({"input":[{"type":"reasoning","encrypted_content":"opaque"},
                    {"role":"user","content":"frozen question"}]}),
                    digest: "fixture-owned-native-digest".into(),
                    size_bytes: 123,
                },
            );
            Ok(ai_native_recovery::RecoveryInputMode::NativeOpaque)
        } else {
            Ok(ai_native_recovery::RecoveryInputMode::SemanticMapped)
        }
    }

    fn permits_selected_native_exact_replay(&self, _input: &ProviderInvocationInput) -> bool {
        // This fixture stands in for the trusted owner; real snapshot checks are exercised
        // by control-plane's exact_native_replay_is_authorized_only_by_the_owned_matching_snapshot.
        self.authorize
    }

    async fn invoke_resolved_llm(
        &self,
        runtime: &CompiledLlmRuntime,
        route: ResolvedProviderRoute,
        input: ProviderInvocationInput,
    ) -> Result<ProviderInvocationOutput> {
        use extension_contracts::provider_contract::{
            CommitLevel, ProviderRecoveryDirective, ProviderRecoveryReceipt, RecoveryDisposition,
            RecoveryReason, RecoveryTransport, SocketIncarnation,
            PROVIDER_RECOVERY_DIRECTIVE_CONTEXT_KEY,
        };
        let directive: ProviderRecoveryDirective = serde_json::from_value(
            input.run_context[PROVIDER_RECOVERY_DIRECTIVE_CONTEXT_KEY].clone(),
        )?;
        self.pipeline_counts_at_send.lock().unwrap().push(
            self.pipeline_calls
                .load(std::sync::atomic::Ordering::SeqCst),
        );
        let call_index = {
            let mut calls = self.calls.lock().unwrap();
            let index = calls.len();
            calls.push((
                runtime.clone(),
                *route.invocation_pin::<usize>().unwrap(),
                input,
            ));
            index
        };
        if call_index > 0 {
            return Ok(ProviderInvocationOutput {
                events: vec![
                    ProviderStreamEvent::OutputItem {
                        phase:
                            extension_contracts::provider_contract::ProviderOutputItemPhase::Done,
                        output_index: 0,
                        item: json!({"type":"message","id":"msg_replayed","role":"assistant",
                        "status":"completed","content":[{"type":"output_text","text":"recovered answer"}]}),
                    },
                    ProviderStreamEvent::Finish {
                        reason: ProviderFinishReason::Stop,
                    },
                ],
                result: ProviderInvocationResult {
                    response_id: Some("resp_replayed".into()),
                    final_content: Some("recovered answer".into()),
                    finish_reason: Some(ProviderFinishReason::Stop),
                    ..Default::default()
                },
                first_token_at: None,
                time_to_first_token_ms: None,
            });
        }
        let mut result = ProviderInvocationResult {
            finish_reason: Some(ProviderFinishReason::Error),
            provider_metadata: json!({}),
            ..Default::default()
        };
        if self.typed_receipt {
            result
                .set_recovery_receipt(ProviderRecoveryReceipt {
                    attempt: 0,
                    transport: RecoveryTransport::AiNativeWebSocket,
                    transport_epoch: directive.transport_epoch,
                    socket_incarnation: Some(SocketIncarnation::new(1).unwrap()),
                    commit_level: CommitLevel::LifecycleOnly,
                    disposition: RecoveryDisposition::LogicalInvocationRetry,
                    reason: RecoveryReason::TransportDisconnected,
                })
                .map_err(anyhow::Error::msg)?;
        }
        if self.error_return {
            return Err(ExtensionContractError::runtime(
                ProviderRuntimeError::normalize("invoke", "transport disconnected", None)
                    .with_provider_details(result.provider_metadata),
            )
            .into());
        }
        Ok(ProviderInvocationOutput {
            events: vec![ProviderStreamEvent::Finish {
                reason: ProviderFinishReason::Error,
            }],
            result,
            first_token_at: None,
            time_to_first_token_ms: None,
        })
    }

    async fn invoke_llm(
        &self,
        _runtime: &CompiledLlmRuntime,
        _input: ProviderInvocationInput,
    ) -> Result<ProviderInvocationOutput> {
        panic!("engine must invoke the selected route")
    }
}

async fn run_exact_replay_fixture(
    native: bool,
    authorize: bool,
    typed_receipt: bool,
    configured: bool,
    error_return: bool,
) -> ExactReplayInference {
    let mut plan = executable_routed_plan();
    let node = plan.nodes.get_mut("llm-b").unwrap();
    node.config = json!({"retry_enabled": configured, "max_retries":1, "retry_interval_ms":0});
    let invoker = ExactReplayInference {
        native,
        authorize,
        typed_receipt,
        error_return,
        resolutions: std::sync::atomic::AtomicUsize::new(0),
        pipeline_calls: std::sync::atomic::AtomicUsize::new(0),
        pipeline_counts_at_send: std::sync::Mutex::new(Vec::new()),
        calls: std::sync::Mutex::new(Vec::new()),
    };
    let pool = serde_json::from_value(json!({"start":{"query":"frozen question"}})).unwrap();
    // This fixture invokes the provider round directly, after the outer engine's binding
    // resolution/rendering boundary. Supply that boundary's output for semantic requests too.
    let rendered_templates = serde_json::from_value(json!({"prompt_messages":[{
        "id":"user-1", "role":"user", "content":"frozen question"
    }]}))
    .unwrap();
    let execution = execute_llm_node_provider_round(
        &plan,
        &plan.nodes["llm-b"],
        &Map::new(),
        &rendered_templates,
        &pool,
        &ExecutionRuntimeContext::default(),
        &invoker,
    )
    .await
    .unwrap();
    let should_recover = if native {
        authorize && typed_receipt
    } else {
        configured || typed_receipt
    };
    assert_eq!(
        execution.error_payload.is_none(), should_recover,
        "native={native} authorize={authorize} typed={typed_receipt} configured={configured} error_return={error_return}; error={:?}; metrics={}; calls={}",
        execution.error_payload, execution.metrics_payload, invoker.calls.lock().unwrap().len(),
    );
    if should_recover {
        assert_eq!(execution.output_payload["text"], "recovered answer");
    }
    invoker
}

#[tokio::test]
async fn engine_native_exact_transport_replay_reuses_selected_route_runtime_and_payload() {
    use extension_contracts::provider_contract::{
        ProviderRecoveryDirective, RecoveryPolicy, PROVIDER_RECOVERY_DIRECTIVE_CONTEXT_KEY,
    };
    let invoker = run_exact_replay_fixture(true, true, true, false, false).await;
    let calls = invoker.calls.lock().unwrap();
    assert_eq!(
        calls.len(),
        2,
        "the owned native request must reach the automatic retry"
    );
    assert_eq!(
        invoker
            .resolutions
            .load(std::sync::atomic::Ordering::SeqCst),
        1,
        "automatic retry cannot rerun account/provider distribution"
    );
    assert_eq!(
        calls[0].0.provider_instance_id,
        calls[1].0.provider_instance_id
    );
    assert_eq!(calls[0].0.model, calls[1].0.model);
    assert_eq!(calls[0].1, calls[1].1, "same route invocation pin");
    let native0 = calls[0].2.native_transport.as_ref().unwrap();
    let native1 = calls[1].2.native_transport.as_ref().unwrap();
    assert_eq!(native0.wire_body, native1.wire_body);
    assert_eq!(native0.digest, native1.digest);
    assert_eq!(native0.protocol, native1.protocol);
    assert_eq!(native0.size_bytes, native1.size_bytes);
    let directives = calls
        .iter()
        .map(|(_, _, input)| {
            serde_json::from_value::<ProviderRecoveryDirective>(
                input.run_context[PROVIDER_RECOVERY_DIRECTIVE_CONTEXT_KEY].clone(),
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    assert!(directives
        .iter()
        .all(|directive| matches!(directive.policy, RecoveryPolicy::NativeOpaque { .. })));
    assert_ne!(directives[0].transport_epoch, directives[1].transport_epoch);
    assert_eq!(
        directives[0].policy.budget().absolute_deadline_unix_ms,
        directives[1].policy.budget().absolute_deadline_unix_ms
    );
    let pipeline_counts = invoker.pipeline_counts_at_send.lock().unwrap();
    assert!(
        pipeline_counts[0] >= 2,
        "routing probe and first real invocation both run the pipeline"
    );
    assert_eq!(
        pipeline_counts[0], pipeline_counts[1],
        "automatic send must not rerun the live pipeline"
    );
    let mut first = calls[0].2.clone();
    let mut replay = calls[1].2.clone();
    assert_ne!(
        first.trace_context["provider_invocation_id"],
        replay.trace_context["provider_invocation_id"]
    );
    assert_eq!(first.trace_context["provider_attempt_index"], "0");
    assert_eq!(replay.trace_context["provider_attempt_index"], "1");
    // Audit the complete input received at the provider boundary. Only these host-owned
    // per-attempt accounting/recovery fields may change; envelope body/query/headers,
    // canonical context, model controls and business run-context must remain identical.
    for input in [&mut first, &mut replay] {
        input.trace_context.remove("provider_invocation_id");
        input.trace_context.remove("provider_attempt_index");
        input
            .run_context
            .remove(PROVIDER_RECOVERY_DIRECTIVE_CONTEXT_KEY);
    }
    assert_eq!(first, replay);
    let envelope = first.client_protocol_envelope.as_ref().unwrap();
    assert!(envelope.body.contains_key("future_extension"));
    assert!(envelope.headers.contains_key("conversation"));
    assert!(envelope.query.contains_key("conversation"));
}

#[tokio::test]
async fn engine_native_replay_capability_never_enables_configured_retry() {
    let denied = run_exact_replay_fixture(true, false, true, true, false).await;
    assert_eq!(
        denied.calls.lock().unwrap().len(),
        1,
        "opaque mode without owner proof stays denied"
    );
    let unreported = run_exact_replay_fixture(true, true, false, true, false).await;
    assert_eq!(
        unreported.calls.lock().unwrap().len(),
        1,
        "missing typed receipt cannot fall into configured native retry"
    );
    let semantic = run_exact_replay_fixture(false, false, false, true, false).await;
    assert_eq!(
        semantic.calls.lock().unwrap().len(),
        2,
        "existing semantic configured retry remains available"
    );
    assert_eq!(
        semantic
            .resolutions
            .load(std::sync::atomic::Ordering::SeqCst),
        2,
        "configured retry retains configured route selection policy"
    );
    assert!(
        semantic.calls.lock().unwrap().iter().all(|(_, _, input)| {
            input.messages.iter().any(|message| {
                message.role == ProviderMessageRole::User && message.content == "frozen question"
            })
        }),
        "both configured attempts must receive the resolved canonical prompt"
    );
    let counts = semantic.pipeline_counts_at_send.lock().unwrap();
    assert!(
        counts[1] > counts[0],
        "configured retry must rebuild through the live pipeline"
    );
    let calls = semantic.calls.lock().unwrap();
    assert_ne!(
        calls[0].2.client_protocol_envelope,
        calls[1].2.client_protocol_envelope
    );
    assert_ne!(
        calls[0].2.run_context["fixture_business_snapshot"],
        calls[1].2.run_context["fixture_business_snapshot"]
    );
}

#[tokio::test]
async fn engine_native_exact_replay_also_recovers_a_typed_invocation_error() {
    let invoker = run_exact_replay_fixture(true, true, true, false, true).await;
    let calls = invoker.calls.lock().unwrap();
    assert_eq!(calls.len(), 2);
    assert_eq!(
        invoker
            .resolutions
            .load(std::sync::atomic::Ordering::SeqCst),
        1
    );
    assert_eq!(calls[0].1, calls[1].1);
}

#[test]
fn provider_invoker_defaults_cannot_authorize_native_snapshot_replay() {
    let mut input = ProviderInvocationInput::default();
    input
        .required_capabilities
        .insert(ProviderInvocationCapability::ResponsesNativePassthrough);
    assert!(!CapturingInference::default().permits_selected_native_exact_replay(&input));
}
