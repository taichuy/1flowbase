use super::*;
use crate::ports::{ProviderRuntimeInvocationOutput, ProviderTransportPayload};
use plugin_framework::provider_contract::{
    ProviderFinishReason, ProviderInvocationResult, ProviderOutputItemPhase, ProviderStreamEvent,
    ProviderToolCall,
};

#[tokio::test]
async fn native_empty_prewarm_cross_flow_delta_creates_verified_callback_history() {
    let tool_item = json!({"type":"function_call", "id":"fc_native", "call_id":"call_native",
        "name":"lookup_weather", "arguments":"{\"city\":\"Shanghai\"}"});
    let service = OrchestrationRuntimeService::for_tests_with_provider_outputs(vec![
        ProviderRuntimeInvocationOutput {
            events: vec![ProviderStreamEvent::Finish {
                reason: ProviderFinishReason::Stop,
            }],
            result: ProviderInvocationResult {
                response_id: Some("upstream-pwarm".into()),
                finish_reason: Some(ProviderFinishReason::Stop),
                ..Default::default()
            },
        },
        ProviderRuntimeInvocationOutput {
            events: vec![
                ProviderStreamEvent::OutputItem {
                    phase: ProviderOutputItemPhase::Done,
                    output_index: 0,
                    item: tool_item.clone(),
                },
                ProviderStreamEvent::Finish {
                    reason: ProviderFinishReason::ToolCall,
                },
            ],
            result: ProviderInvocationResult {
                response_id: Some("upstream-tool-round".into()),
                tool_calls: vec![ProviderToolCall {
                    id: "call_native".into(),
                    name: "lookup_weather".into(),
                    arguments: json!({"city":"Shanghai"}),
                    provider_metadata: json!({}),
                }],
                finish_reason: Some(ProviderFinishReason::ToolCall),
                ..Default::default()
            },
        },
    ]);
    service.enable_native_responses_fixture().await;
    let seeded = service
        .seed_application_with_flow("Native warmup chain")
        .await;
    let tools = json!([{"type":"function", "name":"lookup_weather", "description":"Lookup weather",
        "parameters":{"type":"object","properties":{"city":{"type":"string"}},"required":["city"]}}]);
    let initial_input = json!([{"role":"user","content":"weather"}]);
    let start = || StartFlowDebugRunCommand {
        actor_user_id: seeded.actor_user_id,
        application_id: seeded.application_id,
        input_payload: json!({"node-start":{"query":"weather"}}),
        document_snapshot: None,
        debug_session_id: None,
    };
    let prewarm = service.start_flow_debug_run(start()).await.unwrap();
    let prewarm = service
        .continue_native_responses_fixture(
            ContinueFlowDebugRunCommand {
                application_id: seeded.application_id,
                flow_run_id: prewarm.flow_run.id,
                workspace_id: Uuid::nil(),
            },
            ProviderTransportPayload::openai_responses(json!({"model":"gpt-5.4-mini",
        "generate":false,"tools":tools,"input":initial_input}))
            .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        prewarm.flow_run.status,
        domain::FlowRunStatus::Succeeded,
        "empty native warmup must remain successful: {:?}",
        prewarm.flow_run.error_payload
    );
    let continuation = service
        .native_continuation_fixture(prewarm.flow_run.id)
        .await;
    assert!(
        continuation.native_history().is_some(),
        "proof must come from empty successful provider events"
    );
    let turn = service.start_flow_debug_run(start()).await.unwrap();
    let payload = ProviderTransportPayload::openai_responses(json!({"model":"gpt-5.4-mini",
        "previous_response_id":format!("resp_{}",prewarm.flow_run.id),"tools":tools,"input":[]}))
    .unwrap()
    .bind_openai_continuation(continuation)
    .unwrap();
    let waiting = service
        .continue_native_responses_fixture(
            ContinueFlowDebugRunCommand {
                application_id: seeded.application_id,
                flow_run_id: turn.flow_run.id,
                workspace_id: Uuid::nil(),
            },
            payload,
        )
        .await
        .unwrap();
    assert_eq!(
        waiting.flow_run.status,
        domain::FlowRunStatus::WaitingCallback,
        "native delta must reach callback: {:?}",
        waiting.flow_run.error_payload
    );
    assert_eq!(waiting.callback_tasks.len(), 1);
    let checkpoint = service
        .checkpoint_snapshot_for_tests(waiting.checkpoints.last().unwrap())
        .await;
    let metadata = &checkpoint.variable_pool["node-llm"]["__llm_tool_callback"]
        ["provider_metadata"]["native_response"];
    let mut full_input = initial_input.as_array().unwrap().clone();
    full_input.push(tool_item);
    full_input
        .push(json!({"type":"function_call_output","call_id":"call_native","output":"sunny"}));
    crate::application_public_api::compat::openai::history::validate_full_retry_input(
        &Value::Array(full_input.clone()),
        &metadata["history"],
        &["call_native".into()],
    )
    .expect("cross-flow history must include the prewarm input and canonical completed tool item");
    full_input[0]["content"] = json!("different request");
    assert!(
        crate::application_public_api::compat::openai::history::validate_full_retry_input(
            &Value::Array(full_input),
            &metadata["history"],
            &["call_native".into()],
        )
        .is_err(),
        "the proof must reject a changed prewarm input"
    );
    assert_eq!(metadata["binding"]["node_id"], "node-llm");
    assert_eq!(
        service
            .list_model_failover_attempt_ledger(prewarm.flow_run.id)
            .await
            .len(),
        1
    );
    assert_eq!(
        service
            .list_model_failover_attempt_ledger(waiting.flow_run.id)
            .await
            .len(),
        1
    );
}
