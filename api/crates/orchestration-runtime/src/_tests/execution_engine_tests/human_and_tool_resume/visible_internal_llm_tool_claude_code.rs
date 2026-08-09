use super::visible_internal_llm_tool_fixtures::*;
use super::*;

#[tokio::test]
async fn visible_internal_llm_tool_recall_keeps_native_prompt_across_if_else() {
    const ORIGINAL_REQUIREMENT: &str =
        "Design pages, tabs, and blocks from the original UI design request.";
    const CURRENT_IMAGE_NOTE: &str = "[Image: original 2643x1119]";
    let (invoker, captured_inputs) = sequential_tool_invoker(vec![
        ProviderInvocationResult {
            final_content: Some("I will inspect the screenshot.".to_string()),
            tool_calls: vec![ProviderToolCall {
                id: "call_visible".to_string(),
                name: "inspect_visible_context".to_string(),
                arguments: json!({ "query": "inspect the UI screenshot" }),
                provider_metadata: json!({}),
            }],
            finish_reason: Some(ProviderFinishReason::ToolCall),
            ..ProviderInvocationResult::default()
        },
        final_llm_response("mounted result"),
        final_llm_response("final answer"),
    ]);

    let outcome = start_flow_debug_run(
        &visible_internal_llm_tool_plan_behind_if_else(),
        &json!({
            "__native_model_prompt_context": {
                "system": [
                    { "type": "text", "text": "Follow the original user requirement." }
                ],
                "messages": [
                    { "role": "user", "content": ORIGINAL_REQUIREMENT },
                    { "role": "assistant", "content": "I will inspect the repository." }
                ]
            },
            "node-start": {
                "query": CURRENT_IMAGE_NOTE,
                "history": []
            }
        }),
        &invoker,
    )
    .await
    .unwrap();

    assert!(matches!(
        outcome.stop_reason,
        ExecutionStopReason::Completed
    ));
    let captured = captured_inputs
        .lock()
        .expect("captured inputs mutex poisoned")
        .clone();
    assert_eq!(captured.len(), 3);

    for input in [&captured[0], &captured[2]] {
        assert_eq!(
            input
                .messages
                .iter()
                .filter(|message| message.content == ORIGINAL_REQUIREMENT)
                .count(),
            1,
            "every main LLM round must keep the original AI Native user turn exactly once: {:?}",
            input.messages
        );
        assert_eq!(
            input
                .messages
                .iter()
                .filter(|message| message.content == CURRENT_IMAGE_NOTE)
                .count(),
            1,
            "every main LLM round must keep the current image note exactly once: {:?}",
            input.messages
        );
        assert_eq!(
            input.system_text().as_deref(),
            Some("Follow the original user requirement.")
        );
    }
}

#[tokio::test]
async fn visible_internal_llm_tool_is_hidden_for_claude_code_control_runs() {
    let (invoker, captured_inputs) = sequential_tool_invoker(vec![final_llm_response("summary")]);
    let plan = visible_internal_llm_tool_plan();

    let outcome = start_flow_debug_run(
        &plan,
        &json!({
            "node-start": {
                "query": "Your task is to create a detailed summary of the conversation so far",
                "compatibility": {
                    "claude_code_control": "compact_summary"
                },
                "tools": [
                    {
                        "name": "Bash",
                        "description": "Run a shell command",
                        "input_schema": {
                            "type": "object",
                            "properties": {
                                "command": { "type": "string" }
                            },
                            "required": ["command"]
                        }
                    }
                ]
            }
        }),
        &invoker,
    )
    .await
    .unwrap();

    assert!(
        matches!(outcome.stop_reason, ExecutionStopReason::Completed),
        "expected completed control run, got {:?}",
        outcome.stop_reason
    );
    let captured = captured_inputs
        .lock()
        .expect("captured inputs mutex poisoned")
        .clone();
    assert_eq!(captured.len(), 1);
    assert!(
        captured[0].tools.is_empty(),
        "control runs must not expose client or visible internal LLM tools"
    );
}

#[tokio::test]
async fn visible_internal_llm_tool_stays_available_for_claude_code_compact_resume() {
    let (invoker, captured_inputs) = sequential_tool_invoker(vec![final_llm_response("resume")]);
    let plan = visible_internal_llm_tool_plan();

    let outcome = start_flow_debug_run(
        &plan,
        &json!({
            "node-start": {
                "query": "This session is being continued from a previous conversation that ran out of context.\n\nIf you need specific details from before compaction, use the summary.",
                "compatibility": {
                    "claude_code_control": "compact_resume"
                },
                "tools": [
                    {
                        "name": "Bash",
                        "description": "Run a shell command",
                        "input_schema": {
                            "type": "object",
                            "properties": {
                                "command": { "type": "string" }
                            },
                            "required": ["command"]
                        }
                    }
                ]
            }
        }),
        &invoker,
    )
    .await
    .unwrap();

    assert!(
        matches!(outcome.stop_reason, ExecutionStopReason::Completed),
        "expected completed compact resume run, got {:?}",
        outcome.stop_reason
    );
    let captured = captured_inputs
        .lock()
        .expect("captured inputs mutex poisoned")
        .clone();
    let tool_names = captured[0]
        .tools
        .iter()
        .filter_map(|tool| tool["function"]["name"].as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        tool_names,
        vec!["Bash", "Bash_run_0", "inspect_visible_context"]
    );
}
