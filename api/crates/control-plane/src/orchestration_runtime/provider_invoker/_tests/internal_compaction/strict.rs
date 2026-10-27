use super::*;
use plugin_framework::provider_contract::ProtocolContextEnvelope;

fn external() -> ProtocolContextEnvelope {
    serde_json::from_value(json!({"source_protocol":"openai_chat","body":{}})).unwrap()
}

#[tokio::test]
async fn resolved_host_preserves_current_user_before_user_role_tool_receipts() {
    use orchestration_runtime::execution_engine::internal_compaction::closed_prefix_ends;
    let mut fixture = Fixture::new(
        Runtime::with_provider_results(vec![
            result("Old action a completed. <checkpoint-complete>"),
            result("normal response"),
        ]),
        &[8_600, 1_000, 1_000, 4_000],
    )
    .await;
    fixture.input.client_protocol_envelope = Some(external());
    fixture.input.messages = vec![message(ProviderMessageRole::User, RAW_USER)];
    for id in ["a", "b", "c"] {
        let mut call = message(ProviderMessageRole::Assistant, "");
        call.content_blocks =
            Some(json!([{"type":"tool_use","id":id,"name":"update_record","input":{}}]));
        let mut result = message(ProviderMessageRole::User, "");
        result.content_blocks =
            Some(json!([{"type":"tool_result","tool_use_id":id,"content":"done once"}]));
        fixture.input.messages.extend([call, result]);
    }
    let output = fixture
        .invoker
        .invoke_resolved_llm(
            &fixture.compiled,
            fixture.route.clone(),
            fixture.input.clone(),
        )
        .await
        .unwrap();
    assert_eq!(
        output.result.final_content.as_deref(),
        Some("normal response")
    );
    let generated = fixture.generated.lock().unwrap();
    assert_eq!(generated.len(), 2);
    let answer = &generated[1];
    assert_eq!(answer.messages[0], fixture.input.messages[0]);
    assert_eq!(
        answer
            .messages
            .iter()
            .filter(|message| message.content == RAW_USER)
            .count(),
        1
    );
    assert_eq!(answer.messages[2..], fixture.input.messages[3..]);
    assert_eq!(
        closed_prefix_ends(&answer.messages).unwrap(),
        vec![1, 2, 4, 6]
    );
    let mut cp: WorkingCheckpoint =
        serde_json::from_value(fixture.head().checkpoint.unwrap()).unwrap();
    assert_eq!(cp.covered_source_hashes.len(), 3);
    assert_eq!(cp.retained_item_refs, vec![0]);
    cp.retained_item_refs.clear();
    assert!(cp.view(&fixture.input.messages, &cp.prefix_hash).is_err());
}

// Controlled provider specification: one token per serialized prompt byte plus
// 32 framing tokens. This is deliberately independent of the production budget.
fn measured_tokens(input: &ProviderInvocationInput) -> u64 {
    serde_json::to_vec(&json!([
        input.system,
        input.messages,
        input.tools,
        input.mcp_bindings,
        input.response_format
    ]))
    .unwrap()
    .len() as u64
        + 32
}

async fn measured_fixture() -> Fixture {
    let mut fixture = Fixture::new(
        Runtime::with_provider_result(result(
            "Goal retained; old actions completed, no replay. <checkpoint-complete>",
        )),
        &[],
    )
    .await;
    let (runtime, counts) = fixture
        .invoker
        .runtime
        .clone()
        .with_count_tokens_measurement(measured_tokens);
    fixture.invoker.runtime = runtime;
    fixture.counts = counts;
    fixture.repository.set_compaction_model_window(
        fixture.compiled.provider_instance_id.parse().unwrap(),
        300_000,
    );
    fixture.route = fixture
        .invoker
        .resolve_llm_route(&fixture.compiled)
        .await
        .unwrap();
    fixture
        .input
        .model_parameters
        .insert("requested_context_window".into(), json!(500_000));
    fixture.input.client_protocol_envelope = Some(external());
    fixture.input.tools.clear();
    fixture.input.messages = (0..8)
        .map(|index| {
            message(
                if index % 2 == 0 {
                    ProviderMessageRole::User
                } else {
                    ProviderMessageRole::Assistant
                },
                &format!("record-{index}:{}", "x".repeat(50_000)),
            )
        })
        .collect();
    fixture.input.messages.push(message(
        ProviderMessageRole::User,
        "Latest exact question: status only.",
    ));
    fixture
}

#[tokio::test]
async fn strict_switch_matrix_through_real_host_has_zero_disabled_work() {
    for assistant in [false, true] {
        for is_external in [false, true] {
            for value in [
                None,
                Some(Value::Null),
                Some(json!(false)),
                Some(json!("true")),
                Some(json!(true)),
            ] {
                let fixture = Fixture::new(
                    Runtime::with_provider_result(result("ordinary provider reply")),
                    &[4_000],
                )
                .await;
                fixture.repository.set_compaction_compatibility_mode(
                    fixture.run_id,
                    assistant.then_some("embedded_assistant"),
                );
                let mut input = fixture.input.clone();
                input.run_context.remove(POLICY_KEY);
                if let Some(value) = &value {
                    input
                        .run_context
                        .insert(POLICY_KEY.into(), json!({"enabled":value}));
                }
                input.client_protocol_envelope = is_external.then(external);
                let enabled = value == Some(json!(true));
                let before = input.clone();
                let output = fixture
                    .invoker
                    .invoke_resolved_llm(&fixture.compiled, fixture.route.clone(), input)
                    .await
                    .unwrap();
                assert_eq!(
                    output.result.final_content.as_deref(),
                    Some("ordinary provider reply")
                );
                assert_eq!(fixture.counts.lock().unwrap().len(), usize::from(enabled));
                assert_eq!(
                    fixture.repository.internal_compaction_observations().len(),
                    usize::from(enabled)
                );
                assert!(fixture
                    .repository
                    .internal_compaction_candidates()
                    .is_empty());
                let generated = fixture.generated.lock().unwrap();
                assert_eq!(generated.len(), 1, "no compaction provider retries");
                assert!(!generated[0].run_context.contains_key(SUMMARY_KEY));
                assert_eq!(generated[0].messages, before.messages);
                if !enabled {
                    assert_eq!(generated[0].model_parameters, before.model_parameters);
                }
            }
        }
    }
}

#[tokio::test]
async fn disabled_does_not_require_window_reserve_or_valid_run() {
    let mut fixture = Fixture::new(Runtime::default(), &[]).await;
    fixture.invoker.flow_run_id = Some(Uuid::now_v7());
    let mut input = fixture.input.clone();
    input.run_context.remove(POLICY_KEY);
    input
        .model_parameters
        .insert("max_tokens".into(), json!("invalid"));
    let original = input.clone();
    assert_eq!(
        fixture
            .invoker
            .apply_internal_compaction(&fixture.compiled, &fixture.route, &mut input, None)
            .await
            .unwrap(),
        None
    );
    assert_eq!(input, original);
    assert!(fixture.counts.lock().unwrap().is_empty());
    assert!(fixture
        .repository
        .internal_compaction_observations()
        .is_empty());
    assert!(fixture
        .repository
        .internal_compaction_candidates()
        .is_empty());
    assert!(fixture.generated.lock().unwrap().is_empty());
}

#[tokio::test]
async fn external_history_over_selected_window_is_folded_in_budgeted_closed_batches() {
    let fixture = measured_fixture().await;
    assert!(measured_tokens(&fixture.input) > 300_000);
    let output = fixture
        .invoker
        .invoke_resolved_llm(
            &fixture.compiled,
            fixture.route.clone(),
            fixture.input.clone(),
        )
        .await
        .unwrap();
    let receipt = &output.result.provider_metadata["_1flowbase_upstream_provider_metadata"]
        ["_1flowbase_internal_compaction"];
    assert_eq!(receipt["window"], 300_000);
    assert_eq!(receipt["threshold"], 255_000);
    assert_eq!(receipt["status"], "activated");
    let batches = receipt["summary"]["batches"].as_array().unwrap();
    assert!(batches.len() >= 2);
    let mut cursor = 0;
    for batch in batches {
        assert_eq!(batch["source_start"], cursor);
        cursor = batch["source_end"].as_u64().unwrap();
        assert!(batch["input_tokens"].as_u64().unwrap() + 4096 + 3000 <= 300_000);
    }
    assert_eq!(cursor, receipt["covered_message_count"].as_u64().unwrap());
    let generated = fixture.generated.lock().unwrap();
    assert_eq!(generated.len(), batches.len() + 1);
    for (index, request) in generated.iter().enumerate() {
        assert!(
            measured_tokens(request)
                + request.model_parameters["max_output_tokens"]
                    .as_u64()
                    .unwrap()
                + 3000
                <= 300_000
        );
        if index < batches.len() {
            assert!(request.tools.is_empty() && request.mcp_bindings.is_empty());
            assert!(request.native_transport.is_none());
            assert!(!request.run_context.contains_key(POLICY_KEY));
            let records: Value = serde_json::from_str(&request.messages[0].content).unwrap();
            let indices: Vec<_> = records["historical_records"]
                .as_array()
                .unwrap()
                .iter()
                .map(|r| r["source_message_index"].as_u64().unwrap())
                .collect();
            assert_eq!(
                indices,
                (batches[index]["source_start"].as_u64().unwrap()
                    ..batches[index]["source_end"].as_u64().unwrap())
                    .collect::<Vec<_>>()
            );
        }
    }
    let final_request = generated.last().unwrap();
    assert_eq!(final_request.messages.last(), fixture.input.messages.last());
    assert!(measured_tokens(final_request) < 255_000);
    assert_eq!(fixture.repository.internal_compaction_candidates().len(), 1);
    assert_eq!(
        fixture.repository.internal_compaction_observations()[0].source,
        serde_json::to_value(&fixture.input.messages).unwrap()
    );
}

#[tokio::test]
async fn indivisible_oversized_history_fails_without_summary_or_checkpoint_publication() {
    let mut fixture = measured_fixture().await;
    fixture.input.messages[0].content = "indivisible".repeat(50_000);
    let error = fixture
        .invoker
        .invoke_resolved_llm(
            &fixture.compiled,
            fixture.route.clone(),
            fixture.input.clone(),
        )
        .await
        .unwrap_err();
    assert!(
        error.to_string().contains("one closed history unit"),
        "{error}"
    );
    assert!(fixture.generated.lock().unwrap().is_empty());
    assert!(fixture
        .repository
        .internal_compaction_candidates()
        .is_empty());
    assert_eq!(fixture.head(), InternalCompactionHead::default());
}

#[tokio::test]
async fn stateless_external_requests_have_distinct_checkpoint_keys() {
    let mut fixture = Fixture::new(Runtime::default(), &[4_000, 4_000]).await;
    fixture.invoker.provider_session_scope = None;
    fixture.input.client_protocol_envelope = Some(external());
    fixture.apply(&mut fixture.input.clone()).await.unwrap();
    // No state is published at this size; the second independent run must still
    // address a different head even with identical application, node and history.
    fixture.invoker.flow_run_id = Some(Uuid::now_v7());
    fixture.apply(&mut fixture.input.clone()).await.unwrap();
    let observations = fixture.repository.internal_compaction_observations();
    assert_eq!(observations.len(), 2);
    assert_ne!(observations[0].session_key, observations[1].session_key);
    assert_eq!(fixture.repository.internal_compaction_heads().len(), 2);
}

#[tokio::test]
async fn partial_tokenizer_tools_use_visible_estimate_but_opaque_content_stays_rejected() {
    use plugin_framework::provider_contract::{
        ProviderCountTokensCoverage, ProviderCountTokensMethod,
    };
    for opaque in [false, true] {
        let mut fixture = Fixture::new(
            Runtime::with_provider_result(result("tool-capable answer")),
            &[],
        )
        .await;
        let partial = ProviderCountTokensResult {
            input_tokens: 123,
            method: ProviderCountTokensMethod::ModelTokenizer,
            coverage: ProviderCountTokensCoverage::Partial,
            unknown_block_count: 1,
            ..Default::default()
        };
        let (runtime, counts) = fixture
            .invoker
            .runtime
            .clone()
            .with_count_tokens_results(vec![partial.clone()]);
        fixture.invoker.runtime = runtime;
        fixture.counts = counts;
        if opaque {
            fixture.input.messages[0].content_blocks =
                Some(json!([{"type":"reasoning_redacted","data":"opaque"}]));
        }
        let response = fixture
            .invoker
            .invoke_resolved_llm(
                &fixture.compiled,
                fixture.route.clone(),
                fixture.input.clone(),
            )
            .await;
        if opaque {
            assert!(response
                .unwrap_err()
                .to_string()
                .contains("complete model input token count unavailable"));
            assert!(fixture.generated.lock().unwrap().is_empty());
        } else {
            let output = response.unwrap();
            let count = &output.result.provider_metadata["_1flowbase_upstream_provider_metadata"]
                ["_1flowbase_internal_compaction"]["count"];
            assert_eq!(count["method"], "generic_estimate");
            assert_eq!(count["coverage"], "complete");
            assert_eq!(
                count["fallback_context"],
                "provider_count_partial_visible_input"
            );
            assert_eq!(
                count["provider_count"],
                serde_json::to_value(partial).unwrap()
            );
            let generated = fixture.generated.lock().unwrap();
            assert_eq!(generated.len(), 1);
            assert_eq!(generated[0].tools, fixture.input.tools);
        }
    }
}

#[tokio::test]
async fn enabled_semantic_summary_and_answer_cannot_reinject_preferred_native_body() {
    let mut fixture = measured_fixture().await;
    fixture.invoker.provider_transport_payload = Some(
        crate::ports::ProviderTransportPayload::openai_responses(json!({
            "model":"gpt-5.4-mini", "input":"ORIGINAL-WIRE-MUST-NOT-REPLAY"
        }))
        .unwrap()
        .preferred(),
    );
    // Even a route that supports native forwarding must use the authoritative
    // semantic view when complete-history compaction is explicitly enabled.
    fixture
        .route
        .runtime_capabilities
        .insert("responses.native_passthrough".into());
    fixture
        .route
        .runtime_capabilities
        .insert("responses.native_output.v1".into());
    fixture
        .invoker
        .invoke_resolved_llm(
            &fixture.compiled,
            fixture.route.clone(),
            fixture.input.clone(),
        )
        .await
        .unwrap();
    let generated = fixture.generated.lock().unwrap();
    assert!(generated.len() >= 3);
    for request in generated.iter() {
        assert!(request.native_transport.is_none());
        assert!(request.previous_response_id.is_none());
        assert!(!serde_json::to_string(&request.messages)
            .unwrap()
            .contains("ORIGINAL-WIRE-MUST-NOT-REPLAY"));
    }
}

#[tokio::test]
async fn second_batch_failure_does_not_publish_a_partial_checkpoint_or_answer() {
    let mut fixture = measured_fixture().await;
    let (runtime, generated) = Runtime::with_provider_results(vec![
        result("First batch complete <checkpoint-complete>"),
        result("truncated second batch"),
    ])
    .capturing_invocations();
    let (runtime, counts) = runtime.with_count_tokens_measurement(measured_tokens);
    fixture.invoker.runtime = runtime;
    fixture.generated = generated;
    fixture.counts = counts;
    let error = fixture
        .invoker
        .invoke_resolved_llm(
            &fixture.compiled,
            fixture.route.clone(),
            fixture.input.clone(),
        )
        .await
        .unwrap_err();
    assert!(
        error.to_string().contains("summary is empty or truncated"),
        "{error}"
    );
    assert_eq!(fixture.generated.lock().unwrap().len(), 2);
    assert!(fixture
        .repository
        .internal_compaction_candidates()
        .is_empty());
    assert_eq!(fixture.head(), InternalCompactionHead::default());
}
