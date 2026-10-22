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
                    phase: ProviderOutputItemPhase::Added,
                    output_index: 0,
                    item: tool_item.clone(),
                },
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
    // Validate the stored proof through its version-aware contract, not a stale version literal.
    let mut full_input = initial_input.as_array().unwrap().clone();
    full_input.push(tool_item);
    full_input
        .push(json!({"type":"function_call_output","call_id":"call_native","output":"sunny"}));
    crate::_tests::validate_full_retry_input(
        &Value::Array(full_input.clone()),
        &metadata["history"],
        &["call_native".into()],
    )
    .expect("cross-flow history must include the prewarm input and canonical completed tool item");
    full_input[0]["content"] = json!("different request");
    assert!(
        crate::_tests::validate_full_retry_input(
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

#[tokio::test]
async fn failed_and_length_limited_native_invocations_never_publish_history_proof() {
    for (finish_reason, expected_status) in [
        (ProviderFinishReason::Error, domain::FlowRunStatus::Failed),
        (
            ProviderFinishReason::Length,
            domain::FlowRunStatus::Incomplete,
        ),
    ] {
        // The item lifecycle itself is valid and complete; it is the response that failed
        // or hit its output limit. Completed partial items do not prove a complete response.
        let item = json!({"type":"message","id":"msg_partial","role":"assistant",
            "content":[{"type":"output_text","text":"partial answer"}]});
        let service = OrchestrationRuntimeService::for_tests_with_provider_outputs(vec![
            ProviderRuntimeInvocationOutput {
                events: vec![
                    ProviderStreamEvent::OutputItem {
                        phase: ProviderOutputItemPhase::Added,
                        output_index: 0,
                        item: item.clone(),
                    },
                    ProviderStreamEvent::OutputItem {
                        phase: ProviderOutputItemPhase::Done,
                        output_index: 0,
                        item,
                    },
                    ProviderStreamEvent::Finish {
                        reason: finish_reason.clone(),
                    },
                ],
                result: ProviderInvocationResult {
                    response_id: Some("upstream-partial".into()),
                    final_content: Some("partial answer".into()),
                    finish_reason: Some(finish_reason.clone()),
                    ..Default::default()
                },
            },
        ]);
        service.enable_native_responses_fixture().await;
        let seeded = service
            .seed_application_with_flow("Native unsuccessful history")
            .await;
        let started = service
            .start_flow_debug_run(StartFlowDebugRunCommand {
                actor_user_id: seeded.actor_user_id,
                application_id: seeded.application_id,
                input_payload: json!({"node-start":{"query":"original request"}}),
                document_snapshot: None,
                debug_session_id: None,
            })
            .await
            .unwrap();
        let flow_run_id = started.flow_run.id;
        let ended = service
            .continue_native_responses_fixture(
                ContinueFlowDebugRunCommand {
                    application_id: seeded.application_id,
                    flow_run_id,
                    workspace_id: Uuid::nil(),
                },
                ProviderTransportPayload::openai_responses(json!({"model":"gpt-5.4-mini",
            "input":[{"role":"user","content":"original request"}]}))
                .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            ended.flow_run.status, expected_status,
            "finish reason {finish_reason:?}"
        );
        let continuation = service
            .optional_native_continuation_fixture(flow_run_id)
            .await;
        assert!(
            continuation
                .as_ref()
                .and_then(|value| value.native_history())
                .is_none(),
            "a failed or length-limited real invoker response must not publish complete history"
        );
        assert_eq!(
            service
                .list_model_failover_attempt_ledger(flow_run_id)
                .await
                .len(),
            1,
            "no fixture fallback or hidden retry may manufacture a successful response"
        );
    }
}

#[tokio::test]
async fn unfinished_native_output_item_never_creates_complete_history_proof() {
    // A response ID and Stop alone cannot turn an unfinished item into empty prewarm.
    let service = OrchestrationRuntimeService::for_tests_with_provider_outputs(vec![
        ProviderRuntimeInvocationOutput {
            events: vec![
                ProviderStreamEvent::OutputItem {
                    phase: ProviderOutputItemPhase::Added,
                    output_index: 0,
                    item: json!({"type":"message","id":"msg_open","role":"assistant","content":[]}),
                },
                ProviderStreamEvent::Finish {
                    reason: ProviderFinishReason::Stop,
                },
            ],
            result: ProviderInvocationResult {
                response_id: Some("upstream-open-item".into()),
                finish_reason: Some(ProviderFinishReason::Stop),
                ..Default::default()
            },
        },
    ]);
    service.enable_native_responses_fixture().await;
    let seeded = service
        .seed_application_with_flow("Native unfinished output")
        .await;
    let started = service
        .start_flow_debug_run(StartFlowDebugRunCommand {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            input_payload: json!({"node-start":{"query":"original request"}}),
            document_snapshot: None,
            debug_session_id: None,
        })
        .await
        .unwrap();
    let flow_run_id = started.flow_run.id;
    service
        .continue_native_responses_fixture(
            ContinueFlowDebugRunCommand {
                application_id: seeded.application_id,
                flow_run_id,
                workspace_id: Uuid::nil(),
            },
            ProviderTransportPayload::openai_responses(json!({"model":"gpt-5.4-mini",
        "input":[{"role":"user","content":"original request"}]}))
            .unwrap(),
        )
        .await
        .expect("the real service must persist the terminal outcome");
    let continuation = service
        .optional_native_continuation_fixture(flow_run_id)
        .await;
    assert!(
        continuation
            .as_ref()
            .and_then(|value| value.native_history())
            .is_none(),
        "Added without Done must never masquerade as successful empty output"
    );
    assert_eq!(
        service
            .list_model_failover_attempt_ledger(flow_run_id)
            .await
            .len(),
        1
    );
}

fn text_output(formal_native: bool) -> ProviderRuntimeInvocationOutput {
    let item = json!({"type":"message","id":"msg_selected","role":"assistant","status":"completed","content":[{"type":"output_text","text":"selected transport reply","annotations":[]}]});
    let mut events = Vec::new();
    if formal_native {
        events.extend([
            ProviderStreamEvent::OutputItem {
                phase: ProviderOutputItemPhase::Added,
                output_index: 0,
                item: item.clone(),
            },
            ProviderStreamEvent::OutputItem {
                phase: ProviderOutputItemPhase::Done,
                output_index: 0,
                item,
            },
        ]);
    }
    events.push(ProviderStreamEvent::Finish {
        reason: ProviderFinishReason::Stop,
    });
    ProviderRuntimeInvocationOutput {
        events,
        result: ProviderInvocationResult {
            final_content: Some("selected transport reply".into()),
            response_id: Some("upstream-selected".into()),
            finish_reason: Some(ProviderFinishReason::Stop),
            ..Default::default()
        },
    }
}

fn preferred_notice() -> ProviderTransportPayload {
    ProviderTransportPayload::openai_responses(
        json!({"model":"gpt-5.4-mini","input":[{"type":"function_call_output","output":"notice"}]}),
    )
    .unwrap()
    .preferred()
}

#[tokio::test]
async fn preferred_selected_mode_controls_real_executor_output_recovery_and_metadata() {
    use plugin_framework::provider_contract::{
        ProviderRecoveryDirective, RecoveryPolicy, PROVIDER_RECOVERY_DIRECTIVE_CONTEXT_KEY,
    };
    for (native_target, formal_output, expected_success) in [
        (false, false, true),
        (true, false, false),
        (true, true, true),
    ] {
        let (service, captured) =
            OrchestrationRuntimeService::for_tests_with_captured_provider_outputs(vec![
                text_output(formal_output),
            ]);
        if native_target {
            service.enable_native_responses_fixture().await;
        }
        let seeded = service
            .seed_application_with_flow("Preferred selected transport")
            .await;
        let started = service
            .start_flow_debug_run(StartFlowDebugRunCommand {
                actor_user_id: seeded.actor_user_id,
                application_id: seeded.application_id,
                input_payload: json!({"node-start":{"query":"canonical notice"}}),
                document_snapshot: None,
                debug_session_id: None,
            })
            .await
            .unwrap();
        let detail = service
            .continue_native_responses_fixture(
                ContinueFlowDebugRunCommand {
                    application_id: seeded.application_id,
                    flow_run_id: started.flow_run.id,
                    workspace_id: Uuid::nil(),
                },
                preferred_notice(),
            )
            .await
            .unwrap();
        assert_eq!(
            detail.flow_run.status,
            if expected_success {
                domain::FlowRunStatus::Succeeded
            } else {
                domain::FlowRunStatus::Failed
            },
            "{:?}",
            detail.flow_run.error_payload
        );
        {
            let inputs = captured.lock().unwrap();
            assert_eq!(inputs.len(), 1);
            assert_eq!(inputs[0].native_transport.is_some(), native_target);
            let directive: ProviderRecoveryDirective = serde_json::from_value(
                inputs[0].run_context[PROVIDER_RECOVERY_DIRECTIVE_CONTEXT_KEY].clone(),
            )
            .unwrap();
            assert_eq!(
                matches!(directive.policy, RecoveryPolicy::NativeOpaque { .. }),
                native_target
            );
        }
        if !native_target {
            assert!(service
                .optional_native_continuation_fixture(started.flow_run.id)
                .await
                .is_none());
            assert!(!detail
                .flow_run
                .output_payload
                .to_string()
                .contains("native_response"));
        }
        if native_target && !formal_output {
            assert!(detail
                .flow_run
                .error_payload
                .as_ref()
                .expect("native failure payload")
                .to_string()
                .contains("missing or duplicate formal output item"));
        }
    }
}

#[tokio::test]
async fn required_native_affinity_never_falls_back_in_the_real_executor() {
    let (service, captured) = OrchestrationRuntimeService::for_tests_with_captured_provider_outputs(
        vec![text_output(false)],
    );
    service.enable_native_responses_fixture().await;
    let seeded = service
        .seed_application_with_flow("Required affinity")
        .await;
    let started = service
        .start_flow_debug_run(StartFlowDebugRunCommand {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            input_payload: json!({"node-start":{"query":"canonical notice"}}),
            document_snapshot: None,
            debug_session_id: None,
        })
        .await
        .unwrap();
    let required = preferred_notice().with_affinity(crate::ports::ProviderTransportAffinity::new(
        Uuid::now_v7().to_string(),
        "fixture_provider",
        "openai_compatible",
        "gpt-5.4-mini",
    ));
    let detail = service
        .continue_native_responses_fixture(
            ContinueFlowDebugRunCommand {
                application_id: seeded.application_id,
                flow_run_id: started.flow_run.id,
                workspace_id: Uuid::nil(),
            },
            required,
        )
        .await
        .unwrap();
    assert_eq!(detail.flow_run.status, domain::FlowRunStatus::Failed);
    assert!(captured.lock().unwrap().is_empty());
}

#[tokio::test]
async fn semantic_callback_resume_with_preferred_sidecar_keeps_selected_semantic_recovery() {
    use plugin_framework::provider_contract::{
        ProviderRecoveryDirective, RecoveryPolicy, PROVIDER_RECOVERY_DIRECTIVE_CONTEXT_KEY,
    };
    let calls = vec![ProviderToolCall {
        id: "call-selected".into(),
        name: "lookup_weather".into(),
        arguments: json!({}),
        provider_metadata: json!({}),
    }];
    let (service, captured) =
        OrchestrationRuntimeService::for_tests_with_captured_provider_outputs(vec![
            ProviderRuntimeInvocationOutput {
                events: vec![ProviderStreamEvent::Finish {
                    reason: ProviderFinishReason::ToolCall,
                }],
                result: ProviderInvocationResult {
                    tool_calls: calls,
                    finish_reason: Some(ProviderFinishReason::ToolCall),
                    ..Default::default()
                },
            },
            text_output(false),
        ]);
    let seeded = service.seed_application_with_flow("Preferred resume").await;
    let started = service
        .start_flow_debug_run(StartFlowDebugRunCommand {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            input_payload: json!({"node-start":{"query":"weather"}}),
            document_snapshot: None,
            debug_session_id: None,
        })
        .await
        .unwrap();
    let waiting = service
        .continue_native_responses_fixture(
            ContinueFlowDebugRunCommand {
                application_id: seeded.application_id,
                flow_run_id: started.flow_run.id,
                workspace_id: Uuid::nil(),
            },
            preferred_notice(),
        )
        .await
        .unwrap();
    assert_eq!(
        waiting.flow_run.status,
        domain::FlowRunStatus::WaitingCallback
    );
    let completed = service.complete_callback_task(CompleteCallbackTaskCommand {
        responses_continuation:None, transport_connection_scope:None, observation_context:None,
        native_transport:Some(preferred_notice()), actor_user_id:seeded.actor_user_id,
        application_id:seeded.application_id, callback_task_id:waiting.callback_tasks[0].id,
        response_payload:json!({"tool_results":[{"tool_call_id":"call-selected","content":"sunny"}]}),
    }).await.unwrap();
    assert_eq!(
        completed.flow_run.status,
        domain::FlowRunStatus::Succeeded,
        "{:?}",
        completed.flow_run.error_payload
    );
    let inputs = captured.lock().unwrap();
    assert_eq!(inputs.len(), 2);
    for input in inputs.iter() {
        assert!(input.native_transport.is_none());
        let directive: ProviderRecoveryDirective = serde_json::from_value(
            input.run_context[PROVIDER_RECOVERY_DIRECTIVE_CONTEXT_KEY].clone(),
        )
        .unwrap();
        assert!(matches!(
            directive.policy,
            RecoveryPolicy::SemanticMapped { .. }
        ));
    }
}

#[tokio::test]
async fn selected_input_fact_commits_before_provider_output_and_failure_prevents_invocation() {
    for fail_commit in [false, true] {
        let stream = std::sync::Arc::new(crate::_tests::RecordingRuntimeEventStream::default());
        let mut output = text_output(false);
        output.events.insert(
            0,
            ProviderStreamEvent::TextDelta {
                delta: "selected transport reply".into(),
            },
        );
        let (service, captured) =
            OrchestrationRuntimeService::for_tests_with_captured_provider_outputs(vec![output]);
        let service = std::sync::Arc::new(service.with_runtime_event_stream(stream.clone()));
        let seeded = service
            .seed_application_with_flow("Selected input commit gate")
            .await;
        let started = service
            .start_flow_debug_run(StartFlowDebugRunCommand {
                actor_user_id: seeded.actor_user_id,
                application_id: seeded.application_id,
                input_payload: json!({"node-start":{"query":"canonical notice"}}),
                document_snapshot: None,
                debug_session_id: None,
            })
            .await
            .unwrap();
        let run_id = started.flow_run.id;
        let (entered, release) = service.gate_llm_selection_fixture(fail_commit);
        let owner = tokio::spawn({
            let service = service.clone();
            async move {
                service
                    .continue_native_responses_fixture(
                        ContinueFlowDebugRunCommand {
                            application_id: seeded.application_id,
                            flow_run_id: run_id,
                            workspace_id: Uuid::nil(),
                        },
                        preferred_notice(),
                    )
                    .await
            }
        });
        tokio::time::timeout(std::time::Duration::from_secs(3), entered.notified())
            .await
            .unwrap();
        assert!(
            captured.lock().unwrap().is_empty(),
            "provider ran before selection commit"
        );
        assert!(!owner.is_finished());
        assert!(service
            .llm_selection_events_fixture(run_id)
            .await
            .iter()
            .all(|event| event.event_type != crate::ports::LlmInputSelection::EVENT_TYPE));
        release.notify_one();
        let detail = owner.await.unwrap().unwrap();
        let rows = service.llm_selection_events_fixture(run_id).await;
        let selected = rows
            .iter()
            .filter(|event| event.event_type == crate::ports::LlmInputSelection::EVENT_TYPE)
            .collect::<Vec<_>>();
        if fail_commit {
            assert!(
                captured.lock().unwrap().is_empty(),
                "provider ran after selection commit failed"
            );
            assert!(selected.is_empty());
            assert_eq!(detail.flow_run.status, domain::FlowRunStatus::Failed);
        } else {
            assert_eq!(captured.lock().unwrap().len(), 1);
            assert_eq!(selected.len(), 1);
            assert_eq!(selected[0].source, domain::RuntimeEventSource::Host);
            assert_eq!(selected[0].trust_level, domain::RuntimeTrustLevel::HostFact);
            assert_eq!(selected[0].payload["representation"], "semantic_mapped");
            let live = stream.events();
            let selection_index = live
                .iter()
                .position(|event| event.event_type == crate::ports::LlmInputSelection::EVENT_TYPE)
                .unwrap();
            let output_index = live
                .iter()
                .position(|event| event.event_type == "text_delta")
                .unwrap();
            assert!(selection_index < output_index);
            assert_eq!(detail.flow_run.status, domain::FlowRunStatus::Succeeded);
        }
    }
}
