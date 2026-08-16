use serde_json::json;

use super::*;

#[test]
fn tools_have_a_native_translation_receipt() {
    let translated = translate_messages_request(json!({
        "model": "claude-compatible",
        "messages": [
            { "role": "user", "content": "say hello" }
        ],
        "tools": [
            {
                "name": "read_file",
                "description": "Read a file",
                "input_schema": {
                    "type": "object",
                    "properties": {
                        "file_path": { "type": "string" }
                    }
                }
            }
        ],
        "tool_choice": { "type": "auto" }
    }))
    .expect("tools have a Native owner");

    assert!(translated
        .report
        .has_decision("$.tools", TranslationDecisionKind::Normalized));
    assert_eq!(
        translated.request.inputs.as_value()["tools"][0]["name"],
        "read_file"
    );
}

#[test]
fn issue_1736_ac_001_eager_tool_input_streaming_degrades_without_rejecting_the_tool() {
    let translated = translate_messages_request(json!({
        "model": "claude-compatible",
        "messages": [{ "role": "user", "content": "inspect the repository" }],
        "tools": [{
            "name": "Bash",
            "description": "Run a command",
            "input_schema": {
                "type": "object",
                "properties": { "command": { "type": "string" } },
                "required": ["command"]
            },
            "eager_input_streaming": true
        }]
    }))
    .expect("eager input streaming is a transport hint, not a reason to reject the tool");

    assert_eq!(
        translated.request.inputs.as_value()["tools"][0]["name"],
        "Bash"
    );
    assert!(translated.report.has_decision(
        "$.tools[0].eager_input_streaming",
        TranslationDecisionKind::Dropped
    ));
}

#[test]
fn issue_1736_ac_003_semantic_tool_fields_fail_with_a_typed_unsupported_error() {
    for field in ["strict", "defer_loading"] {
        let mut tool = json!({
            "name": "Bash",
            "input_schema": { "type": "object" }
        });
        tool[field] = json!(true);
        let error = translate_messages_request(json!({
            "model": "claude-compatible",
            "messages": [{ "role": "user", "content": "inspect the repository" }],
            "tools": [tool]
        }))
        .expect_err("semantic tool fields must not be silently discarded");

        assert_eq!(error.error_type, "unsupported_feature");
        assert!(error.report.has_decision(
            &format!("$.tools[0].{field}"),
            TranslationDecisionKind::Unsupported
        ));
    }
}

#[test]
fn issue_1736_ac_003_unknown_tool_fields_remain_rejected() {
    let error = translate_messages_request(json!({
        "model": "claude-compatible",
        "messages": [{ "role": "user", "content": "inspect the repository" }],
        "tools": [{
            "name": "Bash",
            "input_schema": { "type": "object" },
            "future_unknown_field": true
        }]
    }))
    .expect_err("unclassified tool fields must continue to fail closed");

    assert_eq!(error.error_type, "invalid_request");
    assert_eq!(error.message, "unknown Anthropic tool field");
}

#[test]
fn prompt_markers_are_not_interpreted_as_system_context() {
    let request = map_messages_request(json!({
        "model": "1flowbase",
        "messages": [
            {
                "role": "user",
                "content": "<system-reminder>internal tools</system-reminder>\n\nhi？"
            }
        ]
    }))
    .unwrap();

    assert_eq!(
        request.query,
        "<system-reminder>internal tools</system-reminder>\n\nhi？"
    );
    assert!(request.system_text().is_none());
    assert!(request.history.is_empty());
}

#[test]
fn maps_top_level_system_content_blocks_into_native_system() {
    let request = map_messages_request(json!({
        "model": "1flowbase",
        "system": [
            {
                "type": "text",
                "text": "Use Claude Code project instructions.",
                "cache_control": { "type": "ephemeral" }
            },
            {
                "type": "text",
                "text": "Preserve repository safety rules."
            }
        ],
        "messages": [
            { "role": "user", "content": "hi？" }
        ]
    }))
    .unwrap();

    assert_eq!(
        request.system_text().as_deref(),
        Some("Use Claude Code project instructions.\n\nPreserve repository safety rules.")
    );
    assert_eq!(request.query, "hi？");
}

#[test]
fn prompt_markers_are_not_interpreted_in_history() {
    let request = map_messages_request(json!({
        "model": "1flowbase",
        "messages": [
            {
                "role": "user",
                "content": "<system-reminder>available tools</system-reminder>\n\nhi？"
            },
            {
                "role": "assistant",
                "content": "<think>private reasoning</think>嗨，有什么需要我帮忙的？"
            },
            {
                "role": "user",
                "content": "uploads/agent-flow-preview-debug.png 描述一下这幅图说什么？"
            }
        ]
    }))
    .unwrap();

    assert_eq!(
        request.query,
        "uploads/agent-flow-preview-debug.png 描述一下这幅图说什么？"
    );
    assert_eq!(
        request.history,
        vec![
            json!({"role": "user", "content": "<system-reminder>available tools</system-reminder>\n\nhi？"}),
            json!({"role": "assistant", "content": "<think>private reasoning</think>嗨，有什么需要我帮忙的？"}),
        ]
    );
    assert!(request.system_text().is_none());
}

#[test]
fn duplicate_turns_are_preserved_without_prompt_marker_heuristics() {
    let request = map_messages_request(json!({
        "model": "1flowbase",
        "messages": [
            {"role": "user", "content": "Describe image"},
            {"role": "assistant", "content": "old draft"},
            {"role": "user", "content": "Describe image"},
            {"role": "assistant", "content": "<think>retry</think>final draft"},
            {"role": "user", "content": "Continue"}
        ]
    }))
    .unwrap();

    assert_eq!(request.query, "Continue");
    assert_eq!(
        request.history,
        vec![
            json!({"role": "user", "content": "Describe image"}),
            json!({"role": "assistant", "content": "old draft"}),
            json!({"role": "user", "content": "Describe image"}),
            json!({"role": "assistant", "content": "<think>retry</think>final draft"}),
        ]
    );
}

#[test]
fn replayed_current_user_turn_keeps_prior_history() {
    let request = map_messages_request(json!({
        "model": "1flowbase",
        "messages": [
            {"role": "user", "content": "Describe image"},
            {"role": "assistant", "content": "old image answer"},
            {"role": "user", "content": "Describe image"}
        ]
    }))
    .unwrap();

    assert_eq!(request.query, "Describe image");
    assert_eq!(
        request.history,
        vec![
            json!({"role": "user", "content": "Describe image"}),
            json!({"role": "assistant", "content": "old image answer"}),
        ]
    );
}

#[test]
fn latest_media_is_preserved_without_replay_heuristics() {
    let request = map_messages_request(json!({
        "model": "1flowbase",
        "messages": [
            {"role": "user", "content": "Describe image"},
            {"role": "assistant", "content": "old image answer"},
            {
                "role": "user",
                "content": [
                    {"type": "text", "text": "Describe image"},
                    {
                        "type": "image",
                        "source": {
                            "type": "base64",
                            "media_type": "image/png",
                            "data": "aW1hZ2U="
                        }
                    }
                ]
            }
        ]
    }))
    .unwrap();

    assert_eq!(request.query, "Describe image");
    assert_eq!(request.history.len(), 3);
    assert_eq!(request.history[2]["role"], json!("user"));
    assert_eq!(request.history[2]["content"], json!(""));
    assert_eq!(
        request.history[2]["content_blocks"][0]["type"],
        json!("image")
    );
}

#[test]
fn beautified_marker_text_is_not_interpreted() {
    let request = map_messages_request(json!({
            "model": "1flowbase",
            "messages": [
                {"role": "user", "content": "hi？"},
                {
                    "role": "assistant",
                    "content": "<think>draft</think>嗨！\n\n---\n\n下面是美化后内容\n\n你好，有需要我随时帮你。"
                },
                {"role": "user", "content": "继续"}
            ]
        }))
        .unwrap();

    assert_eq!(
        request.history,
        vec![
            json!({"role": "user", "content": "hi？"}),
            json!({"role": "assistant", "content": "<think>draft</think>嗨！\n\n---\n\n下面是美化后内容\n\n你好，有需要我随时帮你。"}),
        ]
    );
}

#[test]
fn thinking_content_has_a_native_reasoning_receipt() {
    let translated = translate_messages_request(json!({
            "model": "1flowbase",
            "messages": [
                {
                    "role": "user",
                    "content": [
                        {
                            "type": "text",
                            "text": "<system-reminder>private Claude Code context</system-reminder>\n\nhi？"
                        }
                    ]
                },
                {
                    "role": "assistant",
                    "content": [
                        {"type": "thinking", "thinking": "private reasoning"},
                        {"type": "text", "text": "<think>draft</think>你好"}
                    ]
                },
                {"role": "user", "content": "继续"}
            ]
        }))
        .expect("thinking content has a Native reasoning owner");

    assert_eq!(
        translated.request.history[1]["content_blocks"][0],
        json!({"type": "reasoning", "text": "private reasoning"})
    );
    assert!(translated.report.has_decision(
        "$.messages[1].content[0].type",
        TranslationDecisionKind::Normalized
    ));
}

#[test]
fn issue_1743_returned_thinking_and_text_close_into_canonical_history() {
    let translated = translate_messages_request(json!({
        "model": "1flowbase",
        "messages": [
            {"role": "user", "content": "first question"},
            {
                "role": "assistant",
                "content": [
                    {
                        "type": "thinking",
                        "thinking": "canonical reasoning",
                        "signature": "signed-reasoning"
                    },
                    {"type": "text", "text": "canonical visible answer"},
                    {"type": "redacted_thinking", "data": "sealed-reasoning"}
                ]
            },
            {"role": "user", "content": "follow up"}
        ]
    }))
    .expect("returned Anthropic content blocks should map to canonical history");

    assert_eq!(translated.request.query, "follow up");
    assert_eq!(
        translated.request.history[1]["content"],
        json!("canonical visible answer")
    );
    assert_eq!(
        translated.request.history[1]["content_blocks"],
        json!([
            {
                "type": "reasoning",
                "text": "canonical reasoning",
                "signature": "signed-reasoning"
            },
            {"type": "text", "text": "canonical visible answer"},
            {"type": "reasoning_redacted", "data": "sealed-reasoning"}
        ])
    );
    assert!(!translated.request.history[1]["content"]
        .as_str()
        .expect("visible history text")
        .contains("reasoning"));
}

#[test]
fn issue_1743_no_reasoning_keeps_tool_and_media_history() {
    let translated = translate_messages_request(json!({
        "model": "1flowbase",
        "messages": [
            {"role": "user", "content": "inspect image"},
            {
                "role": "assistant",
                "content": [
                    {"type": "text", "text": "I will inspect it."},
                    {
                        "type": "tool_use",
                        "id": "toolu_read",
                        "name": "Read",
                        "input": {"file_path": "uploads/test.png"}
                    }
                ]
            },
            {
                "role": "user",
                "content": [{
                    "type": "tool_result",
                    "tool_use_id": "toolu_read",
                    "content": [{
                        "type": "image",
                        "source": {
                            "type": "base64",
                            "media_type": "image/png",
                            "data": "aW1hZ2U="
                        }
                    }]
                }]
            },
            {"role": "user", "content": "continue"}
        ]
    }))
    .expect("text, tool, and media history should remain canonical without reasoning");

    assert_eq!(translated.request.query, "continue");
    assert_eq!(
        translated.request.history[1]["content"],
        json!("I will inspect it.")
    );
    assert_eq!(
        translated.request.history[1]["tool_calls"][0]["id"],
        json!("toolu_read")
    );
    assert_eq!(translated.request.history[2]["role"], json!("tool"));
    assert_eq!(
        translated.request.history[2]["content_blocks"][0]["type"],
        json!("image")
    );
    assert!(translated.request.history[1]
        .get("content_blocks")
        .is_none());
}

#[test]
fn tool_result_history_maps_to_native_tool_messages() {
    let translated = translate_messages_request(json!({
        "model": "1flowbase",
        "messages": [
            {"role": "user", "content": "describe image"},
            {
                "role": "assistant",
                "content": [{
                    "type": "tool_use",
                    "id": "toolu_read",
                    "name": "Read",
                    "input": {"file_path": "uploads/agent-flow-preview-debug.png"}
                }]
            },
            {
                "role": "user",
                "content": [{
                    "type": "tool_result",
                    "tool_use_id": "toolu_read",
                    "content": [{
                        "type": "image",
                        "source": {
                            "type": "base64",
                            "media_type": "image/png",
                            "data": "aW1hZ2U="
                        }
                    }]
                }]
            },
            {"role": "user", "content": "next question"}
        ]
    }))
    .expect("tool result history has a Native owner");

    assert_eq!(translated.request.history[1]["role"], "assistant");
    assert_eq!(
        translated.request.history[1]["tool_calls"][0]["id"],
        "toolu_read"
    );
    assert_eq!(translated.request.history[2]["role"], "tool");
    assert_eq!(translated.request.history[2]["tool_call_id"], "toolu_read");
    assert_eq!(
        translated.request.history[2]["content_blocks"][0]["type"],
        "image"
    );
}

#[test]
fn latest_tool_result_maps_to_native_query_for_callback_routing() {
    let translated = translate_messages_request(json!({
        "model": "1flowbase",
        "messages": [
            {
                "role": "assistant",
                "content": [{
                    "type": "tool_use",
                    "id": "toolu_read",
                    "name": "Read",
                    "input": {"file_path": "uploads/test-01.png"}
                }]
            },
            {
                "role": "user",
                "content": [{
                    "type": "tool_result",
                    "tool_use_id": "toolu_read",
                    "content": "-rw-r--r-- 1 Lw 197121 17907 Jun 12 15:25 uploads/test-01.png"
                }]
            }
        ]
    }))
    .expect("latest tool result has a Native representation");

    assert_eq!(translated.request.history[0]["role"], "assistant");
    assert_eq!(
        translated.request.history[0]["tool_calls"][0]["id"],
        "toolu_read"
    );
    assert_eq!(
        translated.request.query,
        "-rw-r--r-- 1 Lw 197121 17907 Jun 12 15:25 uploads/test-01.png"
    );
}
