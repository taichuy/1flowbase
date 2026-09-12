use super::*;
use crate::execution_engine::resume_flow_debug_run_with_runtime_context;
use extension_contracts::provider_contract::ProviderOutputItemPhase;

// #2036 AC-001/002/003: drive the real scheduler through three client tool waits.
#[tokio::test]
async fn native_three_tool_rounds_keep_checkpoint_and_execute_downstream_once() {
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
    let context = ExecutionRuntimeContext::from_plan_input(&plan, input.as_object().unwrap())
        .unwrap()
        .with_provider_invocation_capability(
            ProviderInvocationCapability::ResponsesNativePassthrough,
        );
    let mut outcome = start_flow_debug_run_with_runtime_context(&plan, &input, context, &invoker)
        .await
        .unwrap();
    let mut traces = Vec::new();
    for round in 0..3 {
        let ExecutionStopReason::WaitingCallback(ref wait) = outcome.stop_reason else {
            panic!("must wait for tool {round}")
        };
        assert_eq!(wait.node_id, "node-llm");
        assert_eq!(
            wait.request_payload["tool_calls"][0]["id"],
            format!("call_{round}")
        );
        assert!(!outcome.variable_pool.contains_key("node-answer"));
        traces.extend(
            outcome
                .node_traces
                .iter()
                .map(|trace| trace.node_id.clone()),
        );
        let checkpoint = outcome.checkpoint_snapshot.as_ref().unwrap();
        let context = ExecutionRuntimeContext::from_plan_input(&plan, &checkpoint.variable_pool)
            .unwrap()
            .with_provider_invocation_capability(
                ProviderInvocationCapability::ResponsesNativePassthrough,
            );
        outcome = resume_flow_debug_run_with_runtime_context(&plan,checkpoint,"node-llm",
            &json!({"tool_results":[{"tool_call_id":format!("call_{round}"),"content":format!("result {round}")}]}),context,&invoker).await.unwrap();
    }
    assert!(matches!(
        outcome.stop_reason,
        ExecutionStopReason::Completed
    ));
    traces.extend(
        outcome
            .node_traces
            .iter()
            .map(|trace| trace.node_id.clone()),
    );
    assert_eq!(
        traces
            .iter()
            .filter(|id| id.as_str() == "node-start")
            .count(),
        1
    );
    assert_eq!(
        traces.iter().filter(|id| id.as_str() == "node-llm").count(),
        4
    );
    assert_eq!(
        traces
            .iter()
            .filter(|id| id.as_str() == "node-answer")
            .count(),
        1
    );
    assert_eq!(captured.lock().unwrap().len(), 4);
    assert_eq!(outcome.variable_pool["node-answer"]["answer"], "finished");
}
