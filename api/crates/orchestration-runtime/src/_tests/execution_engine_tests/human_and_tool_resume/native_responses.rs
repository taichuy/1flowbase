use super::*;
use extension_contracts::provider_contract::ProviderOutputItemPhase;

// Each client-owned native sampling request has an independent workflow lifetime.
#[tokio::test]
async fn native_three_tool_rounds_complete_independent_invocations() {
    let plan = llm_answer_plan();
    let outputs = (0..4)
        .map(|round| {
            let text = if round < 3 { "need tools" } else { "finished" };
            let mut items = vec![
                json!({"id":format!("msg_{round}"),"type":"message","role":"assistant",
            "content":[{"type":"output_text","text":text}]}),
            ];
            let mut result = if round < 3 {
                items.push(
                    json!({"id":format!("item_{round}"),"type":"custom_tool_call",
                "call_id":format!("call_{round}"),"name":"exec","input":format!("read {round}")}),
                );
                tool_call_response(vec![ProviderToolCall {
                    id: format!("call_{round}"),
                    name: "exec".into(),
                    arguments: json!({"input":format!("read {round}")}),
                    provider_metadata: json!({"type":"custom_tool_call"}),
                }])
            } else {
                final_llm_response(text)
            };
            result.response_id = Some(format!("resp_provider_{round}"));
            let events = items
                .into_iter()
                .enumerate()
                .flat_map(|(index, item)| {
                    [
                        ProviderOutputItemPhase::Added,
                        ProviderOutputItemPhase::Done,
                    ]
                    .map(|phase| ProviderStreamEvent::OutputItem {
                        phase,
                        output_index: index,
                        item: item.clone(),
                    })
                })
                .collect();
            ProviderInvocationOutput {
                events,
                result,
                first_token_at: None,
                time_to_first_token_ms: None,
            }
        })
        .collect();
    let (invoker, captured) = sequential_tool_output_invoker(outputs);
    let input = json!({"node-start":{"query":"read the dependency chain"}});
    for round in 0..4 {
        let context = ExecutionRuntimeContext::from_plan_input(&plan, input.as_object().unwrap())
            .unwrap()
            .with_provider_invocation_capability(
                ProviderInvocationCapability::ResponsesNativePassthrough,
            );
        let outcome = start_flow_debug_run_with_runtime_context(&plan, &input, context, &invoker)
            .await
            .unwrap();
        assert!(
            matches!(outcome.stop_reason, ExecutionStopReason::Completed),
            "client sampling {round}: {:?}",
            outcome.stop_reason
        );
        assert_eq!(
            outcome
                .node_traces
                .iter()
                .filter(|trace| trace.node_id == "node-llm")
                .count(),
            1
        );
        assert!(outcome.variable_pool.contains_key("node-answer"));
        if round < 3 {
            let llm = outcome
                .node_traces
                .iter()
                .find(|trace| trace.node_id == "node-llm")
                .unwrap();
            assert_eq!(llm.metrics_payload["usage"]["input_tokens"], 11);
            assert_eq!(llm.metrics_payload["usage"]["output_tokens"], 3);
            assert!(
                llm.provider_events.iter().any(|event| matches!(event,
                ProviderStreamEvent::OutputItem {phase:ProviderOutputItemPhase::Done,item,..}
                if item["type"] == "custom_tool_call" && item["input"] == format!("read {round}")))
            );
            assert_eq!(
                outcome.variable_pool["node-llm"]["tool_calls"][0]["id"],
                format!("call_{round}")
            );
        } else {
            assert_eq!(outcome.variable_pool["node-answer"]["answer"], "finished");
        }
    }
    assert_eq!(captured.lock().unwrap().len(), 4);
}
