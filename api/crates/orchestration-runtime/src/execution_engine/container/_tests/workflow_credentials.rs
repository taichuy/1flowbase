use super::*;

struct RecoveringInference {
    predecessor: uuid::Uuid,
    successor: uuid::Uuid,
    calls: Mutex<Vec<ProviderInvocationInput>>,
    committed_effects: AtomicUsize,
}

#[async_trait]
impl ProviderInvoker for RecoveringInference {
    async fn invoke_llm(
        &self,
        _: &CompiledLlmRuntime,
        input: ProviderInvocationInput,
    ) -> Result<ProviderInvocationOutput> {
        let mut calls = self.calls.lock().unwrap();
        let run = if calls.is_empty() {
            self.predecessor
        } else {
            self.successor
        };
        let messages = serde_json::to_string(&input.messages).unwrap();
        for subject in [
            WorkflowCredentialSubject::Creator,
            WorkflowCredentialSubject::Executor,
        ] {
            assert!(
                messages.contains(&subject.reference(run)),
                "round read a stale credential: {messages}"
            );
        }
        assert!(messages.contains("frozen-environment"));
        calls.push(input);
        if calls.len() == 1 {
            bail!("controlled recoverable inference failure");
        }
        Ok(ProviderInvocationOutput {
            events: vec![ProviderStreamEvent::TextDelta {
                delta: "recovered".into(),
            }],
            result: ProviderInvocationResult {
                final_content: Some("recovered".into()),
                finish_reason: Some(ProviderFinishReason::Stop),
                ..Default::default()
            },
            first_token_at: None,
            time_to_first_token_ms: None,
        })
    }
}

#[async_trait]
impl CapabilityInvoker for RecoveringInference {
    async fn invoke_capability_node(
        &self,
        _: &CompiledPluginRuntime,
        _: Value,
        _: Value,
    ) -> Result<CapabilityInvocationOutput> {
        unreachable!("no plugin in this fixture")
    }
}

#[async_trait]
impl CodeInvoker for RecoveringInference {
    async fn invoke_code_node(
        &self,
        _: &CompiledCodeRuntime,
        _: Value,
        _: Value,
    ) -> Result<CodeInvocationOutput> {
        let effect = self.committed_effects.fetch_add(1, Ordering::SeqCst) + 1;
        Ok(CodeInvocationOutput {
            output_payload: json!({"value":effect}),
            console_logs: vec![],
        })
    }
}

#[tokio::test]
async fn workflow_credentials_native_successor_refreshes_nested_rounds_without_replaying_effects() {
    let predecessor = uuid::Uuid::from_u128(1001);
    let successor = uuid::Uuid::from_u128(1002);
    let mut outer = container("outer", "iteration", None, 1);
    outer
        .bindings
        .insert("output".into(), selector(&["inner", "result"]));
    let mut inner = container("inner", "iteration", Some("outer"), 1);
    inner
        .bindings
        .insert("output".into(), selector(&["llm", "text"]));
    let effect = code("effect", "inner");
    let mut llm = node("llm", "llm", Some("inner"));
    llm.llm_runtime = Some(CompiledLlmRuntime {
        provider_instance_id: "fixture".into(),
        provider_instance_display_name: "fixture".into(),
        provider_code: "openai".into(),
        protocol: "responses".into(),
        model: "fixture".into(),
        routing: None,
    });
    llm.outputs = vec![output("text", "string")];
    llm.bindings.insert("prompt_messages".into(), CompiledBinding {
        kind:"prompt_messages".into(), i18n_text_ref:None,
        selector_paths:vec![vec!["sys".into(),"creator_credential".into()],vec!["sys".into(),"executor_credential".into()],vec!["env".into(),"frozen".into()]],
        raw_value:json!([{"id":"user","role":"user","content":{"kind":"templated_text","value":"{{sys.creator_credential}} {{sys.executor_credential}} {{env.frozen}}"}}]),
    });
    let plan = plan(vec![outer, inner, effect, llm]);
    let invoker = RecoveringInference {
        predecessor,
        successor,
        calls: Mutex::new(vec![]),
        committed_effects: AtomicUsize::new(0),
    };
    let ledger = Ledger::default();
    // Protocol context is host-owned and normalized out of untrusted run input.
    // Supply the same ephemeral host context on initial execution and recovery.
    let runtime_context = ExecutionRuntimeContext::default().with_ephemeral_protocol_context(
        json!({"locator":"frozen-protocol"}),
        serde_json::from_value(json!({"source_protocol":"openai_responses"})).unwrap(),
    );
    let mut input = json!({"source":{"items":[1]},"env":{"frozen":"frozen-environment"}});
    WorkflowCredentialReferences::for_run(predecessor)
        .inject_into_pool(input.as_object_mut().unwrap());
    let failed = start_flow_debug_run_with_runtime_context_and_lifecycle(
        &plan,
        &input,
        runtime_context.clone(),
        &invoker,
        &ledger,
    )
    .await
    .unwrap();
    assert!(matches!(failed.stop_reason, ExecutionStopReason::Failed(_)));
    assert_eq!(invoker.committed_effects.load(Ordering::SeqCst), 1);

    // The successor consumes the predecessor's real durable transition snapshot.
    let predecessor_checkpoint = snapshot(&ledger);
    let original = predecessor_checkpoint.clone();
    let mut checkpoint = predecessor_checkpoint.clone();
    let deadline =
        (time::OffsetDateTime::now_utc().unix_timestamp_nanos() / 1_000_000) as i64 + 60_000;
    checkpoint.variable_pool["sys"]["native_inference_recovery"] =
        json!({"node_id":"llm","remaining_attempts":1,"absolute_deadline_unix_ms":deadline});
    checkpoint
        .container_state
        .as_mut()
        .unwrap()
        .root_variable_pool = checkpoint.variable_pool.clone();
    WorkflowCredentialReferences::for_run(successor).inject_into_snapshot(&mut checkpoint);
    let state = checkpoint.container_state.as_ref().unwrap();
    assert_eq!(
        state.frames.len(),
        2,
        "both already-started nested containers survive"
    );
    for pool in std::iter::once(&checkpoint.variable_pool)
        .chain(std::iter::once(&state.root_variable_pool))
        .chain(
            state
                .frames
                .values()
                .flat_map(|frame| frame.rounds.values().map(|round| &round.variable_pool)),
        )
    {
        assert_eq!(
            pool["sys"]["creator_credential"],
            WorkflowCredentialSubject::Creator.reference(successor)
        );
        assert_eq!(
            pool["sys"]["executor_credential"],
            WorkflowCredentialSubject::Executor.reference(successor)
        );
    }
    let recovered = recover_native_inference_with_runtime_context_and_lifecycle(
        &plan,
        &checkpoint,
        "llm",
        runtime_context,
        &invoker,
        &ledger,
    )
    .await
    .unwrap();
    assert_eq!(recovered.stop_reason, ExecutionStopReason::Completed);
    assert_eq!(
        recovered.variable_pool["outer"]["result"],
        json!([["recovered"]])
    );
    assert_eq!(invoker.calls.lock().unwrap().len(), 2);
    assert_eq!(
        invoker.committed_effects.load(Ordering::SeqCst),
        1,
        "committed upstream effect must not replay"
    );
    assert_eq!(
        recovered.variable_pool["env"],
        json!({"frozen":"frozen-environment"})
    );
    assert_eq!(
        recovered.variable_pool["sys"]["protocol_context"],
        json!({"locator":"frozen-protocol"})
    );
    assert_eq!(
        recovered.variable_pool["sys"]["native_inference_recovery"]["remaining_attempts"],
        1
    );
    assert_eq!(
        predecessor_checkpoint, original,
        "predecessor checkpoint is immutable"
    );
}
